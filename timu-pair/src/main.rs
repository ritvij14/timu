use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use qrcode::QrCode;
use qrcode::render::unicode;
use timu_pair::{
    CleanupGuard, CliOptions, CommandOutput, PairingPayload, System, append_authorized_key_line,
    build_temporary_authorized_key, choose_address, discover_addresses, ensure_ssh_available,
    is_expired, pairing_id_from_random_bytes, reject_unsafe_authorized_keys_path,
    replace_temporary_authorized_key_in_file, wait_for_completion,
};

struct RealSystem;

fn main() {
    if let Err(error) = run() {
        eprintln!("timu: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--complete") {
        return complete_pairing(&args);
    }
    let options = CliOptions::parse(args).map_err(|error| error.to_string())?;
    let system = RealSystem;
    ensure_ssh_available(&system, options.port)?;
    let host = match options.host {
        Some(host) => host,
        None => choose_address(discover_addresses(&system)?, |question| {
            system.prompt(question)
        })?,
    };
    let username = options
        .username
        .or_else(|| env::var("USER").ok())
        .filter(|value| !value.is_empty())
        .ok_or("could not determine SSH username; use --user")?;
    prepare_pairing(&system, host, username, options.port)
}

fn complete_pairing(args: &[String]) -> Result<(), String> {
    if args.len() != 5 {
        return Err("invalid completion invocation".into());
    }
    let expires_at = args[4].parse::<u64>().map_err(|_| "invalid expiry")?;
    if is_expired(unix_now()?, expires_at) {
        return Err("pairing credential has expired".into());
    }
    let authorized_keys = Path::new(&args[2]);
    let mut input = String::new();
    io::stdin()
        .take(8193)
        .read_to_string(&mut input)
        .map_err(|_| "could not read device public key")?;
    if input.len() > 8192 {
        return Err("device public key is too large".into());
    }
    replace_temporary_authorized_key_in_file(authorized_keys, &args[1], input.trim())
        .map_err(|error| error.to_string())?;
    fs::write(&args[3], b"paired\n").map_err(|_| "could not signal completion")?;
    Ok(())
}

fn prepare_pairing(
    system: &dyn System,
    host: String,
    username: String,
    port: u16,
) -> Result<(), String> {
    let now = system.now();
    let expires_at = now + 300;
    let pairing_id = new_pairing_id()?;
    let root = env::temp_dir().join(format!("timu-pair-{pairing_id}"));
    fs::create_dir(&root).map_err(|_| "could not create pairing directory")?;
    // The root owns every local ceremony artifact. Register its cleanup before
    // any later setup operation can fail or the ceremony can be cancelled.
    let mut cleanup = CleanupGuard::new(root.clone(), pairing_id.clone());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .map_err(|_| "could not secure pairing directory")?;
    let private_key = root.join("ephemeral");
    run_status(
        Command::new("ssh-keygen")
            .args([
                "-q",
                "-t",
                "ed25519",
                "-N",
                "",
                "-C",
                &format!("timu-pair:{pairing_id}"),
                "-f",
            ])
            .arg(&private_key),
        "generate pairing key",
    )?;
    let public_key = fs::read_to_string(private_key.with_extension("pub"))
        .map_err(|_| "could not read the pairing public key")?;
    let fingerprint = host_fingerprint()?;
    let machine_name = machine_hostname()?;
    let authorized_keys = authorized_keys_path()?;
    ensure_authorized_keys(&authorized_keys)?;
    let done = root.join("done");
    let helper = root.join("install-device-key");
    let executable = env::current_exe().map_err(|_| "could not determine executable path")?;
    let script = format!(
        "#!/bin/sh\nexec '{}' --complete '{}' '{}' '{}' '{}'\n",
        shell_path(&executable)?,
        pairing_id,
        shell_path(&authorized_keys)?,
        shell_path(&done)?,
        expires_at
    );
    fs::write(&helper, script).map_err(|_| "could not write pairing helper")?;
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700))
        .map_err(|_| "could not secure pairing helper")?;
    let temporary_line = build_temporary_authorized_key(
        &pairing_id,
        helper.to_str().ok_or("invalid helper path")?,
        public_key.trim(),
    )
    .map_err(|e| e.to_string())?;
    append_authorized_key_line(&authorized_keys, &temporary_line)
        .map_err(|error| error.to_string())?;
    // Register immediately after the append so every later failure, including
    // signal-handler installation, removes only this ceremony's tagged line.
    cleanup.register_authorization(authorized_keys.clone());
    let private_key_text =
        fs::read_to_string(&private_key).map_err(|_| "could not read the pairing key")?;
    let payload = PairingPayload {
        version: 1,
        pairing_id,
        machine_name,
        host: host.clone(),
        port,
        username: username.clone(),
        host_key_fingerprint: fingerprint.clone(),
        expires_at_unix: expires_at,
        ephemeral_private_key: private_key_text,
    };
    let qr = payload.encode_for_qr().map_err(|e| e.to_string())?;
    println!("\nPairing address: {username}@{host}:{port}");
    println!("SSH host key:   {fingerprint}");
    println!("Expires in:     5 minutes\n");
    println!("Scan this QR with the timu app:\n");
    print_qr(&qr)?;
    println!("\nWaiting for your iPhone to finish pairing…");
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&cancelled);
    ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst))
        .map_err(|_| "could not install signal handler")?;
    wait_for_completion(system, &done, expires_at, &cancelled)?;
    println!("Paired successfully. You can return to the timu app.");
    drop(cleanup);
    Ok(())
}

fn new_pairing_id() -> Result<String, String> {
    let mut bytes = [0; 16];
    fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| "could not generate pairing id")?;
    Ok(pairing_id_from_random_bytes(bytes))
}

impl System for RealSystem {
    fn family(&self) -> &'static str {
        env::consts::OS
    }

    fn command(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::inherit())
            .output()
            .map_err(|e| e.to_string())?;
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            status: output.status.code().unwrap_or(-1),
        })
    }

    fn prompt(&self, question: &str) -> Result<String, String> {
        print!("{question}");
        io::stdout().flush().map_err(|e| e.to_string())?;
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|e| e.to_string())?;
        Ok(answer)
    }

    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
    }

    fn tcp_reachable(&self, host: &str, port: u16, timeout: Duration) -> bool {
        format!("{host}:{port}")
            .parse()
            .ok()
            .and_then(|address| TcpStream::connect_timeout(&address, timeout).ok())
            .is_some()
    }

    fn route_address(&self) -> Option<String> {
        let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect("8.8.8.8:80").ok()?;
        Some(socket.local_addr().ok()?.ip().to_string())
    }

    fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }
}

fn authorized_keys_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".ssh/authorized_keys"))
}
fn ensure_authorized_keys(path: &Path) -> Result<(), String> {
    let dir = path.parent().ok_or("invalid authorized_keys path")?;
    reject_unsafe_authorized_keys_path(path)
        .map_err(|_| "refusing unsafe .ssh directory or authorized_keys")?;
    fs::create_dir_all(dir).map_err(|_| "could not create .ssh directory")?;
    reject_unsafe_authorized_keys_path(path)
        .map_err(|_| "refusing unsafe .ssh directory or authorized_keys")?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
        .map_err(|_| "could not secure .ssh directory")?;
    if !path.exists() {
        fs::write(path, b"").map_err(|_| "could not create authorized_keys")?;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| "could not secure authorized_keys".to_string())
}
fn machine_hostname() -> Result<String, String> {
    let output = Command::new("hostname")
        .output()
        .map_err(|e| e.to_string())?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() && !name.is_empty() {
        Ok(name)
    } else {
        Err("could not determine machine hostname".into())
    }
}
fn host_fingerprint() -> Result<String, String> {
    let output = Command::new("ssh-keygen")
        .args(["-lf", "/etc/ssh/ssh_host_ed25519_key.pub", "-E", "sha256"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("could not read the SSH host-key fingerprint".into());
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .nth(1)
        .map(str::to_string)
        .ok_or_else(|| "invalid SSH host-key fingerprint".into())
}
fn unix_now() -> Result<u64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs())
}
fn shell_path(path: &Path) -> Result<String, String> {
    let value = path.to_str().ok_or("path is not UTF-8")?;
    if value.contains('\'') || value.contains('\n') {
        return Err("unsupported quote in path".into());
    }
    Ok(value.into())
}
fn run_status(command: &mut Command, action: &str) -> Result<(), String> {
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to {action}"))
    }
}
fn print_qr(value: &str) -> Result<(), String> {
    let code = QrCode::new(value).map_err(|e| e.to_string())?;
    println!(
        "{}",
        code.render::<unicode::Dense1x2>()
            .dark_color(unicode::Dense1x2::Light)
            .light_color(unicode::Dense1x2::Dark)
            .build()
    );
    Ok(())
}
