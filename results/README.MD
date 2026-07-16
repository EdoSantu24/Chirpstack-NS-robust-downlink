# Results

This folder contains the raw evaluation logs behind Section V of the paper. Each subfolder corresponds to one mitigation from `mitigations/`, generated using the scripts in `test-scripts/`: enqueue a batch of downlinks, then record which gateway the Network Server assigned each one to.

We only kept detailed logs for `04-randomization`, `05-score-based-selection`, and `06-first-uplink-selection`; `01`–`03` are also reported in the paper's evaluation (Fig. 4) but we don't have separate raw logs for them here.

## `04-randomization/`

- **routing-log.xlsx** — per-downlink gateway assignment and the resulting legitimate/malicious split.

## `05-score-based-selection/`

- **routing-log.xlsx** — per-downlink gateway assignment, same format as above.
- **ns-score-log.xlsx** — Network Server-side log of the score breakdown per gateway per downlink (RSSI/SNR scaled values, seniority and join-reliability contributions, noise, final score), as printed by the `[GW SCORE]` log lines in `mitigations/05-score-based-selection/downlink/helpers.rs`.

## `06-first-uplink-selection/`

Three runs varying the end device's position relative to the two gateways, since this mitigation's effectiveness depends on relative arrival time rather than signal strength:

- **routing-log-normal.xlsx** — standard testbed layout (gateways roughly equidistant from the device).
- **routing-log-device-near-legitimate-gw.xlsx** — device moved closer to the legitimate gateway.
- **routing-log-device-near-malicious-gw.xlsx** — device moved closer to the malicious gateway.
