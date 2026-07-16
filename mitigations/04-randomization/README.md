# 04 — Randomized Selection

This is the first "selection algorithm" mitigation in the paper (Section IV-B) — rather than filtering out suspicious gateways before selection like `02` and `03` do, it changes the selection rule itself, so the outcome no longer depends solely on which gateway reports the best numbers.

## What it does

Instead of deterministically returning the top-sorted candidate, `select_downlink_gateway` picks uniformly at random among all gateways that clear the SNR margin, using `rand::seq::SliceRandom::choose`. A gateway that reports the best SNR is still eligible, but it's just one candidate among however many others also cleared the margin — it no longer wins by default.

This is the current behaviour of ChirpStack v4.

## Paper mapping

Section IV-B, "Randomized selection."

## Deploying it

Replace `chirpstack/src/downlink/helpers.rs` with `helpers.rs` from this folder, then rebuild and redeploy per `setup/chirpstack-ns-gw/README.md`.

## Known limitations

- If the malicious gateway is the only one clearing the margin, the eligible set collapses to just that gateway and the attack succeeds with probability 1 — randomization only helps when there's real competition in the eligible set.
- Even with multiple eligible gateways, the Network Server can just as easily pick one barely above the demodulation threshold as one with a much stronger link, since eligibility is binary rather than weighted by signal quality.
- An attacker could run multiple virtual gateway instances on the same physical hardware, each with a distinct identifier, inflating the number of malicious candidates in the eligible set and raising their effective win probability well above 1/N for a single attacker.

More fundamentally, randomization distributes the impact of the attack rather than addressing its root cause: malicious gateways still aren't identified as such, and eligibility still depends entirely on unverified, self-reported SNR values.
