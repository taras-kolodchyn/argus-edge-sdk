#include <WiFi.h>
#include <PubSubClient.h>

const char* ssid = "YOUR_WIFI";
const char* pass = "YOUR_PASS";

const char* mqtt_host = "192.168.0.10"; // or your machine IP
const int   mqtt_port = 1883;
const char* mqtt_user = "devuser";
const char* mqtt_pass = "devpass";

WiFiClient espClient;
PubSubClient client(espClient);

void setup() {
  Serial.begin(115200);
  WiFi.begin(ssid, pass);
  while (WiFi.status() != WL_CONNECTED) { delay(500); Serial.print("."); }

  client.setServer(mqtt_host, mqtt_port);
  while (!client.connected()) {
    client.connect("device-123", mqtt_user, mqtt_pass);
    delay(500);
  }
}

void loop() {
  if (!client.connected()) {
    client.connect("device-123", mqtt_user, mqtt_pass);
  }
  client.loop();

  String topic = "gaia/devices/device-123/telemetry";
  String payload = "{"temp":24.3,"pm25":12,"ts":" + String((unsigned long)millis()) + "}";
  client.publish(topic.c_str(), payload.c_str());
  delay(5000);
}
