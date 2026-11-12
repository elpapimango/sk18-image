//! Home Assistant, straight from the device: key presses become events on Home Assistant's REST API
//! (`POST /api/events/<event>`, authorised by a long-lived access token).
//!
//! This is for an SK-18 that is used with Home Assistant on its own network, with or without controldeck. Plain HTTP
//! only: the device carries no TLS stack, so the address must be the `http://` one (Home Assistant's default,
//! port 8123). Everything here is pure or uses `std::net`, so it is tested on any host.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Event type used when none is configured.
pub const DEFAULT_EVENT: &str = "sk18_key";
const TIMEOUT: Duration = Duration::from_secs(4);

/// What is saved about Home Assistant.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HaConfig {
    /// Send key events.
    pub enabled: bool,
    /// Base address, for example `http://homeassistant.local:8123` or `http://192.168.0.5:8123`.
    pub url: String,
    /// Long-lived access token. Never leaves the device once stored.
    pub token: String,
    /// Event type fired on Home Assistant's bus.
    pub event: String,
}

impl HaConfig {
    /// For the file on the device.
    pub fn to_json(&self) -> Value {
        json!({ "enabled": self.enabled, "url": self.url, "token": self.token, "event": self.event })
    }

    /// Reads what [`HaConfig::to_json`] wrote; anything missing is empty or off.
    pub fn from_json(value: &Value) -> Self {
        let text = |name: &str| {
            value
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let event = text("event");
        Self {
            enabled: value.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            url: text("url"),
            token: text("token"),
            event: if event.is_empty() {
                DEFAULT_EVENT.to_owned()
            } else {
                event
            },
        }
    }

    /// What the host may see: no token.
    pub fn to_public(&self) -> Value {
        json!({ "enabled": self.enabled, "url": self.url, "event": self.event, "token_set": !self.token.is_empty() })
    }
}

/// The Home Assistant access, shared between the settings (which change it) and the sender (which reads it).
pub type SharedHa = Arc<RwLock<HaConfig>>;

/// Hands key presses to a background thread that posts them, so that a slow or unreachable Home Assistant never
/// delays the keys or the pictures. When the thread falls behind, the newest events are dropped.
#[derive(Clone)]
pub struct Notifier {
    config: SharedHa,
    queue: SyncSender<Value>,
}

impl Notifier {
    /// Starts the sender thread.
    pub fn spawn(config: SharedHa) -> Self {
        let (queue, events) = sync_channel::<Value>(16);
        let shared = config.clone();
        std::thread::spawn(move || {
            let mut last_report: Option<Instant> = None;
            while let Ok(body) = events.recv() {
                let config = shared.read().expect("ha lock").clone();
                if !config.enabled {
                    continue;
                }
                if let Err(e) = send_event(&config, &body) {
                    // One line every ten seconds is enough to see that it is not working.
                    if last_report.is_none_or(|t| t.elapsed() > Duration::from_secs(10)) {
                        eprintln!("sk18-link: Home Assistant: {e}");
                        last_report = Some(Instant::now());
                    }
                }
            }
        });
        Self { config, queue }
    }

    /// A key was pressed or released; `row` and `col` count from 1 as the viewer sees the device.
    pub fn key(&self, row: u8, col: u8, pressed: bool) {
        if self.config.read().expect("ha lock").enabled {
            let _ = self.queue.try_send(key_body(row, col, pressed));
        }
    }
}

/// Where to connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Host name or address.
    pub host: String,
    /// TCP port.
    pub port: u16,
}

/// Understands `http://host`, `http://host:8123`, `host`, `host:8123` (a trailing `/` is fine).
pub fn parse_url(text: &str) -> Result<Endpoint, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("the Home Assistant address is empty".into());
    }
    let rest = match text.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("http") => rest,
        Some((scheme, _)) if scheme.eq_ignore_ascii_case("https") => {
            return Err("https is not possible from the device (it has no TLS); use the http:// address of Home Assistant".into())
        }
        Some((scheme, _)) => return Err(format!("unsupported scheme {scheme:?}; use http://")),
        None => text,
    };
    let rest = rest.trim_end_matches('/');
    if rest.is_empty() || rest.contains('/') || rest.contains(char::is_whitespace) {
        return Err("the address must be just a host and an optional port, for example http://homeassistant.local:8123".into());
    }
    let (host, port) = match rest.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (
            host,
            port.parse::<u16>()
                .map_err(|_| format!("{port:?} is not a port number"))?,
        ),
        _ => (rest, 8123),
    };
    if host.is_empty() {
        return Err("the host name is empty".into());
    }
    Ok(Endpoint {
        host: host.to_owned(),
        port,
    })
}

/// A valid event type: letters, digits and `_`, starting with a letter.
pub fn valid_event(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= 64
}

/// The body fired for a key; `row` and `col` count from 1 as the viewer sees the device.
pub fn key_body(row: u8, col: u8, pressed: bool) -> Value {
    json!({
        "key": format!("r{row}c{col}"),
        "row": row,
        "column": col,
        "pressed": pressed,
        "action": if pressed { "down" } else { "up" },
        "device": "sk18",
    })
}

fn request(method: &str, path: &str, endpoint: &Endpoint, token: &str, body: Option<&Value>) -> Vec<u8> {
    let payload = body.map(Value::to_string).unwrap_or_default();
    let mut text = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}:{}\r\nAuthorization: Bearer {token}\r\nAccept: application/json\r\nConnection: close\r\n",
        endpoint.host, endpoint.port
    );
    if body.is_some() {
        text.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            payload.len()
        ));
    }
    text.push_str("\r\n");
    text.push_str(&payload);
    text.into_bytes()
}

/// Splits a raw HTTP response into its status code and body.
pub fn parse_response(raw: &[u8]) -> Result<(u16, String), String> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text.split_once("\r\n\r\n").ok_or("the answer is not HTTP")?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or("the answer has no status line")?;
    Ok((status, body.to_owned()))
}

fn exchange(endpoint: &Endpoint, bytes: &[u8]) -> Result<(u16, String), String> {
    let address = (endpoint.host.as_str(), endpoint.port)
        .to_socket_addrs()
        .map_err(|e| format!("cannot find {}: {e}", endpoint.host))?
        .next()
        .ok_or_else(|| format!("cannot find {}", endpoint.host))?;
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT)
        .map_err(|e| format!("cannot connect to {address}: {e}"))?;
    stream.set_read_timeout(Some(TIMEOUT)).ok();
    stream.set_write_timeout(Some(TIMEOUT)).ok();
    stream
        .write_all(bytes)
        .map_err(|e| format!("sending failed: {e}"))?;
    let mut raw = Vec::new();
    // `Connection: close`: the answer ends with the connection. A timeout after the headers still leaves a usable answer.
    let mut buf = [0u8; 2048];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&buf[..n]);
                if raw.len() > 64 * 1024 {
                    break;
                }
            }
            Err(e)
                if !raw.is_empty()
                    && matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
            {
                break
            }
            Err(e) => return Err(format!("no answer from {address}: {e}")),
        }
    }
    parse_response(&raw)
}

fn explain(status: u16) -> String {
    match status {
        401 => "Home Assistant refused the access token (401)".into(),
        404 => "that address answers, but it does not look like Home Assistant's API (404)".into(),
        other => format!("Home Assistant answered {other}"),
    }
}

/// Fires one event. Used by the sender thread.
pub fn send_event(config: &HaConfig, body: &Value) -> Result<(), String> {
    let endpoint = parse_url(&config.url)?;
    let path = format!("/api/events/{}", config.event);
    let (status, _) = exchange(
        &endpoint,
        &request("POST", &path, &endpoint, &config.token, Some(body)),
    )?;
    if status == 200 {
        Ok(())
    } else {
        Err(explain(status))
    }
}

/// Asks the API for its greeting, which proves address, port and token. Returns a sentence for the user.
pub fn check(config: &HaConfig) -> Result<String, String> {
    if config.token.is_empty() {
        return Err("there is no access token".into());
    }
    let endpoint = parse_url(&config.url)?;
    let started = Instant::now();
    let (status, body) = exchange(
        &endpoint,
        &request("GET", "/api/", &endpoint, &config.token, None),
    )?;
    if status != 200 {
        return Err(explain(status));
    }
    let message = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| "connected".into());
    Ok(format!("{message} ({} ms)", started.elapsed().as_millis()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn addresses_are_understood_in_the_usual_spellings() {
        let ok = |s: &str| parse_url(s).unwrap();
        assert_eq!(
            ok("http://homeassistant.local:8123"),
            Endpoint {
                host: "homeassistant.local".into(),
                port: 8123
            }
        );
        assert_eq!(
            ok("192.168.0.5"),
            Endpoint {
                host: "192.168.0.5".into(),
                port: 8123
            }
        );
        assert_eq!(
            ok(" HTTP://ha.lan:80/ "),
            Endpoint {
                host: "ha.lan".into(),
                port: 80
            }
        );
        assert!(parse_url("https://ha.example").unwrap_err().contains("no TLS"));
        assert!(parse_url("ftp://x").is_err());
        assert!(parse_url("").is_err());
        assert!(parse_url("host:notaport").is_err());
        assert!(parse_url("http://host/api").is_err());
    }

    #[test]
    fn event_names_follow_home_assistants_rules() {
        assert!(valid_event("sk18_key"));
        assert!(!valid_event("1key"));
        assert!(!valid_event("a b"));
        assert!(!valid_event("a/b"));
        assert!(!valid_event(""));
    }

    #[test]
    fn the_saved_form_keeps_the_token_and_the_public_form_hides_it() {
        let config = HaConfig {
            enabled: true,
            url: "http://x".into(),
            token: "secret".into(),
            event: "e".into(),
        };
        assert_eq!(HaConfig::from_json(&config.to_json()), config);
        assert!(!config.to_public().to_string().contains("secret"));
        assert_eq!(config.to_public()["token_set"], true);
        assert_eq!(HaConfig::from_json(&json!({})).event, DEFAULT_EVENT);
    }

    #[test]
    fn a_key_event_names_the_key_as_the_viewer_counts_it() {
        let body = key_body(2, 5, true);
        assert_eq!(body["key"], "r2c5");
        assert_eq!(body["action"], "down");
        assert_eq!(key_body(1, 1, false)["action"], "up");
    }

    #[test]
    fn responses_are_split_into_status_and_body() {
        assert_eq!(
            parse_response(b"HTTP/1.1 200 OK\r\nA: b\r\n\r\n{\"message\":\"x\"}").unwrap(),
            (200, "{\"message\":\"x\"}".into())
        );
        assert!(parse_response(b"garbage").is_err());
    }

    /// A one-shot server that records the request and answers `reply`.
    fn serve(reply: &'static str) -> (u16, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
            let mut seen = Vec::new();
            let mut buf = [0u8; 1024];
            while let Ok(n) = stream.read(&mut buf) {
                if n == 0 {
                    break;
                }
                seen.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&seen).contains("\r\n\r\n")
                    && (!seen.starts_with(b"POST") || seen.ends_with(b"}"))
                {
                    break;
                }
            }
            stream.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&seen).into_owned()
        });
        (port, handle)
    }

    #[test]
    fn an_event_is_posted_with_the_token_and_the_body() {
        let (port, server) = serve("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let config = HaConfig {
            enabled: true,
            url: format!("http://127.0.0.1:{port}"),
            token: "tok".into(),
            event: "sk18_key".into(),
        };
        send_event(&config, &key_body(1, 2, true)).unwrap();
        let request = server.join().unwrap();
        assert!(
            request.starts_with("POST /api/events/sk18_key HTTP/1.1\r\n"),
            "{request}"
        );
        assert!(request.contains("Authorization: Bearer tok\r\n"), "{request}");
        assert!(request.ends_with(&key_body(1, 2, true).to_string()), "{request}");
    }

    #[test]
    fn the_check_reads_the_greeting_and_explains_a_refusal() {
        let (port, server) =
            serve("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{\"message\":\"API running.\"}");
        let config = HaConfig {
            enabled: true,
            url: format!("127.0.0.1:{port}"),
            token: "tok".into(),
            event: "e".into(),
        };
        assert!(check(&config).unwrap().starts_with("API running."));
        assert!(server.join().unwrap().starts_with("GET /api/ HTTP/1.1"));

        let (port, server) = serve("HTTP/1.1 401 Unauthorized\r\nConnection: close\r\n\r\n");
        let config = HaConfig {
            url: format!("127.0.0.1:{port}"),
            token: "bad".into(),
            ..config
        };
        assert!(check(&config).unwrap_err().contains("refused the access token"));
        server.join().unwrap();

        assert!(check(&HaConfig {
            url: "x".into(),
            ..HaConfig::default()
        })
        .unwrap_err()
        .contains("no access token"));
    }
}
