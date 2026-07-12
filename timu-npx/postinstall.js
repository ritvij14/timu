// postinstall.js — downloads the pre-built timu-pair binary for the
// current platform from GitHub releases.
//
// Binary naming convention:
//   timu-pair-{platform}-{arch}
//   e.g. timu-pair-linux-x64, timu-pair-darwin-arm64
//
// These are published as release assets on GitHub. The script fetches
// the one matching the host platform and saves it to bin/timu-pair.

const https = require("https");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const REPO = "ritvij14/timu";
const VERSION = "0.1.0"; // kept in sync with package.json + Cargo.toml + release.yml

// Map process.platform + process.arch to the release asset name.
function platformAsset() {
  const { platform, arch } = process;

  const map = {
    "linux-x64": "timu-pair-linux-x64",
    "linux-arm64": "timu-pair-linux-arm64",
    "darwin-x64": "timu-pair-darwin-x64",
    "darwin-arm64": "timu-pair-darwin-arm64",
  };

  const key = `${platform}-${arch}`;
  const asset = map[key];

  if (!asset) {
    console.warn(`timu: no pre-built binary for ${key}.`);
    console.warn(`timu: build from source with: cd timu-pair && cargo build --release`);
    console.warn(`timu: then copy target/release/timu-pair to timu-npx/bin/`);
    process.exit(0); // not a hard failure — user can build manually
  }

  return asset;
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);

    function handleResponse(res) {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        // follow redirect
        file.close();
        fs.unlinkSync(dest);
        return download(res.headers.location, dest).then(resolve).catch(reject);
      }
      if (res.statusCode !== 200) {
        file.close();
        fs.unlinkSync(dest);
        return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
      res.pipe(file);
      file.on("finish", () => {
        file.close();
        resolve();
      });
    }

    https.get(url, handleResponse).on("error", (err) => {
      file.close();
      if (fs.existsSync(dest)) fs.unlinkSync(dest);
      reject(err);
    });
  });
}

async function main() {
  const asset = platformAsset();
  const binDir = path.join(__dirname, "bin");
  const dest = path.join(binDir, "timu-pair");

  // If binary already exists (e.g. dev mode), skip download.
  if (fs.existsSync(dest)) {
    console.log("timu: binary already present, skipping download.");
    return;
  }

  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${asset}`;
  console.log(`timu: downloading ${asset} from GitHub releases...`);

  try {
    await download(url, dest);
    fs.chmodSync(dest, 0o755);
    console.log("timu: installed successfully.");
  } catch (err) {
    console.warn(`timu: download failed — ${err.message}`);
    console.warn("timu: you can build from source: cd timu-pair && cargo build --release");
    console.warn("timu: then copy target/release/timu-pair to timu-npx/bin/");
    process.exit(0); // don't fail npm install for users who can build
  }
}

main();