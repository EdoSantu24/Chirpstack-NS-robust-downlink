This is the companion repository for *"Robust Downlink Gateway Selection in LoRaWAN: Server-Side Mitigations Against Radio Parameter Spoofing Attacks"*.

It contains only the files we changed relative to a stock ChirpStack v4 Network Server deployment. To reproduce our testbed, clone the [official ChirpStack repository](https://github.com/chirpstack/chirpstack) and layer these files on top of it, as described in `setup/chirpstack-ns-gw/README.md`.

## The attack

LoRaWAN commits to a single gateway for each downlink transmission, and that gateway is chosen by the Network Server based on the RSSI and SNR values gateways report alongside the uplinks they forward. Nothing in the protocol verifies that these values are truthful.

We show that a malicious gateway can exploit this by reporting fixed, artificially excellent RSSI/SNR values, which makes the Network Server select it for every downlink regardless of its real link quality. The gateway then silently drops the downlink instead of transmitting it, while still sending the Network Server a valid acknowledgment. Because the end device never receives the packet, it retries, repeatedly, in the case of the OTAA join procedure, which depends on a downlink (the JoinAccept) to complete. The result is a denial of service: the device can get stuck unable to join the network, and if it does have a session, application downlinks never arrive. We also measured the side effect on the end device's power draw, since every failed receive window and retry adds current consumption that shortens battery life.

The malicious gateway is implemented as a modified Semtech packet forwarder (`setup/malicious-gw/`); everything else in this repo is the corresponding server-side response.

## Server-side mitigations

The attack works because the Network Server trusts unverified radio metadata to make a routing decision. We address this in two ways, matching the structure of Section IV of the paper:

- **Detection mechanisms** run on top of an existing gateway selection algorithm and filter out gateways whose reported metadata looks suspicious, without changing how the final gateway is picked.
- **Selection algorithms** replace the routing policy itself, so the outcome no longer depends solely on self-reported values.

We implemented six variants of `select_downlink_gateway`, the ChirpStack function that picks which gateway transmits a downlink, each isolated in its own folder under `mitigations/`:

| Folder | Paper term | Type | What it does |
|---|---|---|---|
| `01-best-signal-quality` | Best signal quality (baseline) | Selection algorithm | Deterministically picks the candidate with the best SNR/RSSI above the required margin. This is the vulnerable behavior the attack exploits, and the one every other folder is compared against. |
| `02-too-good-to-be-true` | Too good to be true (TGTBT) detection | Detection mechanism | Excludes gateways reporting RSSI/SNR values that are physically implausible for a real link. |
| `03-fixed-values-detection` | Stationary value detection | Detection mechanism | Tracks each gateway's reported values over time and flags one whose reports never vary, since real radio conditions always show some fluctuation. |
| `04-randomization` | Randomized selection | Selection algorithm | Picks uniformly at random among gateways that clear the SNR margin, instead of always taking the best one. |
| `05-score-based-selection` | Composite score-based selection | Selection algorithm | Our main contribution: scores each gateway on RSSI, SNR, seniority, and join-reliability, with a small random component, and picks the highest scorer. Touches multiple files across ChirpStack's `downlink`, `uplink`, `storage`, and `backend` modules — see the folder structure below. |
| `06-first-uplink-selection` | First-uplink-first-sent | Selection algorithm | Ignores RSSI/SNR entirely and picks whichever gateway's copy of the uplink reached the server first. |

Detection mechanisms (02, 03) are meant to be layered on top of a selection algorithm rather than used standalone; in our testbed we ran them on top of the `01` baseline. Selection algorithms (01, 04, 05, 06) are mutually exclusive — only one is active in the Network Server at a time.

Each mitigation folder has its own README with implementation detail, deployment steps, and known limitations.

## Setting up the testbed

We recommend going through the setup folders in this order:

1. **`setup/chirpstack-ns-gw/`** — deploy the Network Server from source via Docker on a Raspberry Pi, and set up the legitimate gateway.
2. **`setup/malicious-gw/`** — build and flash the modified packet forwarder onto a second gateway.
3. **`setup/end-device/`** — flash and configure the ESP32 + RN2483A end device. See `docs/esp32-rn2483a/` for the hardware datasheets.
4. **`mitigations/`** — pick a mitigation folder and apply its file(s) to your ChirpStack source tree in place of the stock downlink selection logic. For every mitigation except `05-score-based-selection` this is a single `helpers.rs` drop-in; for `05` you'll need to place each file at its corresponding path under ChirpStack's `src/` (see the folder's own README for the exact mapping).
5. **`test-scripts/`** — run the provided scripts to enqueue downlinks and confirm which gateway the Network Server selected.


## Ethics and responsible use

> This repository is provided **for academic research, controlled experiments, and defensive security evaluation only**.
>
> Running the malicious gateway code, or any of the mitigation code, against production LoRaWAN networks or devices without explicit authorization may violate regulations, service agreements, and local laws.
>
> We assume **no responsibility** for misuse of the material provided in this repository.
