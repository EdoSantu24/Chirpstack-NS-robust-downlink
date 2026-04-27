# ChirpStack Gateway Bridge Configuration

The `chirpstack-gateway-bridge.toml` file is the configuration file for the ChirpStack Gateway Bridge. In this project, the Bridge convert raw Semtech UDP packet data from the LoRaWAN gateway into MQTT messages for the Network Server.

Since the Network Server is deployed from source, a manual configuration for the Gateway Bridge was required. While the official repository (chirpstack-gateway-bridge on Github) contains the complete source code to build the software, our implementation only uses this specific .toml file to configure a pre-built Docker image. This allows the bridge to function correctly within our network environment without needing to implement or manage the entire codebase.

This file defines the listening ports and the destination MQTT broker.

* **UDP Backend**: Configured to bind to `0.0.0.0:1700`, the standard port for receiving incoming gateway traffic.
* **MQTT Integration**: Defines the topic structure for EU868 gateway events and commands.
* **Server Address**: Specifically configured to use `tcp://mosquitto:1883` to ensure the bridge can communicate with the Mosquitto container within the Docker network.

The Gateway Bridge service must be added to your main `docker-compose.yml` file.

Once the configuration is in place and the bridge is added to the compose file, you can start the container in detached mode: `sudo docker compose up -d gateway-bridge`

To verify that the Gateway Bridge is correctly receiving and forwarding packets while the server is running, you can follow the logs in real time: `sudo docker compose logs -f gateway-bridge`
