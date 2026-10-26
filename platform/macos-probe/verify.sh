#!/bin/sh
set -eu
cd "$(dirname "$0")"
probe_tmp=$(mktemp -d "${TMPDIR:-/tmp}/tavra-macos-probe.XXXXXX")
trap 'rm -rf "$probe_tmp"' EXIT HUP INT TERM
xcrun swiftc -target arm64-apple-macosx13.0 -framework NetworkExtension Sources/Probe.swift Tests/main.swift -o "$probe_tmp/fixtures"
"$probe_tmp/fixtures"
for template in Templates/*.plist Templates/*.entitlements; do plutil -lint "$template"; done
