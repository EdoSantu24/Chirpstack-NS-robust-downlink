# End-Device Setup

The end device is an ESP32 DEVKIT V1 interfaced with a Microchip RN2483 LoRaWAN module and an 868 MHz antenna. In this testbed it plays the role of the attack's target: it sends periodic uplinks and waits on downlink reception, so any disruption to the downlink path shows up directly in its behavior.

The device joins via OTAA and operates as a Class A device on SF7 / 125 kHz, per LoRaWAN 1.0.4. Datasheets for both the RN2483A and the ESP-WROOM-32 module are in `docs/esp32-rn2483a/`.

## Behavior under attack

When the malicious gateway (`setup/malicious-gw/`) is selected for downlink and silently drops the JoinAccept or a queued downlink, the device never receives it and retries. Our communication stack backs off roughly one minute between consecutive failed attempts before retrying again — this is what produces the repeated retry pattern behind the energy-consumption results in the paper (Section III-C, Fig. 2).
