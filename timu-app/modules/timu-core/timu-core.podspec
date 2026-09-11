Pod::Spec.new do |s|
  s.name           = 'timu-core'
  s.version        = '0.1.0'
  s.summary        = 'timu-core Rust engine for the timu app (SSH + tmux, ADR-012)'
  s.description    = 'Expo module exposing the timu-core UniFFI bindings: SSH connect, readiness, tmux sessions, chat, live pane streaming, and the SQLite store.'
  s.author         = 'timu'
  s.homepage       = 'https://github.com/ritvij14/timu'
  s.platforms      = { :ios => '16.4' }
  s.source         = { git: '' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  # Rust static lib (device + simulator), built by scripts/build-core.sh.
  # The xcframework provides linking; the `timu_coreFFI` C module resolves
  # through the flat ffi-headers module map on the header search path.
  s.vendored_frameworks = 'Frameworks/TimuCoreFFI.xcframework'

  # Hand-written wrapper + generated UniFFI Swift bindings.
  # The Generated/ header dirs are xcframework build inputs only, not sources.
  s.source_files = 'ios/TimuCoreModule.swift', 'ios/Generated/timu_core.swift'

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'HEADER_SEARCH_PATHS' => '"$(inherited)" "${PODS_ROOT}/../../modules/timu-core/Frameworks/ffi-headers"'
  }
end