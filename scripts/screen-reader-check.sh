#!/usr/bin/env bash
# Reads and uses the `interface` example's window as a screen reader does, through AT-SPI
# (docs/adr/0146-screen-readers.md): on a session bus of D-Bus of its own, which starts no service
# by itself, it starts the AT-SPI service, turns it on, runs the example on the Wayland display that
# the environment names, and has scripts/screen-reader-check.py read and use it. scripts/check.sh
# runs it in a headless Weston.
#
# Usage: scripts/screen-reader-check.sh <the example's program> <cooked data>
#
# It needs the Nix shell's AT-SPI (`SLIME_AT_SPI`), D-Bus, Python with PyGObject and
# `GI_TYPELIB_PATH`.
set -euo pipefail

if [ -z "${SLIME_SCREEN_READER_BUS:-}" ]; then
    # A bus of its own, which activates no service: the desktop's portals, which winit's
    # decorations ask for their theme, stay out of the check.
    bus=$(mktemp -d)
    cat >"$bus/session.conf" <<CONF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=$bus</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
CONF
    status=0
    SLIME_SCREEN_READER_BUS=$bus dbus-run-session --config-file="$bus/session.conf" -- "$0" "$@" ||
        status=$?
    rm -rf "$bus"
    exit "$status"
fi

example=$1
assets=$2
here=$(dirname "$0")
logs=$SLIME_SCREEN_READER_BUS
pids=()
trap 'kill "${pids[@]}" 2>/dev/null || true' EXIT

# The AT-SPI bus and its registry, which a desktop's session would start; then AT-SPI turned on, as
# a screen reader turns it on, which AccessKit's adapter waits for. The bus keeps that setting in
# GSettings, whose memory alone holds it here: dconf, which a desktop's session runs, is not.
GSETTINGS_BACKEND=memory "$SLIME_AT_SPI/libexec/at-spi-bus-launcher" --launch-immediately \
    >"$logs/bus.log" 2>&1 &
pids+=("$!")
for _ in $(seq 100); do
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.a11y.Bus.GetAddress >/dev/null 2>&1 && break
    sleep 0.1
done
"$SLIME_AT_SPI/libexec/at-spi2-registryd" >"$logs/registry.log" 2>&1 &
pids+=("$!")
# The registry must answer on the AT-SPI bus before the example announces itself to it.
address=$(gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.a11y.Bus.GetAddress | sed "s/^('\(.*\)',)$/\1/")
for _ in $(seq 300); do
    gdbus call --address "$address" --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
        --method org.freedesktop.DBus.NameHasOwner org.a11y.atspi.Registry 2>/dev/null |
        grep -q true && break
    sleep 0.1
done
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>' >/dev/null
if ! gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.freedesktop.DBus.Properties.Get org.a11y.Status IsEnabled | grep -q true; then
    echo "AT-SPI did not turn on:"
    cat "$logs/bus.log"
    exit 1
fi

"$example" --assets "$assets" >"$logs/example.log" 2>&1 &
example_pid=$!
pids+=("$example_pid")
if ! python3 "$here/screen-reader-check.py"; then
    echo "the example's log:"
    cat "$logs/example.log"
    echo "the AT-SPI bus's log:"
    cat "$logs/bus.log"
    echo "the AT-SPI registry's log:"
    cat "$logs/registry.log"
    echo "AT-SPI's state:"
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.GetAll org.a11y.Status || true
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.a11y.Bus.GetAddress || true
    for pid in "${pids[@]}"; do
        kill -0 "$pid" 2>/dev/null && echo "$pid runs" || echo "$pid has stopped"
    done
    exit 1
fi
# The example stops as a termination signal asks, cleanly.
kill -TERM "$example_pid"
wait "$example_pid"
