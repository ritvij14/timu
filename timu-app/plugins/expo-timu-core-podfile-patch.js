// Expo config plugin: patches the generated Podfile so expo-modules-core
// compiles under Xcode 26.2. Expo SDK 57 requires Xcode 26.4+ (Swift 6.3);
// under older Xcode the ExpoModulesCore Swift 6 source fails sendability
// checks ("sending 'emitter' risks causing data races"). Building that pod in
// Swift 5 language mode with minimal concurrency checks avoids the
// diagnostics. Remove this plugin once the project builds against Xcode 26.4+.
const { withPodfile } = require("@expo/config-plugins");

const PATCH_MARKER = "SWIFT_STRICT_CONCURRENCY";

const PATCH = `
    # Xcode 26.2 workaround (see plugins/expo-timu-core-podfile-patch.js).
    installer.pods_project.targets.each do |target|
      target.build_configurations.each do |config|
        config.build_settings['SWIFT_STRICT_CONCURRENCY'] = 'minimal'
      end
      if target.name == 'ExpoModulesCore'
        target.build_configurations.each do |config|
          config.build_settings['SWIFT_VERSION'] = '5.0'
        end
      end
    end
`;

const withTimuCorePodfilePatch = (config) => {
  return withPodfile(config, (podfileConfig) => {
    const contents = podfileConfig.modResults.contents;
    if (contents.includes(PATCH_MARKER)) {
      return podfileConfig;
    }
    // Anchor on the end of the generated post_install block.
    const anchor = "    )\n  end\nend";
    if (!contents.includes(anchor)) {
      throw new Error(
        "timu-core Podfile patch failed: post_install anchor not found in generated Podfile"
      );
    }
    podfileConfig.modResults.contents = contents.replace(
      anchor,
      `    )\n${PATCH}  end\nend`
    );
    return podfileConfig;
  });
};

module.exports = withTimuCorePodfilePatch;