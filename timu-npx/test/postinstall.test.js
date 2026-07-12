const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const http = require("node:http");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");
const test = require("node:test");

const postinstall = path.join(__dirname, "..", "postinstall.js");

// Run postinstall.js as a child process with env overrides.
// Returns a Promise that resolves to { status, stderr, stdout }.
function runPostinstall(env) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [postinstall], {
      encoding: "utf8",
      env: { ...process.env, ...env },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => (stdout += d));
    child.stderr.on("data", (d) => (stderr += d));
    child.on("error", reject);
    child.on("close", (code) => {
      resolve({ status: code, stdout, stderr });
    });
    // Safety timeout — kill if it hangs
    setTimeout(() => {
      child.kill("SIGKILL");
      resolve({ status: null, stdout, stderr, timeout: true });
    }, 10000);
  });
}

// Simple HTTP server that serves files from a directory.
function createFileServer(dir) {
  return http.createServer((req, res) => {
    const filePath = path.join(dir, req.url);
    if (!fs.existsSync(filePath)) {
      res.writeHead(404);
      res.end("Not found");
      return;
    }
    const data = fs.readFileSync(filePath);
    res.writeHead(200, { "Content-Type": "application/octet-stream" });
    res.end(data);
  });
}

// Helper to start a server and get its port.
function startServer(server) {
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => resolve(server.address().port));
  });
}

// ── 1. Unsupported platform ─────────────────────────────────────────────────

test("unsupported platform exits nonzero with build-from-source guidance", async () => {
  const result = await runPostinstall({
    TIMU_POSTINSTALL_PLATFORM: "win32",
    TIMU_POSTINSTALL_ARCH: "x64",
  });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /build from source/i);
});

// ── 2. Download failure ──────────────────────────────────────────────────────

test("download failure leaves no partial file and exits nonzero", async () => {
  const binDir = fs.mkdtempSync(path.join(os.tmpdir(), "timu-postinstall-test-"));

  const server = http.createServer((_req, res) => {
    res.writeHead(404);
    res.end("Not found");
  });
  const port = await startServer(server);

  try {
    const result = await runPostinstall({
      TIMU_POSTINSTALL_PLATFORM: "linux",
      TIMU_POSTINSTALL_ARCH: "x64",
      TIMU_POSTINSTALL_URL_BASE: `http://127.0.0.1:${port}`,
      TIMU_POSTINSTALL_BIN_DIR: binDir,
    });

    assert.equal(result.status, 1, "should exit nonzero on download failure");

    const partialFile = path.join(binDir, "timu-pair");
    assert.equal(
      fs.existsSync(partialFile),
      false,
      "partial file should not exist after download failure",
    );
  } finally {
    server.close();
    fs.rmSync(binDir, { recursive: true, force: true });
  }
});

// ── 3. Checksum mismatch ─────────────────────────────────────────────────────

test("checksum mismatch: binary deleted and not made executable", async () => {
  const binDir = fs.mkdtempSync(path.join(os.tmpdir(), "timu-postinstall-test-"));
  const serverDir = fs.mkdtempSync(path.join(os.tmpdir(), "timu-postinstall-srv-"));

  const binaryContent = Buffer.from("fake binary content for mismatch test");
  fs.writeFileSync(path.join(serverDir, "timu-pair-linux-x64"), binaryContent);

  // WRONG checksum — hash of completely different content
  const wrongHash = crypto
    .createHash("sha256")
    .update("totally different content")
    .digest("hex");
  fs.writeFileSync(
    path.join(serverDir, "timu-pair-linux-x64.sha256"),
    `${wrongHash}  timu-pair-linux-x64\n`,
  );

  const server = createFileServer(serverDir);
  const port = await startServer(server);

  try {
    const result = await runPostinstall({
      TIMU_POSTINSTALL_PLATFORM: "linux",
      TIMU_POSTINSTALL_ARCH: "x64",
      TIMU_POSTINSTALL_URL_BASE: `http://127.0.0.1:${port}`,
      TIMU_POSTINSTALL_BIN_DIR: binDir,
    });

    assert.equal(result.status, 1, "should exit nonzero on checksum mismatch");

    const dest = path.join(binDir, "timu-pair");
    assert.equal(
      fs.existsSync(dest),
      false,
      "binary should be deleted on checksum mismatch",
    );
  } finally {
    server.close();
    fs.rmSync(binDir, { recursive: true, force: true });
    fs.rmSync(serverDir, { recursive: true, force: true });
  }
});

// ── 4. Checksum success ──────────────────────────────────────────────────────

test("checksum success: binary made executable only after verification", async () => {
  const binDir = fs.mkdtempSync(path.join(os.tmpdir(), "timu-postinstall-test-"));
  const serverDir = fs.mkdtempSync(path.join(os.tmpdir(), "timu-postinstall-srv-"));

  const binaryContent = Buffer.from("real binary content for success test");
  fs.writeFileSync(path.join(serverDir, "timu-pair-linux-x64"), binaryContent);

  // CORRECT checksum
  const correctHash = crypto
    .createHash("sha256")
    .update(binaryContent)
    .digest("hex");
  fs.writeFileSync(
    path.join(serverDir, "timu-pair-linux-x64.sha256"),
    `${correctHash}  timu-pair-linux-x64\n`,
  );

  const server = createFileServer(serverDir);
  const port = await startServer(server);

  try {
    const result = await runPostinstall({
      TIMU_POSTINSTALL_PLATFORM: "linux",
      TIMU_POSTINSTALL_ARCH: "x64",
      TIMU_POSTINSTALL_URL_BASE: `http://127.0.0.1:${port}`,
      TIMU_POSTINSTALL_BIN_DIR: binDir,
    });

    assert.equal(result.status, 0, "should exit 0 on checksum success");

    const dest = path.join(binDir, "timu-pair");
    assert.equal(fs.existsSync(dest), true, "binary should exist");

    const stat = fs.statSync(dest);
    const mode = stat.mode & 0o777;
    assert.equal(mode, 0o755, "binary should be executable (0o755)");
  } finally {
    server.close();
    fs.rmSync(binDir, { recursive: true, force: true });
    fs.rmSync(serverDir, { recursive: true, force: true });
  }
});