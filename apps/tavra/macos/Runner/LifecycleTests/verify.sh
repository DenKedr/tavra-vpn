#!/bin/sh
set -eu
runner_dir=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
: "${FLUTTER_MACOS_FRAMEWORK_DIR:?set directory containing FlutterMacOS.framework}"
probe_dir=$(mktemp -d "${TMPDIR:-/tmp}/tavra-native-lifecycle.XXXXXX")
trap 'python3 -c "import shutil,sys; shutil.rmtree(sys.argv[1])" "$probe_dir"' EXIT HUP INT TERM
# The test harness replaces application @main only; production sources are unchanged.
sed '/^@main$/d' "$runner_dir/AppDelegate.swift" > "$probe_dir/AppDelegate.swift"
cat > "$probe_dir/RegistryStub.swift" <<'STUB'
import FlutterMacOS
func RegisterGeneratedPlugins(registry: FlutterPluginRegistry) {}
STUB
xcrun swiftc -target arm64-apple-macosx13.0 -F "$FLUTTER_MACOS_FRAMEWORK_DIR" \
  -Xlinker -rpath -Xlinker "$FLUTTER_MACOS_FRAMEWORK_DIR" \
  "$probe_dir/AppDelegate.swift" "$runner_dir/MainFlutterWindow.swift" \
  "$probe_dir/RegistryStub.swift" "$runner_dir/LifecycleTests/main.swift" -o "$probe_dir/fixtures"
"$probe_dir/fixtures"
