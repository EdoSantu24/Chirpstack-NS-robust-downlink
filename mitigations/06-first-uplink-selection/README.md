# 06 — First-Uplink-First-Sent

This is the second selection algorithm from the paper (Section IV-B), and the one that goes furthest in removing trust from the routing decision: it doesn't just filter or reweight the reported RSSI/SNR values, it ignores them completely.

## What it does

Instead of ranking candidates by signal quality at all, `select_downlink_gateway` selects whichever gateway's copy of the uplink reached the Network Server first. ChirpStack collects uplink reports from every receiving gateway during a roughly 200 ms deduplication window and appends them to `rx_info.items` in the order they arrive over each gateway's backhaul link so the item at index 0 is, by construction, the one that got there first. The routing decision becomes a fact the Network Server observes directly (arrival order) rather than a value a gateway self-reports (RSSI/SNR).

## What changed vs. the baseline

Same tenant/private-downlink filtering as `01-best-signal-quality`. Everything after that is gone: no SNR-based sort, no margin check, no `region_config_id`/`min_snr_margin` use (both parameters are kept in the function signature for compatibility but prefixed with `_` since they're now unused). The function just returns `rx_info.items[0]` directly off the filtered set. The `region` and `rand` imports the baseline needed are dropped entirely, since neither SNR margin calculation nor randomization happens here anymore.

## Paper mapping

Section IV-B, "First-uplink-first-sent." 

We specifically tested position sensitivity for this mitigation, since its whole premise depends on relative timing rather than signal strength — see `results/06-first-uplink-selection/` for the normal-layout run alongside two runs with the end device moved closer to each gateway in turn.

## Deploying it

Replace `chirpstack/src/downlink/helpers.rs` with `helpers.rs` from this folder, then rebuild and redeploy per `setup/chirpstack-ns-gw/README.md`.

## Known limitations

These are the paper's own stated limitations for this mitigation, not just ours:

- The propagation delay term (`Tprop`) is negligible in practice, about 33 μs for a 10 km link, compared to time-on-air (tens of milliseconds to seconds, depending on spreading factor) and especially backhaul latency, which can reach hundreds of milliseconds on cellular connections. Since `Tair` and `Tns` are effectively constant across gateways for a given uplink, arrival order ends up driven mainly by `Tbh(g)`, with a smaller contribution from each gateway's processing time.
- The practical consequence: this mechanism can end up selecting whichever gateway has the faster backhaul connection, not whichever one has the strongest radio link or is physically closest to the end device. A gateway with excellent signal quality but a slow backhaul can lose to one with a weaker link but a faster connection back to the server — the opposite of what "best signal" mechanisms optimize for, and not necessarily correlated with which gateway is actually best positioned to deliver the downlink reliably.
- Because RSSI/SNR are ignored entirely, this mitigation gives up any ability to prefer a stronger link when arrival times are close, and offers no defense at all against an attacker who simply also has fast backhaul.
