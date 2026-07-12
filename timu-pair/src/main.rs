// timu-pair: generate a QR code for mobile onboarding.
//
// Detects the local network IP and current username, builds a timu://
// connection URL, and renders it as a scannable QR code in the terminal.
//
// Usage:
//   timu-pair                  # auto-detect IP, user, port 22
//   timu-pair --port 2222      # override SSH port
//   timu-pair --host 10.0.0.5  # override host
//   timu-pair --user admin     # override username
//
// The QR encodes a URL like:
//   timu://connect?host=192.168.1.20&port=22&user=ritvij14

use std::env;
use std::net::UdpSocket;

use qrcode::QrCode;
use qrcode::render::unicode;

/// Detect the local network IP by opening a UDP socket to a public
/// address. This doesn't actually send any packets — it just lets the
/// OS pick the source interface, which tells us our LAN IP.
fn detect_local_ip() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let addr = sock.local_addr().ok()?;
    Some(addr.ip().to_string())
}

fn get_username() -> Option<String> {
    env::var("USER").ok().filter(|s| !s.is_empty())
}

fn parse_args(args: &[String]) -> (Option<String>, Option<String>, u16) {
    let mut host = None;
    let mut user = None;
    let mut port: u16 = 22;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--host" => {
                i += 1;
                if i < args.len() {
                    host = Some(args[i].clone());
                }
            }
            "--user" => {
                i += 1;
                if i < args.len() {
                    user = Some(args[i].clone());
                }
            }
            "--port" => {
                i += 1;
                if i < args.len() {
                    if let Ok(p) = args[i].parse::<u16>() {
                        port = p;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    (host, user, port)
}

fn build_url(host: &str, port: u16, user: &str) -> String {
    format!("timu://connect?host={host}&port={port}&user={user}")
}

fn print_qr(url: &str) {
    let code = QrCode::new(url).unwrap();
    let string = code.render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .build();
    println!("{string}");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let (arg_host, arg_user, port) = parse_args(&args);

    let host = arg_host.or_else(detect_local_ip);
    let user = arg_user.or_else(get_username);

    match (&host, &user) {
        (Some(h), Some(u)) => {
            let url = build_url(h, port, u);
            println!("Host:  {h}");
            println!("Port:  {port}");
            println!("User:  {u}");
            println!("URL:   {url}\n");
            println!("Scan this QR with the timu app to connect:\n");
            print_qr(&url);
        }
        (None, _) => {
            eprintln!("Could not auto-detect your local IP address.");
            eprintln!("Run with: timu-pair --host <your-ip>");
            std::process::exit(1);
        }
        (_, None) => {
            eprintln!("Could not determine your username.");
            eprintln!("Run with: timu-pair --user <your-username>");
            std::process::exit(1);
        }
    }
}