# 02 — Too Good To Be True (TGTBT) Detection

This is the first of two detection mechanisms in the paper (Section IV-A). Rather than changing how the Network Server picks a gateway, it adds a plausibility check in front of the existing selection logic: gateways reporting radio measurements that are physically implausible for a real link get excluded from the candidate pool before selection happens at all.

## What it detects

The filter rejects any gateway reporting SNR above 20 dB or RSSI above -40 dBm. Real LoRa links essentially never see values this good. These thresholds can be derived from the optimal RSSI/SNR figures reported in LoRa literature, so a gateway claiming to exceed them is reporting something implausible rather than measuring a real channel. This is exactly the failure mode of a naive spoofing attacker: to guarantee it always wins the baseline's "best SNR wins" selection (see `01-best-signal-quality`), it's cheapest to just report saturated, maximally attractive values, which is precisely what this filter is built to catch.

## What changed vs. the baseline

This folder's `helpers.rs` is the `01-best-signal-quality` baseline logic with one addition: immediately after the tenant/private-downlink filter and before the SNR-based sort, a second `retain` pass drops any candidate whose reported SNR or RSSI fails the plausibility check above. Everything downstream — sorting by SNR, filtering by margin, returning the top candidate — is unchanged from the baseline. When a gateway gets filtered here, the code logs a `[SECURITY ALERT]` line with the gateway's ID and the offending RSSI/SNR values, so a Network Server operator has a record of it happening.

If every candidate fails the plausibility check, the function returns an error rather than falling back to an implausible gateway.

## Paper mapping

Section IV-A, "Too good to be true (TGTBT) detection." Evaluated in Fig. 4 as the "TGTBT" condition.

## Deploying it

Same as the baseline: replace `chirpstack/src/downlink/helpers.rs` in your ChirpStack source tree with `helpers.rs` from this folder, then rebuild and redeploy per `setup/chirpstack-ns-gw/README.md`.

## Known limitations

This is a threshold filter, so it only catches an attacker that reports values outside the plausible range, exactly the behavior of the naive spoofing attacker modeled in this repo's `setup/malicious-gw/`. An attacker that reports values just inside the plausible window (or adds a small amount of noise to otherwise-excellent numbers) passes this filter untouched, and since the selection logic underneath is still the deterministic "best SNR wins" baseline, that attacker would still win every downlink it clears the filter for. In other words, this mechanism narrows which spoofed values succeed, but doesn't change the fact that the underlying selection still trusts self-reported metadata.
