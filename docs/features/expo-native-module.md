# timu-app native module (timu-core bridge)

> How the Expo app talks to the Rust engine over FFI. Extends
> `docs/features/timu-core.md` (ADR-012); PRD §6–§13 flows.

---

## 1. What this owns

- `modules/timu-core/` — local Expo module (iOS-only for now) exposing the full
  timu-core FFI surface to JS: connection test, connect, readiness, tmux
  session lifecycle, chat send, pane streaming (module events), and the SQLite
  store.
- `scripts/build-core.sh` — generates everything the module needs from
  `timu-core`: UniFFI Swift bindings + a static-lib xcframework
  (device arm64, simulator arm64+x86_64). All Rust artifacts are generated at
  build time; none are committed.

**Flow:** `npx expo prebuild` → `./scripts/build-core.sh` → `npx pod-install`
(rerun build-core.sh after any timu-core change).

## 2. Files

| File | Owns |
| --- | --- |
| `modules/timu-core/ios/TimuCoreModule.swift` | Expo Module: `FfiCore`/`Connection`/`Store`/`PaneStreamHandle` shared objects, `PaneEventSink` → module events, record/DTO conversions, `TIMU_*` error rejection |
| `modules/timu-core/src/TimuCoreModule.ts` | Typed JS adapter: `TimuCore`, `NativeConnection`, `PaneStreamHandle`, `NativeStore`, `TimuNativeError` (strips `TIMU_` prefix) |
| `modules/timu-core/timu-core.podspec` | Vendored xcframework + bindings source list + ffi-headers include path |
| `modules/timu-core/ios/Generated/` | UniFFI Swift bindings (generated, gitignored) |
| `modules/timu-core/Frameworks/` | xcframework + ffi-headers module map (generated, gitignored) |
| `plugins/expo-timu-core-podfile-patch.js` | Config plugin: Xcode 26.2 Swift-concurrency workaround for expo-modules-core (remove once Xcode ≥ 26.4) |

## 3. Build mechanics that matter

- **Bindings source:** `uniffi-bindgen generate --library` runs against a *host*
  dylib (metadata is platform-independent); the iOS static libs come from
  `cargo build --features ffi --target <triple>`.
- **FFI module resolution:** Swift's `import timu_coreFFI` resolves via a flat,
  platform-independent `Frameworks/ffi-headers/module.modulemap` on the pod's
  header search path — Swift cannot read module maps from inside
  `.xcframework` bundles, and CocoaPods does not copy pod-internal xcframework
  slices. The xcframework exists for linking only; the modulemap's
  `link "TimuCoreFFI"` makes Swift emit the `-framework` flag.
- **Simulator slice** is a universal arm64+x86_64 archive (`lipo`); a
  single-arch slice makes CocoaPods' slice selection skip the copy and the app
  link fails.
- **Shared objects:** `Connection`/`Store`/`PaneStreamHandle` are Expo
  `SharedObject`s returned from module functions; methods are bound on the
  object (no flat module functions).
- **Streaming:** `startPaneStream` bridges UniFFI callbacks to module events
  `onPaneEvent` / `onPaneError`, tagged with the tmux session id; the JS
  adapter filters per session and cleans up on `stop()`.

## 4. Known constraints

- **Xcode 26.2:** Expo SDK 57 requires Xcode 26.4+ (expo#47539). Until Xcode
  is upgraded, the config plugin pins ExpoModulesCore to Swift 5 language mode
  + minimal concurrency checks. Delete the plugin after upgrading Xcode.
- **Android:** not wired yet — needs NDK + 4 cross targets + a Kotlin twin of
  `TimuCoreModule` behind the same JS adapter.

## 5. Verified

`npx expo prebuild --clean` → `pod-install` → `xcodebuild ... -sdk
iphonesimulator build` succeeds; Rust FFI symbols are present in the linked
app binary; `tsc --noEmit` clean. Runtime behavior (connect → readiness →
session → streaming) is next: requires a device/simulator run against a real
SSH host.