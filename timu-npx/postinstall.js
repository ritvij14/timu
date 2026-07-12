// postinstall.js — downloads the pre-built timu-pair binary for the
// current platform from GitHub releases.
//
// Binary naming convention:
//   timu-pair-{platform}-{arch}
//   e.g. timu-pair-linux-x64, timu-pair-darwin-arm64
//
// These are published as release assets on GitHub. The script fetches
// the one matching the host platform, verifies its SHA-256 checksum,
// and saves it to bin/timu-pair.
//
// Test seams (env vars, not for end-user use):
//   TIMU_POSTINSTALL_PLATFORM  — override process.platform
//   TIMU_POSTINSTALL_ARCH     — override process.arch
//   TIMU_POSTINSTALL_URL_BASE — override the GitHub release URL prefix
//   TIMU_POSTINSTALL_BIN_DIR  — override the bin/ destination directory

const http = require("http");
const https = require("https");
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");

const REPO = "ritvij14/timu";
const VERSION = "0.1.1"; // kept in sync with package.json + Cargo.toml + release.yml

// Map platform + arch to the release asset name.
function platformAsset() {
  const platform = process.env.TIMU_POSTINSTALL_PLATFORM || process.platform;
  const arch = process.env.TIMU_POSTINSTALL_ARCH || process.arch;

  const map = {
    "linux-x64": "timu-pair-linux-x64",
    "linux-arm64": "timu-pair-linux-arm64",
    "darwin-x64": "timu-pair-darwin-x64",
    "darwin-arm64": "timu-pair-darwin-arm64",
  };

  const key = `${platform}-${arch}`;
  const asset = map[key];

  if (!asset) {
    console.error(`timu: no pre-built binary for ${key}.`);
    console.error(`timu: build from source with: cd timu-pair && cargo build --release`);
    console.error(`timu: then copy target/release/timu-pair to timu-npx/bin/`);
    process.exit(1); // hard failure — user must build from source
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

    const client = url.startsWith("https") ? https : http;
    client.get(url, handleResponse).on("error", (err) => {
      file.close();
      if (fs.existsSync(dest)) fs.unlinkSync(dest);
      reject(err);
    });
  });
}

// Compute SHA-256 of a file, return lowercase hex string.
function sha256File(filePath) {
  const data = fs.readFileSync(filePath);
  return crypto.createHash("sha256").update(data).digest("hex");
}

// Parse the .sha256 file content: "<hash>  <filename>" → just the hash.
function parseChecksum(content) {
  const hash = content.trim().split(/\s+/)[0].toLowerCase();
  return hash;
}

async function main() {
  const asset = platformAsset();
  const binDir = process.env.TIMU_POSTINSTALL_BIN_DIR || path.join(__dirname, "bin");
  const dest = path.join(binDir, "timu-pair");

  // If binary already exists (e.g. dev mode), skip download.
  if (fs.existsSync(dest)) {
    console.log("timu: binary already present, skipping download.");
    process.exit(0);
  }

  const urlBase =
    process.env.TIMU_POSTINSTALL_URL_BASE ||
    `https://github.com/${REPO}/releases/download/v${VERSION}`;
  const binaryUrl = `${urlBase}/${asset}`;
  const checksumUrl = `${binaryUrl}.sha256`;

  console.log(`timu: downloading ${asset} ...`);

  try {
    // Download the binary.
    await download(binaryUrl, dest);

    // Download the checksum file.
    let checksumContent;
    try {
      const checksumDest = dest + ".sha256.tmp";
      await download(checksumUrl, checksumDest);
      checksumContent = fs.readFileSync(checksumDest, "utf8");
      fs.unlinkSync(checksumDest);
    } catch (err) {
      if (fs.existsSync(dest)) fs.unlinkSync(dest);
      console.error(`timu: checksum download failed — ${err.message}`);
      console.error("timu: you can build from source: cd timu-pair && cargo build --release");
      console.error("timu: then copy target/release/timu-pair to timu-npx/bin/");
      process.exit(1);
    }

    // Verify checksum.
    const expected = parseChecksum(checksumContent);
    const actual = sha256File(dest);

    if (expected !== actual) {
      fs.unlinkSync(dest);
      console.error(`timu: checksum mismatch for ${asset}`);
      console.error(`timu: expected ${expected}`);
      console.error(`timu: got      ${actual}`);
      console.error("timu: you can build from source: cd timu-pair && cargo build --release");
      console.error("timu: then copy target/release/timu-pair to timu-npx/bin/");
      process.exit(1);
    }

    // Checksum verified — make executable.
    fs.chmodSync(dest, 0o755);
    console.log("timu: installed successfully.");
    process.exit(0);
  } catch (err) {
    // Download failure — clean up any partial file.
    if (fs.existsSync(dest)) fs.unlinkSync(dest);
    console.error(`timu: download failed — ${err.message}`);
    console.error("timu: you can build from source: cd timu-pair && cargo build --release");
    console.error("timu: then copy target/release/timu-pair to timu-npx/bin/");
    process.exit(1);
  }
}

main();