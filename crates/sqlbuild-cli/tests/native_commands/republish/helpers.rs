//! A fake plain-HTTP proxy that records what `sqb location republish` sends to a network origin.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Output};
use std::sync::mpsc;

use serde_json::json;

pub(crate) const LOCATION_ID: &str = "00000000-0000-0000-0000-000000000001";
pub(crate) const NETWORK_ORIGIN: &str = "http://control.example.com:8000";

/// Run the command through a proxy answering with `states` in order; its output and each request's head.
pub(crate) fn republish_through_proxy(
    key: Option<&str>,
    states: &[&str],
    arguments: &[&str],
) -> (Output, Vec<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind fake proxy");
    let proxy = format!("http://{}", listener.local_addr().expect("proxy address"));
    let bodies: Vec<String> = states.iter().map(|state| status(state)).collect();
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || answer(&listener, &bodies, &sent));
    let home = tempfile::tempdir().expect("isolated home");
    let mut command = Command::new(env!("CARGO_BIN_EXE_sqb"));
    command
        .args(["location", "republish", "--server", NETWORK_ORIGIN])
        .args(["--location-id", LOCATION_ID, "--target", "production"])
        .args(arguments)
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .env("XDG_DATA_HOME", home.path())
        .env("XDG_STATE_HOME", home.path())
        .env("HTTP_PROXY", &proxy)
        .env("http_proxy", &proxy)
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .env_remove("SQLBUILD_API_KEY");
    if let Some(key) = key {
        command.env("SQLBUILD_API_KEY", key);
    }
    let output = command.output().expect("run republish");
    (output, received.try_iter().collect())
}

fn answer(listener: &std::net::TcpListener, bodies: &[String], sent: &mpsc::Sender<String>) {
    for body in bodies {
        let (stream, _) = listener.accept().expect("accept request");
        let mut reader = BufReader::new(stream);
        let mut head = String::new();
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("request head");
            if line.trim().is_empty() {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(value) = lower.strip_prefix("content-length:") {
                length = value.trim().parse().expect("content length");
            }
            head.push_str(&line);
        }
        reader
            .read_exact(&mut vec![0; length])
            .expect("request body");
        sent.send(head).expect("record request");
        write!(
            reader.get_mut(),
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("response");
    }
}

fn status(state: &str) -> String {
    json!({
        "request_id": "00000000-0000-0000-0000-000000000002",
        "location_id": LOCATION_ID,
        "target": "production",
        "state": state,
        "requested_by": "orders-deploy",
        "reason": "deploy",
        "requested_at": "2026-11-22T09:00:00Z",
        "expires_at": "2026-11-22T09:15:00Z",
    })
    .to_string()
}
