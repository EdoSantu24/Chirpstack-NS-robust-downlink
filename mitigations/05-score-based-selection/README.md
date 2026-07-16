# 05 — Composite Score-Based Selection

This is the paper's main contribution (Section IV-C): rather than filtering out suspicious gateways or randomizing among them, it replaces SNR/RSSI-only ranking with a weighted composite score that combines radio quality with each gateway's observed history, so no single self-reported value can dominate the outcome on its own.

## What it does

Every gateway in the candidate pool gets a score: score = 0.17 · RSSI_scaled + 0.17 · SNR_scaled + 0.26 · seniority + 0.40 · join_reliability

- **RSSI_scaled / SNR_scaled**: the gateway's reported values, linearly normalized to [0, 1] between a minimum and maximum expected range.
- **seniority** (`duty_cycle` in code): starts at 0 and increases each time the gateway is selected for a downlink. Rewards gateways with an established track record of actually being used, rather than one that just showed up reporting great numbers.
- **join_reliability**: starts at 1 and decreases whenever the same end device sends two consecutive JoinRequests while this gateway was the one selected to carry the JoinAccept in between. Two JoinRequests in a row from the same device means the JoinAccept never arrived — exactly the signature the attack produces, since the malicious gateway is selected, then silently drops the packet. This is the highest-weighted term, since a pattern of failed deliveries is a much stronger signal of dishonest behavior than an instantaneous radio reading, which can vary naturally or be spoofed either way.

A small non-negative random value (half-Gaussian noise) is added to each gateway's score before the final comparison, so the highest raw score doesn't always win outright — this gives new or previously-penalized gateways an occasional chance to be reselected and rebuild a track record, rather than being locked out permanently by one bad score. The gateway with the highest score-plus-noise is selected.

## What changed 

This mitigation touches more of the ChirpStack tree than any other in this repo, since scoring needs state that outlives a single downlink decision and needs to be updated from both the uplink and downlink sides:

- **`downlink/helpers.rs`** — `select_downlink_gateway` becomes `async` and gains a `dev_eui: Option<&[u8]>` parameter. Candidates that clear the SNR margin are preferred; if none do, scoring runs over the full filtered set instead. After selection, it updates the winning gateway's duty-cycle (always) and join-reliability (only for join flows).
- **`storage/gateway_score.rs`** — new module. Defines the score weights and formula, the RSSI/SNR scaling bounds, and the Redis-backed `GatewayScoreState` (duty cycle, join reliability) plus the consecutive-JoinRequest tracking keyed by DevEUI.
- **`storage/mod.rs`** — one-line addition (`pub mod gateway_score;`) to register the new module so it compiles into the crate.
- **`uplink/join.rs`** — hooks in the consecutive-JoinRequest detection: when a JoinRequest arrives, it checks whether the same device's previous JoinRequest was assigned a gateway that hasn't expired from the tracking window yet, and if so, penalizes that gateway's join-reliability before recording the new gateway.
- **`downlink/join.rs`** — updated to call the new `async` signature with the device's `dev_eui`, so JoinAccept downlinks participate in join-reliability tracking.
- **`uplink/data.rs`** and **`backend/roaming.rs`** — both updated as call into `downlink::helpers`, so both needed to be kept in sync with the new `async` signature for the crate to compile.
  
## Paper mapping

Section IV-C, "Composite Score-based selection". Evaluated in Fig. 4 as the "Score-Based" condition, where the paper reports the legitimate gateway winning 94% of transmissions, with the attacker winning early rounds before its join-reliability penalty accumulates.

## Deploying it

This mitigation needs seven files placed at their corresponding paths in your ChirpStack source tree, not a single drop-in replacement:

| File in this folder | Replaces / adds |
|---|---|
| `downlink/helpers.rs` | `chirpstack/src/downlink/helpers.rs` |
| `downlink/join.rs` | `chirpstack/src/downlink/join.rs` |
| `storage/gateway_score.rs` | new file: `chirpstack/src/storage/gateway_score.rs` |
| `storage/mod.rs` | `chirpstack/src/storage/mod.rs` |
| `uplink/join.rs` | `chirpstack/src/uplink/join.rs` |
| `uplink/data.rs` | `chirpstack/src/uplink/data.rs` |
| `backend/roaming.rs` | `chirpstack/src/backend/roaming.rs` |

This also requires Redis to be reachable from the Network Server, which it already is in the `setup/chirpstack-ns-gw/` Docker deployment. Rebuild and redeploy per that folder's README after placing all seven files.

## Known limitations

- The duty-cycle/seniority and join-reliability parameters update by fixed increments and decrements; suggested adaptive update mechanisms as future work, rather than something evaluated here.
- The malicious gateway still wins in early rounds before its join-reliability penalty has accumulated enough to outweigh its radio-indicator advantage — the 94% legitimate-gateway rate reflects a Network Server that's converged, not immunity from the very first downlink.
- An attacker reporting radio values that stay within the plausible range, or that vary slightly to mimic real channel noise, isn't caught by anything in this scoring formula beyond the (comparatively lower-weighted) radio-indicator terms; the join-reliability term is the one actually doing the work against this specific attack, and it depends on the attacker continuing to drop packets it's assigned.
