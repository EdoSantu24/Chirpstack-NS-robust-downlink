# Testing downlink

This script is designed to automate the testing of gateway selection logic within the ChirpStack Network Server. By sending a sequence of downlink packets, we can verify which gateway is selected for transmission. 

The script executes a testing cycle consisting of the following steps:

- **Packet Injection:** Enqueues a defined number of downlink packets (default: 20) using the ChirpStack gRPC API.

- **Monitoring:** Subscribes to the MQTT broker to intercept the "downlink command" messages sent from the server to the gateways.

- **Identification:** Captures the Gateway ID for each intercepted packet to track which gateway was selected by the Network Server's internal logic.

- **Data Export:** Generates an Excel report containing timestamp log and a statistical summary (packet count and percentage per gateway).

## Configuration

Before running the script, the following parameters must be configured:

- **SERVER_IP:** The IP address and port of your ChirpStack server.
- **MQTT_BROKER:** The IP address of your Mosquitto broker.
- **DEV_EUI:** The unique identifier of the LoRaWAN end device used.
- **API_TOKEN:** A valid ChirpStack API key to enqueue messages.

