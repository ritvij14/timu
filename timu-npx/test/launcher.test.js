const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");

const launcher = path.join(__dirname, "..", "bin", "timu.js");

test("npx launcher forwards arguments and the native exit code", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "timu-npx-test-"));
  const fake = path.join(root, "timu-pair");
  fs.writeFileSync(fake, "#!/bin/sh\nprintf '%s\\n' \"$@\"\nexit 7\n", { mode: 0o755 });

  const result = spawnSync(process.execPath, [launcher, "--host", "dev.example.com"], {
    encoding: "utf8",
    env: { ...process.env, TIMU_PAIR_BINARY: fake },
  });

  assert.equal(result.status, 7);
  assert.equal(result.stdout, "--host\ndev.example.com\n");
  fs.rmSync(root, { recursive: true, force: true });
});

test("npx launcher tells the user how to recover when the native binary is missing", () => {
  const missing = path.join(os.tmpdir(), "timu-binary-that-does-not-exist");

  const result = spawnSync(process.execPath, [launcher], {
    encoding: "utf8",
    env: { ...process.env, TIMU_PAIR_BINARY: missing },
  });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /timu-pair binary not found/);
  assert.match(result.stderr, /cargo build --release/);
});
