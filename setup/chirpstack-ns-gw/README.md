# Setup configuration

This folder contains only the specific configuration files created or modified for this project. It does not contain the full ChirpStack source code, which should be cloned separately from the official repository to perform a deployment from source.

Deploying the ChirpStack Network Server from source was a critical architectural decision for this project because it provided direct access to the internal codebase, which is not possible when using the pre-compiled images in the standard chirpstack-docker setup.

For technical details on how to setup the docker environment refer to the official [ChirpStack GitHub repository](https://github.com/chirpstack/chirpstack).

---

The files included in this directory are intended to replace or supplement the defaults in the cloned repository to support the Raspberry Pi Docker environment.

These files are placed in the root folder (`chirpstack/chirpstack/`):

* **docker-compose.yml**: Orchestrates the containerized infrastructure, including PostgreSQL, Redis, Mosquitto, and the Gateway Bridge.
* **chirpstack-gateway-bridge.toml**: Configures the bridge to translate Semtech UDP packets from the gateway (port 1700) into MQTT messages for the server. 

These files are placed in a new folder called `.config/` inside `chirpstack/chirpstack/` directory of the official repository:
* **chirpstack.toml**: The primary Network Server configuration.
* **region_eu868.toml**: Defines physical radio parameters for the EU868 region. 
