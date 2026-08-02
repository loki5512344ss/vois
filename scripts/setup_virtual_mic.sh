#!/usr/bin/env bash
# Create the "vois" null sink and expose its monitor as a virtual mic.
# The default sink is left untouched: vois routes only its own playback
# stream to the "vois" sink, so other apps keep playing to your real output.
set -eu

SINK_NAME="vois.rs"
SINK_DESC="vois.rs virtual mic"
REMAP_NAME="vois.rs"

CONF_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/wireplumber/wireplumber.conf.d"
CONF_FILE="${CONF_DIR}/51-vois.conf"

# WirePlumber remembers stream targets and restores them, so streams that once
# played to the speakers keep going there even after the default sink changes.
# Disable that so the app's output follows the default (vois.rs).
if [ ! -f "${CONF_FILE}" ] || ! grep -q "restore-target = false" "${CONF_FILE}" 2>/dev/null; then
    mkdir -p "${CONF_DIR}"
    cat > "${CONF_FILE}" <<EOF
wireplumber.settings = {
  node.stream.restore-target = false
  node.stream.restore-props = false
}
EOF
    echo "installed WirePlumber fix (${CONF_FILE})"
    systemctl --user restart wireplumber || true
    echo "restarted WirePlumber"
else
    echo "WirePlumber fix already present"
fi

# Idempotency check: field 2 of `pactl list short <x>` is the object name.
if ! pactl list short sinks | awk -v n="${SINK_NAME}" '$2==n' | grep -q .; then
    pactl load-module module-null-sink \
        sink_name="${SINK_NAME}" \
        sink_properties="device.description=${SINK_DESC}"
    echo "created sink: ${SINK_NAME}"
else
    echo "sink already present: ${SINK_NAME}"
fi

if ! pactl list short sources | awk -v n="${REMAP_NAME}" '$2==n' | grep -q .; then
    pactl load-module module-remap-source \
        source_name="${REMAP_NAME}" \
        master="${SINK_NAME}.monitor"
    echo "created remapped source: ${REMAP_NAME}"
else
    echo "remapped source already present: ${REMAP_NAME}"
fi

echo "virtual mic source name(s): ${SINK_NAME}.monitor and ${REMAP_NAME}"
echo 'use "vois.rs" (or "vois.rs.monitor") as your microphone in apps'
