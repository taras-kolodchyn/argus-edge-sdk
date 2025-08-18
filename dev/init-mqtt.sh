#!/bin/sh
set -euxo pipefail

echo "[init-mqtt] Ensure dirs"
mkdir -p /mosquitto/config /mosquitto/data /certs

echo "[init-mqtt] Write argus.conf"
cat >/mosquitto/config/argus.conf <<'CFG'
persistence true
persistence_location /mosquitto/data/
log_dest stdout

# Plain MQTT for dev (bind on all interfaces)
listener 1883 0.0.0.0
allow_anonymous false
password_file /mosquitto/config/passwords.txt

# TLS listener (bind on all interfaces)
listener 8883 0.0.0.0
cafile /certs/ca.crt
certfile /certs/server.crt
keyfile /certs/server.key
require_certificate false
CFG

echo "[init-mqtt] Ensure password_file directive exists"
grep -q 'password_file[[:space:]]\+/mosquitto/config/passwords.txt' /mosquitto/config/argus.conf || \
  printf "\npassword_file /mosquitto/config/passwords.txt\n" >> /mosquitto/config/argus.conf

echo "[init-mqtt] Install mosquitto-clients and openssl if needed"
if ! command -v mosquitto_passwd >/dev/null || ! command -v openssl >/dev/null; then
  apk add --no-cache mosquitto-clients openssl
fi

echo "[init-mqtt] Create/update passwords.txt"
touch /mosquitto/config/passwords.txt
# ensure correct owner/group before mosquitto_passwd (it warns if not root)
chown root:root /mosquitto/config/passwords.txt
chmod 0600 /mosquitto/config/passwords.txt
mosquitto_passwd -b /mosquitto/config/passwords.txt "${MQTT_USERNAME:-devuser}" "${MQTT_PASSWORD:-devpass}"
# enforce owner/group again in case mosquitto_passwd alters it
chown root:root /mosquitto/config/passwords.txt

echo "[init-mqtt] Generate self-signed TLS certs if absent"
if [ ! -s /certs/ca.crt ] || [ ! -s /certs/server.crt ] || [ ! -s /certs/server.key ]; then
  openssl genrsa -out /certs/ca.key 4096
  openssl req -x509 -new -key /certs/ca.key -sha256 -days 3650 -subj "/CN=Argus Dev CA" -out /certs/ca.crt

  openssl genrsa -out /certs/server.key 2048
  openssl req -new -key /certs/server.key -out /certs/server.csr -subj "/CN=${CERT_CN:-mqtt}"

  cat > /certs/server-ext.cnf <<'CNF'
basicConstraints=CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names
[alt_names]
DNS.1 = mqtt
DNS.2 = mosquitto
DNS.3 = localhost
IP.1 = 127.0.0.1
CNF

  openssl x509 -req -in /certs/server.csr -CA /certs/ca.crt -CAkey /certs/ca.key \
    -CAcreateserial -out /certs/server.crt -days 825 -sha256 -extfile /certs/server-ext.cnf
  chmod 0644 /certs/ca.crt /certs/server.crt
  chown mosquitto:mosquitto /certs/server.key
  chmod 0640 /certs/server.key
fi

echo "[init-mqtt] Done."