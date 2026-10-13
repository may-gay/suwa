#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${SUWA_TEST_SERVER_JAR:?set SUWA_TEST_SERVER_JAR to an official Suwayomi server jar}"
SUWA_TEST_JAVA="${SUWA_TEST_JAVA:-/usr/lib/jvm/java-21-openjdk/bin/java}"
suwa_fixture_dir=$(mktemp -d /tmp/suwa-integration.XXXXXX)
suwa_fixture_pid=''
suwa_fixture_port=$(python3 - <<'PYPORT'
import socket
with socket.socket() as listener:
    listener.bind(('127.0.0.1', 0))
    print(listener.getsockname()[1])
PYPORT
)
export SUWA_TEST_SERVER_URL="http://127.0.0.1:$suwa_fixture_port"
cleanup() {
    if [[ -n "$suwa_fixture_pid" ]]; then
        kill "$suwa_fixture_pid" 2>/dev/null || true
        wait "$suwa_fixture_pid" 2>/dev/null || true
    fi
    printf 'Fixture data and logs: %s\n' "$suwa_fixture_dir"
}
trap cleanup EXIT
cat > "$suwa_fixture_dir/server.conf" <<CONF
server.ip = "127.0.0.1"
server.port = $suwa_fixture_port
server.initialOpenInBrowserEnabled = false
server.systemTrayEnabled = false
server.webUIEnabled = false
server.webUIUpdateCheckInterval = 0.0
server.kcefEnabled = false
server.backupInterval = 0
server.globalUpdateInterval = 0.0
server.extensionStores = []
CONF
python3 scripts/create-fixtures.py "$suwa_fixture_dir/local"
"$SUWA_TEST_JAVA" "-Dsuwayomi.tachidesk.config.server.rootDir=$suwa_fixture_dir" -jar "$SUWA_TEST_SERVER_JAR" > "$suwa_fixture_dir/server.log" 2>&1 &
suwa_fixture_pid=$!
for ((attempt=0; attempt<60; attempt++)); do
    kill -0 "$suwa_fixture_pid" 2>/dev/null || { cat "$suwa_fixture_dir/server.log"; exit 1; }
    if curl --silent --fail --max-time 1 -H 'Content-Type: application/json' --data '{"query":"query{aboutServer{version}}"}' "$SUWA_TEST_SERVER_URL/api/graphql" > /dev/null; then
        cargo test --all-features isolated_suwayomi_roundtrip -- --ignored --nocapture
        exit $?
    fi
    sleep 1
done
cat "$suwa_fixture_dir/server.log"
exit 1
