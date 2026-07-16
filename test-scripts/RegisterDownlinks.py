import paho.mqtt.client as mqtt
from chirpstack_api import gw
import csv
from datetime import datetime
import os
import sys

# --- Configuration ---
# Use environment variables or generic placeholders for public repositories
BROKER_IP = os.environ.get("MQTT_BROKER_IP", "your.broker.ip.here")
BROKER_PORT = 1883 
CSV_FILENAME = "downlink_routing_logs.csv" # Saved to the current working directory

# --- Tracking Variables for Summary ---
total_downlinks = 0
gateway_counts = {}

# --- LoRaWAN Message Type Dictionary (Downlinks Only) ---
MTYPE_MAP = {
    1: "Join-Accept",           
    3: "Unconfirmed Data Down", 
    5: "Confirmed Data Down",   
    7: "Proprietary"            
}

# Create the CSV file and write the header if it doesn't exist
if not os.path.exists(CSV_FILENAME):
    with open(CSV_FILENAME, mode='w', newline='') as file:
        writer = csv.writer(file)
        writer.writerow(["Timestamp", "Selected Gateway ID", "Message Type", "Raw PHYPayload (Hex)"])

# Helper function to process the physical payload and write to CSV
def process_downlink(phy_payload, gw_id):
    global total_downlinks, gateway_counts
    
    if not phy_payload:
        return
        
    # 1. Update our tracking counters
    total_downlinks += 1
    gateway_counts[gw_id] = gateway_counts.get(gw_id, 0) + 1
        
    gw_specific_count = gateway_counts[gw_id]
        
    # 2. Extract the MType
    mhdr = phy_payload[0]
    mtype_bits = (mhdr >> 5) & 0x07
    packet_type = MTYPE_MAP.get(mtype_bits, f"Unknown ({mtype_bits})")
    
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    payload_hex = phy_payload.hex()
    
    # 3. Print to terminal WITH running counts
    print(f"[{timestamp}] DOWNLINK #{total_downlinks} ROUTED | {packet_type.ljust(22)}")
    print(f"   -> Chosen Gateway: {gw_id} (Gateway's packet #{gw_specific_count})")
    print(f"   -> Payload Hex:  {payload_hex}\n")
    
    # 4. Save the log to CSV
    with open(CSV_FILENAME, mode='a', newline='') as file:
        writer = csv.writer(file)
        writer.writerow([timestamp, gw_id, packet_type, payload_hex])


# Callback: When connected to the MQTT broker
def on_connect(client, userdata, flags, reason_code, properties=None):
    print(f"Connected to MQTT Broker at {BROKER_IP}:{BROKER_PORT}")
    print(f"Intercepting Network Server Routing Decisions. Logging to '{CSV_FILENAME}'...")
    print("Press Ctrl+C to stop the script and generate the final summary report.\n")
    client.subscribe("+/gateway/+/command/down")


# Callback: When an MQTT message is received
def on_message(client, userdata, msg):
    try:
        downlink_frame = gw.DownlinkFrame()
        downlink_frame.ParseFromString(msg.payload)
        
        gw_id = downlink_frame.gateway_id
        if isinstance(gw_id, bytes): gw_id = gw_id.hex()
        
        for item in downlink_frame.items:
            process_downlink(item.phy_payload, gw_id)

    except Exception:
        pass


# Function to generate the final percentage summary
def write_final_summary():
    print("\nGenerating final gateway usage summary...")
    
    with open(CSV_FILENAME, mode='a', newline='') as file:
        writer = csv.writer(file)
        
        writer.writerow([])
        writer.writerow([])
        writer.writerow(["--- FINAL ROUTING SUMMARY ---", "", "", ""])
        writer.writerow(["Total Downlinks Processed:", total_downlinks, "", ""])
        writer.writerow(["Gateway ID", "Total Routed", "Percentage of Traffic", ""])
        
        if total_downlinks > 0:
            sorted_gws = sorted(gateway_counts.items(), key=lambda x: x[1], reverse=True)
            
            for gw_id, count in sorted_gws:
                percentage = (count / total_downlinks) * 100
                perc_string = f"{percentage:.2f}%" 
                
                writer.writerow([gw_id, count, perc_string, ""])
                print(f"   -> Gateway {gw_id}: {count} downlinks ({perc_string})")
        else:
            writer.writerow(["No downlinks were routed during this session.", "", "", ""])
            print("   -> No downlinks were captured.")
            
    print(f"Summary saved to {CSV_FILENAME}")


if __name__ == "__main__":
    try:
        client = mqtt.Client(mqtt.CallbackAPIVersion.VERSION2)
    except AttributeError:
        client = mqtt.Client()
        
    client.on_connect = on_connect
    client.on_message = on_message

    try:
        client.connect(BROKER_IP, BROKER_PORT, 60)
        client.loop_forever()
        
    except KeyboardInterrupt:
        write_final_summary()
        sys.exit(0)
    except Exception as e:
        print(f"\n Connection error: {e}")
