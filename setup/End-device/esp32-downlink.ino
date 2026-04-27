#include <rn2xx3.h>
#include <HardwareSerial.h>

// -----------------------------------------------------------
// 1. PIN CONFIGURATION
// -----------------------------------------------------------
#define RX_PIN 16   // Connect to RN2483 TX
#define TX_PIN 17   // Connect to RN2483 RX
#define RST_PIN 4   // Connect to RN2483 Reset

// -----------------------------------------------------------
// 2. LORAWAN KEYS (REPLACE WITH YOUR CHIRPSTACK KEYS)
// -----------------------------------------------------------
// Obtain these from your ChirpStack Device details page
const char *appEui = "XXXXXXXXXXXXXXXX";  
const char *appKey = "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";  

// -----------------------------------------------------------
// 3. SERIAL SETUP
// -----------------------------------------------------------
HardwareSerial myLoRaSerial(2);  
rn2xx3 myLora(myLoRaSerial);

void setup() {
  Serial.begin(115200);
  while (!Serial && millis() < 3000);  

  Serial.println("\n=== ESP32 LoRaWAN Security Testbed ===");

  // Initialize RN2483 Serial communication
  myLoRaSerial.begin(57600, SERIAL_8N1, RX_PIN, TX_PIN);

  // Hardware Reset for RN2483
  pinMode(RST_PIN, OUTPUT);
  digitalWrite(RST_PIN, LOW);
  delay(100);
  digitalWrite(RST_PIN, HIGH);
  delay(1000); 

  initialize_radio();

  // JOIN PROCEDURE
  Serial.println("Attempting to join ChirpStack via OTAA...");

  bool join_result = false;

  // Loop until successfully joined to the Network Server
  while (!join_result) {
    join_result = myLora.initOTAA(appEui, appKey);
    if (!join_result) {
      Serial.println("Join failed. Retrying in 10 seconds...");
      delay(10000);
    }
  }

  Serial.println("Successfully joined Network Server!");
}

void loop() {
  Serial.println("Sending Uplink: 'Hello_ChirpStack'...");

  // Send message and check for RX windows (Downlinks)
  switch (myLora.tx("Hello_ChirpStack")) {
    case TX_SUCCESS:
      Serial.println("TX Successful (No Downlink received)");
      break;
    case TX_WITH_RX:
      Serial.println("TX Successful + Downlink Received!");
      Serial.print("Downlink Payload: ");
      Serial.println(myLora.getRx());
      break;
    default:
      Serial.println("TX Failed - Check Gateway connectivity");
      break;
  }

  // Frequency of transmissions - Adjusted for 20-packet test scenarios
  delay(30000); 
}

void initialize_radio() {
  myLora.autobaud();

  String hweui = myLora.hweui();
  if (hweui.length() != 16) {
    Serial.println("Communication with RN2483 failed! Check wiring.");
    while (1); 
  }

  Serial.print("Radio Connected. DevEUI: ");
  Serial.println(hweui);
}
