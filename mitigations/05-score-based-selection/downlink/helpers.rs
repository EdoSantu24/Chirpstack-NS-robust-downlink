// chirpstack/src/downlink/helpers.rs
//
// Changes vs. original:
//   • `select_downlink_gateway` is now **async** and accepts an extra
//     `dev_eui: Option<&[u8]>` parameter.
//   • Gateway selection is now **score-based** instead of SNR-only:
//       score = 0.17·RSSI_scaled + 0.17·SNR_scaled
//             + 0.33·(1−duty_cycle) + 0.33·join_reliability
//   • The `min_snr_margin` guard is preserved: candidates that meet the margin
//     are preferred; the composite score breaks ties (and selects among the
//     preferred set). If no candidate meets the margin, score-based selection
//     runs over the full filtered set.
//   • After selecting a gateway the function updates its score state in Redis
//     (duty cycle always; join reliability only for join flows).

use std::str::FromStr;

use anyhow::Result;
use tracing::{info, trace, warn};
use uuid::Uuid;

use chirpstack_api::{gw, internal};
use lrwn::region::DataRateModulation;

use crate::config;
use crate::region;
use crate::storage::gateway_score::{self, GatewayScoreState};

use rand::{RngExt, SeedableRng, rngs::StdRng};

/// Standard deviation of the half-Gaussian noise added to each gateway score.
/// With σ=0.08: ~84% of noise values fall below 0.16, rare boosts up to ~0.40.
/// Increase to allow more upsets; decrease for more deterministic selection.
const SCORE_NOISE_SIGMA: f64 = 0.1;

// ─── Gateway selection ────────────────────────────────────────────────────────

/// Selects the best gateway for a downlink transmission using a four-component
/// composite score.
///
/// Parameters
/// ----------
/// `tenant_id`        – when `Some`, private-down gateways from other tenants
///                      are filtered out (same behaviour as before).
/// `region_config_id` – used to derive the required SNR for the current DR.
/// `min_snr_margin`   – SNR head-room above the required minimum.  Gateways
///                      that satisfy this margin form the "preferred" pool from
///                      which the highest-scoring one is chosen.  If the
///                      preferred pool is empty the entire (tenant-filtered) set
///                      is used instead.
/// `rx_info`          – mutable set of RxInfo items; filtered in-place.
/// `dev_eui`          – `Some(eui_bytes)` for join-request flows so that
///                      consecutive-JR tracking and join_reliability updates are
///                      applied.  Pass `None` for regular data downlinks.
///
/// Returns the selected `DeviceGatewayRxInfoItem`.
pub async fn select_downlink_gateway(
    tenant_id: Option<Uuid>,
    region_config_id: &str,
    min_snr_margin: f32,
    rx_info: &mut internal::DeviceGatewayRxInfo,
    dev_eui: Option<&[u8]>,
) -> Result<internal::DeviceGatewayRxInfoItem> {
    // ── 1. Filter out private-down gateways from other tenants ───────────────
    rx_info.items.retain(|item| {
        if let Some(tenant_id) = &tenant_id {
            if tenant_id.as_bytes().to_vec() == item.tenant_id {
                true
            } else {
                !item.is_private_down
            }
        } else {
            !item.is_private_down
        }
    });

    if rx_info.items.is_empty() {
        return Err(anyhow!(
            "RxInfo set is empty after applying tenant filters, no downlink gateway available"
        ));
    }

    // ── 2. Derive required SNR for the current DR (LoRa only) ────────────────
    let region_conf = region::get(region_config_id)?;
    let dr = region_conf.get_data_rate(true, rx_info.dr as u8)?;
    let required_snr: Option<f32> = if let DataRateModulation::Lora(dr) = dr {
        Some(config::get_required_snr_for_sf(dr.spreading_factor)?)
    } else {
        None
    };

    // ── 3. Load score states and compute composite scores ────────────────────
    struct Candidate {
        item: internal::DeviceGatewayRxInfoItem,
        score: f64,
        meets_snr_margin: bool,
    }

    let mut candidates: Vec<Candidate> = Vec::with_capacity(rx_info.items.len());

    for item in &rx_info.items {
        // Fetch dynamic state from Redis; fall back to the safe default on error.
        let state: GatewayScoreState = gateway_score::get_gateway_score_state(&item.gateway_id)
            .await
            .unwrap_or_default();

        let score = gateway_score::compute_score(item.rssi, item.lora_snr, &state);

        let meets_snr_margin = match required_snr {
            Some(req) => item.lora_snr - req >= min_snr_margin,
            None => false, // FSK: no SNR concept, treat as "does not meet" to use full set
        };

        info!(
            "[GW SCORE] gateway={} | RSSI={} (scaled={:.3}) | SNR={} (scaled={:.3}) \
            | DutyCycle={:.3} (contrib={:.3}) | JoinReliability={:.3} (contrib={:.3}) \
            | TOTAL_SCORE={:.4} | meets_snr_margin={}",
            hex::encode(&item.gateway_id),
            item.rssi,
            gateway_score::scale_rssi(item.rssi),
            item.lora_snr,
            gateway_score::scale_snr(item.lora_snr),
            state.duty_cycle,
            gateway_score::WEIGHT_DUTY_CYCLE * (state.duty_cycle),
            state.join_reliability,
            gateway_score::WEIGHT_JOIN_RELIABILITY * state.join_reliability,
            score,
            meets_snr_margin,
        );

        candidates.push(Candidate {
            item: item.clone(),
            score,
            meets_snr_margin,
        });
    }

    // ── 4. Select the highest-scoring candidate ───────────────────────────────
    //
    // Prefer candidates that meet the SNR margin; if none do, use all candidates.
    let preferred: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| c.meets_snr_margin)
        .collect();

    let pool: Vec<&Candidate> = if preferred.is_empty() {
        candidates.iter().collect()
    } else {
        preferred
    };


    // Sample half-Gaussian noise once per candidate using Box-Muller transform.
    // Each gateway's effective score = base score + |N(0, σ)|.
    // The noise is always non-negative, so a gateway's score can only be boosted,
    // never penalised. The mode of the noise is 0 (most of the time the boost is
    // negligible), but the tail gives lower-scored gateways a rare chance to win.


    //let mut rng = rand::rng();
    let noised: Vec<f64> = {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as u64;
        let mut rng = StdRng::seed_from_u64(seed);
        pool
            .iter()
            .map(|c| {
                let u1: f64 = rng.random::<f64>().max(1e-10);
                let u2: f64 = rng.random::<f64>();
                let gaussian = (-2.0_f64 * u1.ln()).sqrt()
                    * (2.0_f64 * std::f64::consts::PI * u2).cos();
                let noise = gaussian.abs() * SCORE_NOISE_SIGMA;
                let final_score = c.score + noise;

                info!(
                    "[GW SCORE] gateway={} | base_score={:.4} | noise={:.4} | final_score={:.4}",
                    hex::encode(&c.item.gateway_id),
                    c.score,
                    noise,
                    final_score,
                );

                final_score
            })
            .collect()
    };
    let selected_idx = noised
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .expect("pool is non-empty");

    let best_score = pool[selected_idx].score;
    let noise_applied = noised[selected_idx] - best_score;
    let selected = pool[selected_idx].item.clone();

    info!(
        "[GW SCORE] >>> Selected gateway={} | base_score={:.4} | noise={:.4} | final_score={:.4}",
        hex::encode(&selected.gateway_id),
        best_score,
        noise_applied,
        noised[selected_idx],
    );

    // ── 5. Update score state in Redis ────────────────────────────────────────
    if let Some(eui) = dev_eui {
        // Join flow: track consecutive JRs and update duty cycle.
        if let Err(e) =
            gateway_score::update_scores_on_join_selection(eui, &selected.gateway_id).await
        {
            warn!(error = %e, "Failed to update join-flow gateway scores in Redis");
        }
    } else {
        // Data flow: update duty cycle only.
        if let Err(e) =
            gateway_score::update_scores_on_data_selection(&selected.gateway_id).await
        {
            warn!(error = %e, "Failed to update data-flow gateway scores in Redis");
        }
    }

    Ok(selected)
}

// ─── TX info helpers (unchanged from original) ───────────────────────────────

pub fn set_tx_info_data_rate(
    tx_info: &mut chirpstack_api::gw::DownlinkTxInfo,
    dr: &DataRateModulation,
) -> Result<()> {
    match dr {
        DataRateModulation::Lora(v) => {
            tx_info.modulation = Some(gw::Modulation {
                parameters: Some(gw::modulation::Parameters::Lora(gw::LoraModulationInfo {
                    bandwidth: v.bandwidth,
                    spreading_factor: v.spreading_factor as u32,
                    code_rate: gw::CodeRate::from_str(&v.coding_rate)
                        .map_err(|e| anyhow!("{}", e))?
                        .into(),
                    polarization_inversion: true,
                    code_rate_legacy: "".into(),
                    preamble: 0,
                    no_crc: false,
                })),
            });
        }
        DataRateModulation::Fsk(v) => {
            tx_info.modulation = Some(gw::Modulation {
                parameters: Some(gw::modulation::Parameters::Fsk(gw::FskModulationInfo {
                    datarate: v.bitrate,
                    frequency_deviation: v.bitrate / 2,
                })),
            });
        }
        DataRateModulation::LrFhss(_) => {
            return Err(anyhow!("LR-FHSS is not supported for downlink"));
        }
    }

    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::tenant;
    use crate::test;

    // Helper: a DeviceGatewayRxInfo with a single item.
    fn single_item(lora_snr: f32, rssi: i32, gw: Vec<u8>) -> internal::DeviceGatewayRxInfo {
        internal::DeviceGatewayRxInfo {
            dr: 0,
            items: vec![internal::DeviceGatewayRxInfoItem {
                lora_snr,
                rssi,
                gateway_id: gw,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_single_gateway_selected() {
        let _guard = test::prepare().await;

        let gw = vec![0x00u8; 8];
        let mut rx = single_item(-5.0, -80, gw.clone());

        let result = select_downlink_gateway(None, "eu868", 0.0, &mut rx, None)
            .await
            .unwrap();

        assert_eq!(result.gateway_id, gw);
    }

    #[tokio::test]
    async fn test_higher_score_wins() {
        let _guard = test::prepare().await;

        // gw2 has much better RSSI+SNR → should win despite both starting with
        // the same duty_cycle (0) and join_reliability (1).
        let gw1 = vec![0x01u8; 8];
        let gw2 = vec![0x02u8; 8];

        let mut rx = internal::DeviceGatewayRxInfo {
            dr: 0,
            items: vec![
                internal::DeviceGatewayRxInfoItem {
                    lora_snr: -15.0,
                    rssi: -110,
                    gateway_id: gw1.clone(),
                    ..Default::default()
                },
                internal::DeviceGatewayRxInfoItem {
                    lora_snr: 5.0,
                    rssi: -60,
                    gateway_id: gw2.clone(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let result = select_downlink_gateway(None, "eu868", 0.0, &mut rx, None)
            .await
            .unwrap();

        assert_eq!(result.gateway_id, gw2);
    }

    #[tokio::test]
    async fn test_is_private_down_filtered() {
        let _guard = test::prepare().await;

        let t = tenant::create(tenant::Tenant {
            name: "test-tenant".into(),
            ..Default::default()
        })
        .await
        .unwrap();

        let gw_same_tenant = vec![0x01u8; 8];
        let gw_other_tenant = vec![0x02u8; 8];

        let mut rx = internal::DeviceGatewayRxInfo {
            items: vec![
                internal::DeviceGatewayRxInfoItem {
                    gateway_id: gw_same_tenant.clone(),
                    is_private_down: true,
                    tenant_id: t.id.as_bytes().to_vec(),
                    lora_snr: 5.0,
                    rssi: -60,
                    ..Default::default()
                },
                internal::DeviceGatewayRxInfoItem {
                    gateway_id: gw_other_tenant.clone(),
                    is_private_down: true,
                    tenant_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                    lora_snr: 10.0, // better signal, but wrong tenant
                    rssi: -50,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let result = select_downlink_gateway(Some(t.id.into()), "eu868", 0.0, &mut rx, None)
            .await
            .unwrap();

        // Only the same-tenant gateway survives the filter.
        assert_eq!(result.gateway_id, gw_same_tenant);
    }

    /// Verifies the score formula directly.
    #[test]
    fn test_compute_score_weights() {
        use crate::storage::gateway_score::compute_score;

        let perfect = GatewayScoreState {
            duty_cycle: 0.0,
            join_reliability: 1.0,
        };
        // Perfect signal + zero duty cycle + full reliability → score ≈ 1.0
        let s = compute_score(-30, 10.0, &perfect);
        assert!((s - 1.0).abs() < 1e-9, "expected ~1.0, got {s}");

        let worst = GatewayScoreState {
            duty_cycle: 1.0,
            join_reliability: 0.0,
        };
        // Worst signal + full duty cycle + no reliability → score ≈ 0.0
        let s = compute_score(-120, -20.0, &worst);
        assert!(s.abs() < 1e-9, "expected ~0.0, got {s}");
    }
}
