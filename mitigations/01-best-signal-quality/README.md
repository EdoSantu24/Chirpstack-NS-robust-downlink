# 01 — Best Signal Quality (Baseline)

This is the vulnerable baseline every other mitigation in this repo is compared against. It implements the "best signal wins" downlink gateway selection that the attack exploits: among the gateways that reported an uplink, the Network Server picks whichever one claims the best SNR, with no check on whether that value is genuine.

## What it does

`select_downlink_gateway` filters out gateways from other tenants that don't allow private downlinks, sorts the remaining candidates by SNR (RSSI as tiebreaker), and keeps only the ones whose SNR clears the required margin for the device's data rate. It then deterministically returns the top candidate from that filtered, sorted list.

Because the malicious gateway always reports an artificially excellent SNR/RSSI (see `setup/malicious-gw/`), it always sorts to the top of this list and is selected for every downlink — which is exactly the failure mode Section III of the paper describes.

Section III (Vulnerability and Threat Model) describes the attack against this exact behavior. Section IV's mitigations are all evaluated relative to this baseline; it corresponds to the "Best RSSI/SNR" condition in the Fig. 4 evaluation.

## Deploying it

Replace `chirpstack/src/downlink/helpers.rs` in your ChirpStack source tree with `downlink/helpers.rs` from this folder, then rebuild and redeploy per `setup/chirpstack-ns-gw/README.md`. This is also the state to return to between testing other mitigations, since 02 and 03 (the detection mechanisms) are meant to run on top of this selection logic rather than replace it.

## Known limitations

This is the vulnerable condition the whole paper responds to, so its "limitation" is the point: it has no defense against spoofed metadata at all. A gateway that reports the best SNR wins, regardless of whether that value is real, and regardless of whether the gateway ever actually transmits the downlink it's assigned.
