# Configuration Files

A `config` folder was created inside chirpstack repository to isolate the specific settings required for this project from the generic templates provided in the ChirpStack source code (i.e. inside the ./configuration folder). By using a dedicated directory, the Network Server can be executed with a configuration optimized for a Docker-based development environment on our Raspberry Pi.

This approach follows the ChirpStack v4 best practice of maintaining a main configuration file alongside separate region-specific files.

---

## `chirpstack.toml`

This file serves as the primary configuration for the Network Server infrastructure. It was generated specifically to align with the compiled version of the server and then modified to ensure reliable communication between Docker containers.

### Key Changes

- **Database Connections**  
  The PostgreSQL DSN and Redis server addresses were updated to point to the `postgres` and `redis` container names instead of `localhost`.

- **MQTT Integration**  
  The MQTT server address was changed to `tcp://mosquitto:1883` to allow the server to communicate with the Mosquitto broker.

- **Logging Fixes**  
  A duplicate `json = false` entry was commented out to prevent configuration errors during startup.

- **Region Activation**  
  The `eu868` region was added to the `enabled_regions` vector to ensure the server loads the correct radio parameters for testing.

- **Role Suffix**  
  The `use_target_role_suffix` parameter was modified to resolve UI-related errors.

---

## `region_eu868.toml`

This file defines the physical radio parameters and gateway backend settings specifically for the EU868 region.
The file was downloaded directly from the official ChirpStack repository to ensure regional accuracy.

---

These files are essential when running the Network Server from source after modifying the helpers.rs file. To start the server using these specific configurations, execute the following command from the chirpstack/chirpstack directory:

`cargo run --config ./config`
