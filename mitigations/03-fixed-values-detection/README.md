# 03 — Stationary Value Detection

This is the second detection mechanism from the paper (Section IV-A), also meant to run on top of an existing selection algorithm rather than replace it. Where `02-too-good-to-be-true` catches gateways reporting a single implausible reading, this one catches a different signature of spoofing: values that show no variance at all.

## What it detects

Real radio links show variation in RSSI and SNR from one uplink to the next, even from a single stationary end device, because of multipath propagation. Uplinks from different end devices vary even more, since they traverse different paths entirely. A gateway whose reported RSSI and SNR stay statistically identical across a run of uplinks isn't behaving like it's measuring a real channel, it rather looks like a hardcoded value being replayed regardless of what's actually happening on the radio link, which is a cheap and obvious way for a spoofing gateway to implement the attack this repo simulates.

The code implements this by keeping an in-memory history per gateway (last reported RSSI, last reported SNR, and a consecutive-match counter). If a gateway reports the exact same RSSI and SNR five times in a row, it's flagged as suspicious and dropped from the candidate pool for that downlink.

## What changed vs. the baseline

This folder's `helpers.rs` is the `01-best-signal-quality` baseline logic with a stationary-value filter added before the SNR-based sort, the same insertion point `02` uses. The difference is that this filter needs memory across calls rather than judging each report in isolation, so it maintains a per-gateway match count: matching the previous report increments the count, any change resets it to zero, and hitting (five, in our implementation) consecutive matches drops that gateway from the candidate pool for the current downlink and logs a `[SECURITY ALERT]` line. Everything downstream of the filter: sorting, margin check, returning the top candidate, is unchanged from the baseline.

## Paper mapping

Section IV-A, "Stationary value detection." Evaluated in Fig. 4 as the "Stationary Value" condition.

## Deploying it

Same as the other mitigations: replace `chirpstack/src/downlink/helpers.rs` in your ChirpStack source tree with `helpers.rs` from this folder, then rebuild and redeploy per `setup/chirpstack-ns-gw/README.md`.

## Known limitations

This only catches an attacker that reports the literal same value repeatedly. An attacker that adds even small variation to its spoofed RSSI/SNR defeats the detection entirely while still reporting values good enough to win the baseline's "best SNR wins" selection underneath. The five-in-a-row threshold is also a fixed constant; we didn't evaluate sensitivity to shorter or longer windows, or to a gateway that varies its spoofed value only occasionally to reset the counter before it trips.
