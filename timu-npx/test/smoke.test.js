const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");

const pkgRoot = path.join(__dirname, "..");

test("smoke: npm pack + install + launcher invocation end-to-end (no network)", () => {
  // --- temp dirs -----------------------------------------------------------
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "timu-smoke-"));
  const installDir = path.join(work, "install");
  fs.mkdirSync(installDir);

  try {
    // --- 1. npm pack in timu-npx/ --------------------------------------------
    const pack = spawnSync("npm", ["pack"], {
      cwd: pkgRoot,
      encoding: "utf8",
    });
    assert.equal(pack.status, 0, `npm pack failed: ${pack.stderr}`);

    const tgzName = pack.stdout.trim().split("\n").pop().trim();
    assert.match(tgzName, /^timu-.*\.tgz$/);
    const tgzPath = path.join(pkgRoot, tgzName);
    assert.ok(fs.existsSync(tgzPath), `tarball not created at ${tgzPath}`);

    // --- 2. install the tarball in a temp dir (skip postinstall to avoid net)
    const install = spawnSync("npm", ["install", tgzPath, "--ignore-scripts"], {
      cwd: installDir,
      encoding: "utf8",
      env: { ...process.env },
    });
    assert.equal(install.status, 0, `npm install failed: ${install.stderr}`);

    // --- 3. verify the bin is installed and resolves -------------------------
    const installedBin = path.join(installDir, "node_modules", ".bin", "timu");
    assert.ok(
      fs.existsSync(installedBin),
      `timu bin not linked at ${installedBin}`,
    );

    // --- 4. create a fake native binary -------------------------------------
    const fake = path.join(work, "timu-pair");
    fs.writeFileSync(fake, "#!/bin/sh\necho SMOKE_OK $@\nexit 42\n", {
      mode: 0o755,
    });

    // --- 5. invoke the installed launcher via TIMU_PAIR_BINARY seam ---------
    const result = spawnSync(installedBin, ["--host", "smoke.example.com"], {
      encoding: "utf8",
      env: { ...process.env, TIMU_PAIR_BINARY: fake },
    });

    assert.equal(result.status, 42, `unexpected exit code. stderr: ${result.stderr}`);
    assert.equal(result.stdout, "SMOKE_OK --host smoke.example.com\n");

    // --- cleanup of tgz in package dir --------------------------------------
  } finally {
    // remove any .tgz produced by npm pack
    for (const f of fs.readdirSync(pkgRoot)) {
      if (/^timu-.*\.tgz$/.test(f)) {
        fs.rmSync(path.join(pkgRoot, f), { force: true });
      }
    }
    // remove the temp work directory
    fs.rmSync(work, { recursive: true, force: true });
  }
});