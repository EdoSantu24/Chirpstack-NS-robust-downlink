// chirpstack/src/storage/gateway_score.rs
//
// This module stores and manages the per-gateway dynamic scoring state in Redis.
// Each gateway carries four components that together form a composite score used
// by `downlink::helpers::select_downlink_gateway` to choose the best gateway for
// every downlink transmission.
//
// Score formula (all terms in [0, 1], weights sum to 1.0):
//
//   score = 0.17 × rssi_scaled
//         + 0.17 × snr_scaled
//         + 0.33 × (1 − duty_cycle)      ← inverted: low usage = better
//         + 0.33 × join_reliability
//
// Dynamic state (persisted in Redis, one entry per gateway):
//   • duty_cycle      – starts at 0; increases by DUTY_CYCLE_STEP each time the
//                       gateway is selected for any downlink.
//   • join_reliability – starts at 1; decreases by JOIN_RELIABILITY_STEP each time
//                       the same device issues two consecutive join-requests while
//                       this gateway is the selected downlink (i.e., the JoinAccept
//                       was not received by the device).
//
// Consecutive-join detection (keyed by DevEUI, persisted in Redis with TTL):
//   When a JoinRequest is processed, the network server records which gateway was
//   chosen for the downlink. If the same device sends another JoinRequest before the
//   record expires, it means the previous JoinAccept was lost, so the previously
//   selected gateway's join_reliability is penalised.

use anyhow::Result;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::trace;

use super::get_async_redis_conn;

// ─── Redis key prefixes ───────────────────────────────────────────────────────

const GW_SCORE_KEY_PREFIX: &str = "cs:gw:score";
const LAST_JOIN_GW_KEY_PREFIX: &str = "cs:gw:last_join";

/// How long (seconds) to remember which gateway was selected for a device's last
/// join-request. After this window a retry is treated as a fresh join attempt
/// and does NOT penalise the previous gateway.
const LAST_JOIN_GW_TTL: u64 = 3600; // 1 hour

// ─── Score tuning constants ───────────────────────────────────────────────────

/// Amount by which duty_cycle increases each time the gateway is selected.
pub const DUTY_CYCLE_STEP: f64 = 0.1;

/// Amount by which join_reliability decreases on each consecutive-join event.
pub const JOIN_RELIABILITY_STEP: f64 = 0.1;

// ─── Score component weights (must sum to 1.0) ────────────────────────────────

pub const WEIGHT_RSSI: f64 = 0.17;
pub const WEIGHT_SNR: f64 = 0.17;
pub const WEIGHT_DUTY_CYCLE: f64 = 0.33;
pub const WEIGHT_JOIN_RELIABILITY: f64 = 0.33;

// ─── RSSI / SNR scaling bounds ────────────────────────────────────────────────

/// Minimum expected RSSI (dBm). Values at or below this map to 0.0.
pub const RSSI_MIN: f64 = -120.0;
/// Maximum expected RSSI (dBm). Values at or above this map to 1.0.
pub const RSSI_MAX: f64 = -30.0;

/// Minimum expected SNR (dB). Values at or below this map to 0.0.
pub const SNR_MIN: f64 = -20.0;
/// Maximum expected SNR (dB). Values at or above this map to 1.0.
pub const SNR_MAX: f64 = 50.0;

// ─── Score state ─────────────────────────────────────────────────────────────

/// Dynamic scoring state stored in Redis for a single gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayScoreState {
    /// Accumulated duty-cycle usage [0, 1].
    /// Starts at 0.0 and grows by `DUTY_CYCLE_STEP` each time this gateway is
    /// selected for a downlink, up to a maximum of 1.0.
    pub duty_cycle: f64,

    /// Join-accept delivery reliability [0, 1].
    /// Starts at 1.0 and shrinks by `JOIN_RELIABILITY_STEP` each time this
    /// gateway was selected for a JoinAccept that was apparently not received
    /// (detected by the device retrying with a second consecutive JoinRequest).
    pub join_reliability: f64,
}

impl Default for GatewayScoreState {
    fn default() -> Self {
        Self {
            duty_cycle: 0.0,
            join_reliability: 1.0,
        }
    }
}

// ─── Scaling helpers ──────────────────────────────────────────────────────────

/// Maps a raw RSSI reading (dBm) linearly into [0, 1].
/// `RSSI_MIN` dBm → 0.0,  `RSSI_MAX` dBm → 1.0.
pub fn scale_rssi(rssi: i32) -> f64 {
    ((rssi as f64 - RSSI_MIN) / (RSSI_MAX - RSSI_MIN)).clamp(0.0, 1.0)
}

/// Maps a raw SNR reading (dB) linearly into [0, 1].
/// `SNR_MIN` dB → 0.0,  `SNR_MAX` dB → 1.0.
pub fn scale_snr(snr: f32) -> f64 {
    ((snr as f64 - SNR_MIN) / (SNR_MAX - SNR_MIN)).clamp(0.0, 1.0)
}

/// Computes the composite gateway score from raw signal measurements and the
/// gateway's current dynamic state.  Returns a value in [0, 1]; higher is better.
pub fn compute_score(rssi: i32, snr: f32, state: &GatewayScoreState) -> f64 {
    WEIGHT_RSSI * scale_rssi(rssi)
        + WEIGHT_SNR * scale_snr(snr)
        + WEIGHT_DUTY_CYCLE * state.duty_cycle           // ← direct: more selections = higher score
        + WEIGHT_JOIN_RELIABILITY * state.join_reliability
}

// ─── Redis helpers ────────────────────────────────────────────────────────────

fn gw_score_key(gateway_id: &[u8]) -> String {
    format!("{}:{}", GW_SCORE_KEY_PREFIX, hex::encode(gateway_id))
}

fn last_join_gw_key(dev_eui: &[u8]) -> String {
    format!("{}:{}", LAST_JOIN_GW_KEY_PREFIX, hex::encode(dev_eui))
}

// ─── Public async API ─────────────────────────────────────────────────────────

/// Retrieves the current score state for `gateway_id`.
/// Returns `GatewayScoreState::default()` when no entry exists yet.
pub async fn get_gateway_score_state(gateway_id: &[u8]) -> Result<GatewayScoreState> {
    let key = gw_score_key(gateway_id);
    let mut conn = get_async_redis_conn().await?;
    let val: Option<String> = conn.get(&key).await?;
    Ok(match val {
        Some(s) => serde_json::from_str(&s)?,
        None => GatewayScoreState::default(),
    })
}

/// Persists `state` for `gateway_id` in Redis (no expiry – state is permanent
/// until explicitly reset or the Redis instance is flushed).
pub async fn set_gateway_score_state(gateway_id: &[u8], state: &GatewayScoreState) -> Result<()> {
    let key = gw_score_key(gateway_id);
    let val = serde_json::to_string(state)?;
    let mut conn = get_async_redis_conn().await?;
    conn.set::<_, _, ()>(&key, val).await?;
    Ok(())
}

/// Returns the gateway ID that was previously selected for a JoinRequest from
/// `dev_eui`, if the record has not yet expired.
pub async fn get_last_join_gateway(dev_eui: &[u8]) -> Result<Option<Vec<u8>>> {
    let key = last_join_gw_key(dev_eui);
    let mut conn = get_async_redis_conn().await?;
    let val: Option<String> = conn.get(&key).await?;
    Ok(match val {
        Some(s) => Some(hex::decode(s)?),
        None => None,
    })
}

/// Records that `gateway_id` was selected for a JoinRequest from `dev_eui`.
/// The record expires automatically after `LAST_JOIN_GW_TTL` seconds.
pub async fn set_last_join_gateway(dev_eui: &[u8], gateway_id: &[u8]) -> Result<()> {
    let key = last_join_gw_key(dev_eui);
    let val = hex::encode(gateway_id);
    let mut conn = get_async_redis_conn().await?;
    conn.set_ex::<_, _, ()>(&key, val, LAST_JOIN_GW_TTL).await?;
    Ok(())
}

/// Clears the last-join-gateway record for `dev_eui`.
/// Call this after a JoinAccept has been successfully transmitted so that the
/// next JoinRequest from the same device is not mistakenly treated as a retry.
pub async fn clear_last_join_gateway(dev_eui: &[u8]) -> Result<()> {
    let key = last_join_gw_key(dev_eui);
    let mut conn = get_async_redis_conn().await?;
    conn.del::<_, ()>(&key).await?;
    Ok(())
}

// ─── Composite update helpers (called by downlink::helpers) ───────────────────

/// Called when `selected_gateway_id` is chosen for a **join-request** downlink.
///
/// Actions performed:
/// 1. If `dev_eui` has a previous join-gateway record (the device is retrying),
///    decrement that gateway's `join_reliability` by `JOIN_RELIABILITY_STEP`.
/// 2. Record `selected_gateway_id` as the current gateway for `dev_eui`.
/// 3. Increment `selected_gateway_id`'s `duty_cycle` by `DUTY_CYCLE_STEP`.
pub async fn update_scores_on_join_selection(
    dev_eui: &[u8],
    selected_gateway_id: &[u8],
) -> Result<()> {
    // 1. Penalise the previously selected gateway if the device is retrying.
    if let Some(prev_gw_id) = get_last_join_gateway(dev_eui).await? {
        let mut prev_state = get_gateway_score_state(&prev_gw_id).await?;
        prev_state.join_reliability =
            (prev_state.join_reliability - JOIN_RELIABILITY_STEP).max(0.0);
        set_gateway_score_state(&prev_gw_id, &prev_state).await?;
        trace!(
            prev_gateway = %hex::encode(&prev_gw_id),
            new_join_reliability = prev_state.join_reliability,
            "Decremented join_reliability – consecutive JoinRequest detected \
             (JoinAccept was not received by the device)"
        );
    }

    // 2. Register the newly selected gateway for this device.
    set_last_join_gateway(dev_eui, selected_gateway_id).await?;

    // 3. Accumulate duty cycle for the selected gateway.
    let mut state = get_gateway_score_state(selected_gateway_id).await?;
    state.duty_cycle = (state.duty_cycle + DUTY_CYCLE_STEP).min(1.0);
    set_gateway_score_state(selected_gateway_id, &state).await?;

    trace!(
        gateway = %hex::encode(selected_gateway_id),
        new_duty_cycle = state.duty_cycle,
        "Incremented duty_cycle for selected join-downlink gateway"
    );

    Ok(())
}

/// Called when `gateway_id` is chosen for a **regular data** downlink.
///
/// Only increments `duty_cycle`; join reliability is not affected here.
pub async fn update_scores_on_data_selection(gateway_id: &[u8]) -> Result<()> {
    let mut state = get_gateway_score_state(gateway_id).await?;
    state.duty_cycle = (state.duty_cycle + DUTY_CYCLE_STEP).min(1.0);
    set_gateway_score_state(gateway_id, &state).await?;

    trace!(
        gateway = %hex::encode(gateway_id),
        new_duty_cycle = state.duty_cycle,
        "Incremented duty_cycle for selected data-downlink gateway"
    );

    Ok(())
}
