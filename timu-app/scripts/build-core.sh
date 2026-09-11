#!/usr/bin/env bash
# Build timu-core for iOS and generate everything the timu-core Expo module
# needs (ADR-012): UniFFI Swift bindings + the Rust static lib as an
# xcframework. Run from timu-app/ after changing timu-core:
#   ./scripts/build-core.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CORE="$ROOT/../timu-core"
MOD="$ROOT/modules/timu-core"
GEN="$MOD/ios/Generated"
FRAMEWORKS="$MOD/Frameworks"

cd "$CORE"

# Host dylib is the bindgen metadata source (platform-independent).
cargo build --features uniffi-cli
cargo run --features uniffi-cli --bin uniffi-bindgen -- generate \
  --library target/debug/libtimu_core.dylib \
  --language swift \
  --out-dir "$GEN"

for target in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
  if ! rustup target list --installed | grep -q "$target"; then
    rustup target add "$target"
  fi
  cargo build --release --features ffi --target "$target"
done

# Wrap each static lib in a static framework so Swift can import the
# `timu_coreFFI` module from the vendored xcframework (library-only
# xcframeworks don't expose module maps to CocoaPods pods).
# Simulator slice must contain both arches or CocoaPods' slice selection
# skips the copy and the app link fails.
lipo -create \
  "$CORE/target/aarch64-apple-ios-sim/release/libtimu_core.a" \
  "$CORE/target/x86_64-apple-ios/release/libtimu_core.a" \
  -output "$CORE/target/libtimu_core-sim-universal.a"

mkframework() {
  local lib=$1 out=$2
  local fw="$out/TimuCoreFFI.framework"
  rm -rf "$fw"
  mkdir -p "$fw/Headers"
  cp "$lib" "$fw/TimuCoreFFI"
  cp "$GEN/timu_coreFFI.h" "$fw/Headers/"
  cat > "$fw/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleIdentifier</key>
	<string>dev.timu.TimuCoreFFI</string>
	<key>CFBundleName</key>
	<string>TimuCoreFFI</string>
	<key>CFBundlePackageType</key>
	<string>FMWK</string>
	<key>CFBundleShortVersionString</key>
	<string>0.1.0</string>
	<key>CFBundleVersion</key>
	<string>1</string>
	<key>MinimumOSVersion</key>
	<string>16.4</string>
</dict>
</plist>
EOF
}

# Flat, platform-independent header dir: clang discovers module.modulemap
# on the include path, giving Swift the `timu_coreFFI` module regardless of
# which xcframework slice is linked (CocoaPods does not copy pod-internal
# xcframework slices, and Swift cannot read module maps from .xcframework
# bundles). `link` makes Swift emit the -framework flag for linking.
mkdir -p "$FRAMEWORKS/ffi-headers"
cp "$GEN/timu_coreFFI.h" "$FRAMEWORKS/ffi-headers/"
cat > "$FRAMEWORKS/ffi-headers/module.modulemap" <<'EOF'
module timu_coreFFI {
    umbrella header "timu_coreFFI.h"
    link "TimuCoreFFI"
    export *
}
EOF

mkframework "$CORE/target/aarch64-apple-ios/release/libtimu_core.a" "$FRAMEWORKS/device"
mkframework "$CORE/target/libtimu_core-sim-universal.a" "$FRAMEWORKS/simulator"

rm -rf "$FRAMEWORKS/TimuCoreFFI.xcframework"
xcodebuild -create-xcframework \
  -framework "$FRAMEWORKS/device/TimuCoreFFI.framework" \
  -framework "$FRAMEWORKS/simulator/TimuCoreFFI.framework" \
  -output "$FRAMEWORKS/TimuCoreFFI.xcframework"

echo "OK: $FRAMEWORKS/TimuCoreFFI.xcframework + $GEN/timu_core.swift"