#!/usr/bin/env node

// timu — npx entry point.
// Runs the timu-pair Rust binary which detects the local network IP,
// current username, and SSH port, then renders a QR code in the terminal.
//
// Usage: npx timu [--host <ip>] [--user <name>] [--port <port>]

const { spawn } = require("child_process");
const path = require("path");

// The Rust binary lives next to this script after `npm install`.
// In dev/test mode, it falls back to a pre-built path.
const binPath = path.join(__dirname, "timu-pair");

const child = spawn(binPath, process.argv.slice(2), { stdio: "inherit" });

child.on("error", (err) => {
  if (err.code === "ENOENT") {
    console.error("timu-pair binary not found at " + binPath);
    console.error("If you're running from source, build it first:");
    console.error("  cd timu-pair && cargo build --release");
    console.error("  cp target/release/timu-pair ../timu-npx/bin/");
    process.exit(1);
  }
  console.error(err);
  process.exit(1);
});

child.on("exit", (code) => process.exit(code || 0));