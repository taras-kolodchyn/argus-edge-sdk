/*
  Argus Edge SDK – ESP32 Arduino example
  Flow: WiFi -> HTTP register -> HTTP login -> MQTT publish telemetry

  Requirements (Arduino IDE):
  - Board: ESP32 (Arduino core)
  - Libraries: PubSubClient, ArduinoJson

  Configure the values below for your WiFi and the host IP where docker compose runs.
*/

#include <WiFi.h>
#include <HTTPClient.h>
#include <PubSubClient.h>
#include <Preferences.h>
#include <ArduinoJson.h>

// ====== CONFIG ======
// WiFi credentials
const char* WIFI_SSID = "YOUR_WIFI";
const char* WIFI_PASS = "YOUR_PASS";

// Host/IP of your dev machine running docker compose (not "localhost")
const char* AUTH_HOST = "192.168.0.10"; // change to your host IP
const uint16_t AUTH_PORT = 8080;
const char* MQTT_HOST = "192.168.0.10"; // change to your host IP
const uint16_t MQTT_PORT = 1883;

// Device identity and secret used for registration
// Leave DEVICE_ID empty to auto-generate from chip id
const char* DEVICE_ID_CFG = ""; // e.g., "device-123"
const char* PRE_SHARED_SECRET = "testsecret";

// ====== GLOBALS ======
WiFiClient wifiClient;
PubSubClient mqtt(wifiClient);
Preferences prefs;

String device_id;
String reg_token;       // token from /auth/device/register
String mqtt_username;   // from /auth/device/register
String mqtt_password;   // from /auth/device/register
String access_token;    // from /auth/device/login
String device_topic_base;

unsigned long lastPublishMs = 0;
unsigned long lastLoginMs = 0;

static String make_device_id() {
  if (DEVICE_ID_CFG && DEVICE_ID_CFG[0] != '\0') return String(DEVICE_ID_CFG);
  uint64_t mac = ESP.getEfuseMac();
  char buf[32];
  snprintf(buf, sizeof(buf), "esp32-%04X%08X", (uint16_t)(mac >> 32), (uint32_t)mac);
  return String(buf);
}

static bool wifi_connect() {
  Serial.print("[wifi] connecting to "); Serial.println(WIFI_SSID);
  WiFi.mode(WIFI_STA);
  WiFi.begin(WIFI_SSID, WIFI_PASS);
  unsigned long start = millis();
  while (WiFi.status() != WL_CONNECTED && millis() - start < 20000) {
    delay(500);
    Serial.print(".");
  }
  Serial.println();
  if (WiFi.status() == WL_CONNECTED) {
    Serial.print("[wifi] connected: "); Serial.println(WiFi.localIP());
    return true;
  } else {
    Serial.println("[wifi] failed to connect");
    return false;
  }
}

static bool http_post_json(const String& url, const String& body, String& out) {
  HTTPClient http;
  http.begin(url);
  http.addHeader("Content-Type", "application/json");
  int code = http.POST(body);
  out = http.getString();
  http.end();
  Serial.printf("[http] POST %s -> %d\n", url.c_str(), code);
  if (code >= 200 && code < 300) return true;
  Serial.println(out);
  return false;
}

static bool do_register() {
  String url = String("http://") + AUTH_HOST + ":" + AUTH_PORT + "/auth/device/register";
  StaticJsonDocument<256> req;
  req["device_id"] = device_id;
  req["pre_shared_secret"] = PRE_SHARED_SECRET;
  String body;
  serializeJson(req, body);

  String resp;
  if (!http_post_json(url, body, resp)) return false;

  StaticJsonDocument<512> doc;
  auto err = deserializeJson(doc, resp);
  if (err) { Serial.printf("[register] JSON parse error: %s\n", err.c_str()); return false; }

  reg_token = (const char*)doc["token"];
  mqtt_username = (const char*)doc["mqtt_username"];
  mqtt_password = (const char*)doc["mqtt_password"];

  Serial.println("[register] ok");
  Serial.print("  token: "); Serial.println(reg_token);
  Serial.print("  mqtt_user: "); Serial.println(mqtt_username);

  // persist for reuse
  prefs.putString("device_id", device_id);
  prefs.putString("token", reg_token);
  prefs.putString("mqtt_user", mqtt_username);
  prefs.putString("mqtt_pass", mqtt_password);
  return true;
}

static bool do_login() {
  String url = String("http://") + AUTH_HOST + ":" + AUTH_PORT + "/auth/device/login";
  StaticJsonDocument<256> req;
  req["device_id"] = device_id;
  req["token"] = reg_token;
  String body;
  serializeJson(req, body);

  String resp;
  if (!http_post_json(url, body, resp)) return false;

  StaticJsonDocument<512> doc;
  auto err = deserializeJson(doc, resp);
  if (err) { Serial.printf("[login] JSON parse error: %s\n", err.c_str()); return false; }

  access_token = (const char*)doc["access_token"];
  Serial.println("[login] ok");
  lastLoginMs = millis();
  return true;
}

static bool mqtt_connect() {
  mqtt.setServer(MQTT_HOST, MQTT_PORT);
  if (mqtt.connected()) return true;
  Serial.print("[mqtt] connecting to "); Serial.print(MQTT_HOST); Serial.print(":"); Serial.println(MQTT_PORT);
  // Use device_id as client id
  bool ok = mqtt.connect(device_id.c_str(), mqtt_username.c_str(), mqtt_password.c_str());
  if (ok) {
    Serial.println("[mqtt] connected");
    String otaTopic = device_topic_base + "/ota";
    mqtt.subscribe(otaTopic.c_str(), 1);
    Serial.print("[mqtt] subscribed -> "); Serial.println(otaTopic);
  } else {
    Serial.println("[mqtt] connect failed");
  }
  return ok;
}

static void mqtt_publish_telemetry() {
  StaticJsonDocument<256> payload;
  payload["temp"] = 25 + (millis() / 1000) % 3; // demo
  payload["pm25"] = 10;
  payload["noise"] = 42;
  payload["ts"] = millis();
  String out;
  serializeJson(payload, out);

  mqtt.publish(device_topic_base.c_str(), out.c_str());
  Serial.print("[mqtt] published -> "); Serial.println(device_topic_base);
}

static void publish_ota_status(const String& job_id, const char* status, const char* message) {
  String topic = device_topic_base + "/ota/status";
  StaticJsonDocument<256> doc;
  doc["job_id"] = job_id;
  doc["device_id"] = device_id;
  doc["status"] = status;
  doc["message"] = message;
  String out;
  serializeJson(doc, out);
  mqtt.publish(topic.c_str(), out.c_str());
  Serial.printf("[ota] %s -> %s (%s)\n", job_id.c_str(), status, message);
}

static void handle_ota_command(const JsonDocument& doc) {
  const char* job_id_cstr = doc["job_id"] | "";
  if (!job_id_cstr[0]) {
    Serial.println("[ota] missing job_id in command");
    return;
  }
  String job_id(job_id_cstr);
  const char* artifact = doc["artifact_url"] | "";
  const char* version = doc["version"] | "unknown";
  Serial.printf("[ota] received job %s -> version %s\n", job_id_cstr, version);
  if (artifact[0]) {
    Serial.print("[ota] artifact: ");
    Serial.println(artifact);
  }

  publish_ota_status(job_id, "in_progress", "downloading artifact");
  delay(1000);
  publish_ota_status(job_id, "completed", "mock install complete");
}

void mqtt_callback(char* topic, byte* payload, unsigned int length) {
  String topicStr(topic);
  if (topicStr != device_topic_base + "/ota") {
    return;
  }
  StaticJsonDocument<512> doc;
  DeserializationError err = deserializeJson(doc, payload, length);
  if (err) {
    Serial.printf("[ota] command parse error: %s\n", err.c_str());
    return;
  }
  handle_ota_command(doc);
}

void setup() {
  Serial.begin(115200);
  delay(1000);
  device_id = make_device_id();
  Serial.print("[boot] device_id: "); Serial.println(device_id);
  prefs.begin("argus", false);
  // try load cached credentials
  String cached_dev = prefs.getString("device_id", "");
  if (cached_dev.length()) {
    device_id = cached_dev; // preserve stable id between boots if present
  }
  device_topic_base = String("gaia/devices/") + device_id;
  reg_token = prefs.getString("token", "");
  mqtt_username = prefs.getString("mqtt_user", "");
  mqtt_password = prefs.getString("mqtt_pass", "");

  if (!wifi_connect()) return;
  // If we don't have cached creds, register; otherwise use cached
  if (reg_token.isEmpty() || mqtt_username.isEmpty() || mqtt_password.isEmpty()) {
    if (!do_register()) return;
  } else {
    Serial.println("[boot] using cached MQTT credentials");
  }
  // login to obtain access_token (e.g., for future API calls)
  if (!do_login()) return;

  mqtt.setCallback(mqtt_callback);
}

void loop() {
  if (WiFi.status() != WL_CONNECTED) {
    wifi_connect();
  }

  if (!mqtt.connected()) {
    mqtt_connect();
  }
  mqtt.loop();

  unsigned long now = millis();
  // refresh login hourly (mock access_token lifetime ~1h)
  if (now - lastLoginMs > 55UL * 60UL * 1000UL) {
    do_login();
  }
  if (mqtt.connected() && now - lastPublishMs > 5000) {
    lastPublishMs = now;
    mqtt_publish_telemetry();
  }
}
