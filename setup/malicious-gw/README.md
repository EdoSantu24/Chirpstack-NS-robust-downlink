# Gateway logic

This folder contains modifications to a LoRaWAN gateway packet forwarder, developed to study and evaluate vulnerabilities in the downlink gateway selection process of LoRaWAN Network Servers.

The prototype demonstrates how falsified radio metadata, combined with malicious gateway behavior, can bias gateway selection and lead to a Denial of Service (DoS) condition for end devices.  


> This code is provided **for academic research, controlled experiments, and defensive security evaluation only**.  
> Running modified gateways on production networks or networks without explicit authorization may violate regulations, service agreements, and local laws.
>
> The authors assume **no responsibility** for misuse of the material provided in this directory.


## Hardware and Software Requirements

- Raspberry Pi 4  
- WM1302 LoRaWAN Gateway Module (SPI interface)
- **Library:** Official Hardware Abstraction Layer (HAL) from the `Lora-net/sx1302_hal` repository

## Implementation Details 

The attack is implemented by modifying the `lora_pkt_fwd.c` file to manipulate the feedback provided to the Network Server and disrupt the physical delivery of downlink packets.

- **Faking Signal Quality:** The RSSI and SNR are set manually to fixed best values. These values ensure the gateway always appears to have the best link quality, forcing the Network Server to select it for all downlink traffic.
- **Silent Packet Dropping:** The lgw_send function is commented out. This prevents the gateway from physically transmitting the downlink packet to the end device, while the gateway continues to send valid acknowledgments (ACKs) to the server.

## Installation Steps
- Clone the official repository: git clone https://github.com/Lora-net/sx1302_hal.git.
- Replace the original `packet_forwarder/src/lora_pkt_fwd.c` with the version provided in this folder.
- Recompile by running: `make`.
- Launch the malicious gateway using `./lora_pkt_fwd-c global_conf.json.sx1250.EU868` (for EU868 region).
