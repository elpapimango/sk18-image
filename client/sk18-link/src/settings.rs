//! Settings of the device that controldeck (or anything speaking the deck protocol) can read and change over the
//! link: the Wi-Fi network and Home Assistant. They are extra JSON methods next to `getInfo` and `setBacklight`:
//!
//! - `getSettings` returns `{ wifi: {...}, home_assistant: {...} }`. Neither the Wi-Fi key nor the token is ever sent.
//! - `setWifi { ssid, passphrase?, country, hidden }` writes `/etc/wpa_supplicant.conf` (mode 0600; it holds the derived
//!   256-bit key, never the passphrase) and makes `wpa_supplicant` read it. Without a passphrase the stored key of the
//!   same network is kept. `clearWifi` removes the network.
//! - `setHomeAssistant { enabled, url, event, token? }` stores the access to Home Assistant; without a token the stored
//!   one is kept. `testHomeAssistant { url?, token? }` tries it and answers with a sentence.
//!
//! The commands the module runs (`wpa_passphrase`, `wpa_cli`, `ifup`) go through [`System`], so everything is tested
//! without a board.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::ha::{self, HaConfig};
pub use crate::ha::SharedHa;

/// The Wi-Fi interface of the board.
pub const WLAN: &str = "wlan0";
/// Where the Wi-Fi network is stored (the same file `tools/flash/provision-wifi.sh` writes).
pub const WPA_CONF: &str = "/etc/wpa_supplicant.conf";
/// Where the Home Assistant access is stored.
pub const HA_CONF: &str = "/etc/sk18-home-assistant.json";

/// Settings the protocol logic can ask for. `None` means "not a settings method".
pub trait Settings {
    /// Runs `method` with `params`.
    fn call(&mut self, method: &str, params: &Value) -> Option<Result<Value, String>>;
}

/// The operating system, as far as the settings need it.
pub trait System {
    /// Runs a program to completion and returns its standard output; `Err` carries the failure text.
    fn run(&self, program: &str, args: &[&str], stdin: Option<&str>) -> Result<String, String>;
    /// Starts a program in the background and does not wait.
    fn spawn_detached(&self, program: &str, args: &[&str]);
    /// Whether the network interface exists (the USB adapter is plugged in and switched).
    fn interface_present(&self, name: &str) -> bool;
}

/// The real thing.
#[derive(Debug, Default, Clone, Copy)]
pub struct LinuxSystem;

impl System for LinuxSystem {
    fn run(&self, program: &str, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{program}: {e}"))?;
        if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
            pipe.write_all(text.as_bytes()).map_err(|e| format!("{program}: {e}"))?;
        }
        let out = child.wait_with_output().map_err(|e| format!("{program}: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(format!("{program}: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }

    fn spawn_detached(&self, program: &str, args: &[&str]) {
        // `setsid` so that the child outlives a restart of this daemon; its output is not needed.
        let _ = Command::new("setsid")
            .arg(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }

    fn interface_present(&self, name: &str) -> bool {
        Path::new("/sys/class/net").join(name).exists()
    }
}

/// The stored Wi-Fi network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiNetwork {
    /// Network name as bytes (it is stored as hex, so any bytes are safe).
    pub ssid: Vec<u8>,
    /// The derived 256-bit key as 64 hex digits.
    pub psk: String,
    /// Two-letter regulatory domain.
    pub country: String,
    /// The network does not announce its name.
    pub hidden: bool,
}

impl WifiNetwork {
    /// The `wpa_supplicant.conf` text.
    pub fn render(&self) -> String {
        let hex: String = self.ssid.iter().map(|b| format!("{b:02x}")).collect();
        let mut text = format!(
            "ctrl_interface=/run/wpa_supplicant\nupdate_config=0\ncountry={}\nnetwork={{\n\tssid={hex}\n\tkey_mgmt=WPA-PSK\n\tpsk={}\n",
            self.country, self.psk
        );
        if self.hidden {
            text.push_str("\tscan_ssid=1\n");
        }
        text.push_str("}\n");
        text
    }

    /// Reads a file written by [`WifiNetwork::render`] or by `provision-wifi.sh`; `None` when it has no network.
    pub fn parse(text: &str) -> Option<Self> {
        let (mut ssid, mut psk, mut country, mut hidden) = (None, None, String::new(), false);
        let mut inside = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with("network=") {
                inside = true;
            } else if line == "}" {
                inside = false;
            } else if let Some(value) = line.strip_prefix("country=") {
                country = value.to_owned();
            } else if inside {
                if let Some(value) = line.strip_prefix("ssid=") {
                    ssid = Some(match value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
                        Some(quoted) => quoted.as_bytes().to_vec(),
                        None => unhex(value)?,
                    });
                } else if let Some(value) = line.strip_prefix("psk=") {
                    psk = Some(value.to_owned());
                } else if line == "scan_ssid=1" {
                    hidden = true;
                }
            }
        }
        Some(Self { ssid: ssid?, psk: psk?, country, hidden })
    }

    /// The name for display (lossy for non-UTF-8 names).
    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.ssid).into_owned()
    }
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 || !text.is_ascii() {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok()).collect()
}

fn is_hex64(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// What the Wi-Fi link is doing, from `wpa_cli status`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LinkStatus {
    /// `wpa_state`.
    pub wpa_state: String,
    /// The address, once DHCP gave one.
    pub ip: Option<String>,
}

/// Reads `wpa_cli status` output.
pub fn parse_status(text: &str) -> LinkStatus {
    let mut status = LinkStatus::default();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("wpa_state=") {
            status.wpa_state = v.trim().to_owned();
        } else if let Some(v) = line.strip_prefix("ip_address=") {
            status.ip = Some(v.trim().to_owned());
        }
    }
    status
}

fn signal_dbm(text: &str) -> Option<i64> {
    text.lines().find_map(|l| l.strip_prefix("RSSI=")).and_then(|v| v.trim().parse().ok())
}

/// The real settings of the board, or a fake system in the tests.
pub struct SystemSettings<S> {
    system: S,
    wpa_conf: PathBuf,
    ha_conf: PathBuf,
    ha: SharedHa,
}

impl<S: System> SystemSettings<S> {
    /// Uses the given files; loads the saved Home Assistant access into `ha` (which the key sender shares).
    pub fn new(system: S, wpa_conf: impl Into<PathBuf>, ha_conf: impl Into<PathBuf>, ha: SharedHa) -> Self {
        let this = Self { system, wpa_conf: wpa_conf.into(), ha_conf: ha_conf.into(), ha };
        *this.ha.write().expect("ha lock") = this.load_ha();
        this
    }

    fn load_ha(&self) -> HaConfig {
        std::fs::read_to_string(&self.ha_conf)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(|v| HaConfig::from_json(&v))
            .unwrap_or_else(|| HaConfig::from_json(&Value::Null))
    }

    fn network(&self) -> Option<WifiNetwork> {
        std::fs::read_to_string(&self.wpa_conf).ok().and_then(|t| WifiNetwork::parse(&t))
    }

    /// Writes a secret file through a temporary one, readable by root only.
    fn write_private(path: &Path, text: &str) -> Result<(), String> {
        let temporary = path.with_extension("tmp");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|e| format!("{}: {e}", temporary.display()))?;
        file.write_all(text.as_bytes()).map_err(|e| format!("{}: {e}", temporary.display()))?;
        file.sync_all().ok();
        std::fs::rename(&temporary, path).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn wifi_state(&self, network: &Option<WifiNetwork>) -> Value {
        let Some(network) = network else {
            return json!({ "configured": false, "state": "not-configured" });
        };
        let mut wifi = json!({
            "configured": true,
            "ssid": network.name(),
            "country": network.country,
            "hidden": network.hidden,
        });
        let state = if !self.system.interface_present(WLAN) {
            "no-adapter".to_owned()
        } else {
            match self.system.run("wpa_cli", &["-i", WLAN, "status"], None) {
                Err(_) => "not-running".to_owned(),
                Ok(text) => {
                    let status = parse_status(&text);
                    match (status.wpa_state.as_str(), status.ip) {
                        ("COMPLETED", Some(ip)) => {
                            wifi["ip"] = json!(ip);
                            if let Ok(poll) = self.system.run("wpa_cli", &["-i", WLAN, "signal_poll"], None) {
                                wifi["signal_dbm"] = json!(signal_dbm(&poll));
                            }
                            "connected".to_owned()
                        }
                        ("COMPLETED", None) => "getting-address".to_owned(),
                        ("DISCONNECTED" | "INACTIVE" | "INTERFACE_DISABLED", _) => "disconnected".to_owned(),
                        _ => "connecting".to_owned(),
                    }
                }
            }
        };
        wifi["state"] = json!(state);
        wifi
    }

    fn get_settings(&self) -> Value {
        let network = self.network();
        json!({
            "wifi": self.wifi_state(&network),
            "home_assistant": self.ha.read().expect("ha lock").to_public(),
        })
    }

    /// The 256-bit key for `passphrase` on network `ssid`.
    fn derive_psk(&self, ssid: &str, passphrase: &str) -> Result<String, String> {
        if is_hex64(passphrase) {
            return Ok(passphrase.to_ascii_lowercase());
        }
        if !(8..=63).contains(&passphrase.len()) || !passphrase.bytes().all(|b| (32..=126).contains(&b)) {
            return Err("a Wi-Fi password has 8 to 63 characters (letters, digits and symbols, no accents)".into());
        }
        let out = self.system.run("wpa_passphrase", &[ssid], Some(passphrase))?;
        out.lines()
            .map(str::trim)
            .find_map(|l| l.strip_prefix("psk=").filter(|v| is_hex64(v)))
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| "wpa_passphrase gave no key".to_owned())
    }

    fn set_wifi(&mut self, params: &Value) -> Result<Value, String> {
        let text = |name: &str| params.get(name).and_then(Value::as_str).unwrap_or_default();
        let ssid = text("ssid");
        if ssid.is_empty() || ssid.len() > 32 {
            return Err("the network name must be 1 to 32 bytes".into());
        }
        let country = text("country").to_ascii_uppercase();
        if country.len() != 2 || !country.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err("the country must be a two-letter code such as DE, GB or US".into());
        }
        let hidden = params.get("hidden").and_then(Value::as_bool).unwrap_or(false);
        let passphrase = text("passphrase");
        let psk = if passphrase.is_empty() {
            match self.network() {
                Some(old) if old.ssid == ssid.as_bytes() => old.psk,
                _ => return Err("enter the password of the network".into()),
            }
        } else {
            self.derive_psk(ssid, passphrase)?
        };
        let network = WifiNetwork { ssid: ssid.as_bytes().to_vec(), psk, country, hidden };
        Self::write_private(&self.wpa_conf, &network.render())?;
        self.apply_wifi();
        Ok(self.get_settings())
    }

    /// Makes the running `wpa_supplicant` read the file, or starts the interface when nothing is running yet.
    fn apply_wifi(&self) {
        if self.system.run("wpa_cli", &["-i", WLAN, "reconfigure"], None).is_err() && self.system.interface_present(WLAN) {
            self.system.spawn_detached("/sbin/ifup", &[WLAN]);
        }
    }

    fn clear_wifi(&mut self) -> Result<Value, String> {
        match std::fs::remove_file(&self.wpa_conf) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", self.wpa_conf.display())),
        }
        if self.system.interface_present(WLAN) {
            self.system.spawn_detached("/sbin/ifdown", &[WLAN]);
        }
        Ok(self.get_settings())
    }

    fn set_home_assistant(&mut self, params: &Value) -> Result<Value, String> {
        let text = |name: &str| params.get(name).and_then(Value::as_str);
        let mut config = self.ha.read().expect("ha lock").clone();
        if let Some(url) = text("url") {
            let url = url.trim();
            if !url.is_empty() {
                ha::parse_url(url)?;
            }
            config.url = url.to_owned();
        }
        if let Some(event) = text("event") {
            let event = if event.trim().is_empty() { ha::DEFAULT_EVENT } else { event.trim() };
            if !ha::valid_event(event) {
                return Err("the event name may only contain letters, digits and _, and must start with a letter".into());
            }
            config.event = event.to_owned();
        }
        if let Some(token) = text("token").filter(|t| !t.is_empty()) {
            if !token.bytes().all(|b| (33..=126).contains(&b)) {
                return Err("the access token must be a single line without spaces".into());
            }
            config.token = token.to_owned();
        }
        if params.get("clear_token").and_then(Value::as_bool) == Some(true) {
            config.token.clear();
        }
        config.enabled = params.get("enabled").and_then(Value::as_bool).unwrap_or(config.enabled);
        if config.enabled && (config.url.is_empty() || config.token.is_empty()) {
            return Err("to send events, Home Assistant needs an address and an access token".into());
        }
        Self::write_private(&self.ha_conf, &config.to_json().to_string())?;
        *self.ha.write().expect("ha lock") = config;
        Ok(self.get_settings())
    }

    fn test_home_assistant(&self, params: &Value) -> Result<Value, String> {
        let mut config = self.ha.read().expect("ha lock").clone();
        if let Some(url) = params.get("url").and_then(Value::as_str).filter(|u| !u.trim().is_empty()) {
            config.url = url.trim().to_owned();
        }
        if let Some(token) = params.get("token").and_then(Value::as_str).filter(|t| !t.is_empty()) {
            config.token = token.to_owned();
        }
        ha::check(&config).map(|message| json!({ "message": message }))
    }
}

impl<S: System> Settings for SystemSettings<S> {
    fn call(&mut self, method: &str, params: &Value) -> Option<Result<Value, String>> {
        Some(match method {
            "getSettings" => Ok(self.get_settings()),
            "setWifi" => self.set_wifi(params),
            "clearWifi" => self.clear_wifi(),
            "setHomeAssistant" => self.set_home_assistant(params),
            "testHomeAssistant" => self.test_home_assistant(params),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Default)]
    struct FakeSystem {
        calls: RefCell<Vec<String>>,
        detached: RefCell<Vec<String>>,
        adapter: bool,
        supplicant_running: bool,
        status: String,
    }

    impl System for &FakeSystem {
        fn run(&self, program: &str, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
            self.calls.borrow_mut().push(format!("{program} {} <{}>", args.join(" "), stdin.unwrap_or("")));
            match (program, args) {
                ("wpa_passphrase", _) => Ok("network={\n\tssid=\"x\"\n\t#psk=\"hunter2hunter2\"\n\tpsk=".to_owned() + &"ab".repeat(32) + "\n}\n"),
                ("wpa_cli", [_, _, "status"]) if self.supplicant_running => Ok(self.status.clone()),
                ("wpa_cli", [_, _, "signal_poll"]) => Ok("RSSI=-58\nLINKSPEED=65\n".into()),
                ("wpa_cli", [_, _, "reconfigure"]) if self.supplicant_running => Ok("OK\n".into()),
                _ => Err(format!("{program}: failed")),
            }
        }

        fn spawn_detached(&self, program: &str, args: &[&str]) {
            self.detached.borrow_mut().push(format!("{program} {}", args.join(" ")));
        }

        fn interface_present(&self, _: &str) -> bool {
            self.adapter
        }
    }

    fn scratch() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("sk18-settings-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn settings<'a>(system: &'a FakeSystem, dir: &Path) -> SystemSettings<&'a FakeSystem> {
        SystemSettings::new(system, dir.join("wpa.conf"), dir.join("ha.json"), SharedHa::default())
    }

    fn up() -> FakeSystem {
        FakeSystem {
            adapter: true,
            supplicant_running: true,
            status: "ssid=Home\nwpa_state=COMPLETED\nip_address=192.168.0.124\n".into(),
            ..FakeSystem::default()
        }
    }

    fn set_wifi_params() -> Value {
        json!({ "ssid": "Home", "passphrase": "correct horse", "country": "de", "hidden": true })
    }

    #[test]
    fn the_stored_network_survives_a_round_trip_and_reads_the_provisioning_scripts_file() {
        let network = WifiNetwork { ssid: "Café ✓".as_bytes().to_vec(), psk: "cd".repeat(32), country: "DE".into(), hidden: true };
        assert_eq!(WifiNetwork::parse(&network.render()), Some(network));
        let provisioned = format!("ctrl_interface=/run/wpa_supplicant\nupdate_config=0\ncountry=GB\nnetwork={{\n\tssid=486f6d65\n\tkey_mgmt=WPA-PSK\n\tpsk={}\n}}\n", "ef".repeat(32));
        let parsed = WifiNetwork::parse(&provisioned).unwrap();
        assert_eq!((parsed.name().as_str(), parsed.country.as_str(), parsed.hidden), ("Home", "GB", false));
        assert!(WifiNetwork::parse("country=DE\n").is_none());
    }

    #[test]
    fn setting_wifi_stores_the_derived_key_never_the_password_and_applies_it() {
        let (system, dir) = (up(), scratch());
        let mut s = settings(&system, &dir);
        let got = s.call("setWifi", &set_wifi_params()).unwrap().unwrap();
        let file = std::fs::read_to_string(dir.join("wpa.conf")).unwrap();
        assert!(!file.contains("correct horse"), "{file}");
        assert!(file.contains(&"ab".repeat(32)) && file.contains("country=DE") && file.contains("scan_ssid=1"), "{file}");
        assert!(system.calls.borrow().iter().any(|c| c == "wpa_passphrase Home <correct horse>"));
        assert!(system.calls.borrow().iter().any(|c| c.starts_with("wpa_cli -i wlan0 reconfigure")));
        assert_eq!(got["wifi"]["ssid"], "Home");
        assert_eq!(got["wifi"]["state"], "connected");
        assert_eq!(got["wifi"]["ip"], "192.168.0.124");
        assert_eq!(got["wifi"]["signal_dbm"], -58);
        assert!(!got.to_string().contains(&"ab".repeat(32)), "the key must not be reported");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir.join("wpa.conf")).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn without_a_running_supplicant_the_interface_is_started() {
        let system = FakeSystem { adapter: true, ..FakeSystem::default() };
        let dir = scratch();
        settings(&system, &dir).call("setWifi", &set_wifi_params()).unwrap().unwrap();
        assert_eq!(*system.detached.borrow(), vec!["/sbin/ifup wlan0".to_owned()]);
        // no adapter: nothing to start, and the state says why
        let system = FakeSystem::default();
        let got = settings(&system, &dir).call("getSettings", &Value::Null).unwrap().unwrap();
        assert_eq!(got["wifi"]["state"], "no-adapter");
    }

    #[test]
    fn the_password_can_stay_when_only_the_same_network_is_edited() {
        let (system, dir) = (up(), scratch());
        let mut s = settings(&system, &dir);
        s.call("setWifi", &set_wifi_params()).unwrap().unwrap();
        let edit = json!({ "ssid": "Home", "country": "GB", "hidden": false });
        s.call("setWifi", &edit).unwrap().unwrap();
        let file = std::fs::read_to_string(dir.join("wpa.conf")).unwrap();
        assert!(file.contains(&"ab".repeat(32)) && file.contains("country=GB") && !file.contains("scan_ssid"), "{file}");
        // another network needs its password
        let other = json!({ "ssid": "Other", "country": "GB" });
        assert!(s.call("setWifi", &other).unwrap().unwrap_err().contains("password"));
    }

    #[test]
    fn bad_wifi_input_is_refused_and_nothing_is_written() {
        let (system, dir) = (up(), scratch());
        let mut s = settings(&system, &dir);
        for bad in [
            json!({ "ssid": "", "passphrase": "longenough", "country": "DE" }),
            json!({ "ssid": "x".repeat(33), "passphrase": "longenough", "country": "DE" }),
            json!({ "ssid": "Home", "passphrase": "short", "country": "DE" }),
            json!({ "ssid": "Home", "passphrase": "longenough", "country": "GER" }),
            json!({ "ssid": "Home", "passphrase": "longenough", "country": "1A" }),
            json!({ "ssid": "Home", "passphrase": "ünïcödé-pass", "country": "DE" }),
        ] {
            assert!(s.call("setWifi", &bad).unwrap().is_err(), "{bad}");
        }
        assert!(!dir.join("wpa.conf").exists());
    }

    #[test]
    fn a_raw_64_digit_key_is_taken_as_it_is() {
        let (system, dir) = (up(), scratch());
        let key = "A1".repeat(32);
        settings(&system, &dir).call("setWifi", &json!({ "ssid": "Home", "passphrase": key, "country": "DE" })).unwrap().unwrap();
        assert!(std::fs::read_to_string(dir.join("wpa.conf")).unwrap().contains(&"a1".repeat(32)));
        assert!(!system.calls.borrow().iter().any(|c| c.starts_with("wpa_passphrase")));
    }

    #[test]
    fn clearing_removes_the_network_and_stops_the_interface() {
        let (system, dir) = (up(), scratch());
        let mut s = settings(&system, &dir);
        s.call("setWifi", &set_wifi_params()).unwrap().unwrap();
        let got = s.call("clearWifi", &Value::Null).unwrap().unwrap();
        assert!(!dir.join("wpa.conf").exists());
        assert_eq!(got["wifi"]["configured"], false);
        assert_eq!(system.detached.borrow().last().unwrap(), "/sbin/ifdown wlan0");
        s.call("clearWifi", &Value::Null).unwrap().unwrap(); // clearing nothing is fine
    }

    #[test]
    fn link_states_follow_the_supplicant() {
        let dir = scratch();
        for (status, want) in [
            ("wpa_state=COMPLETED\n", "getting-address"),
            ("wpa_state=SCANNING\n", "connecting"),
            ("wpa_state=4WAY_HANDSHAKE\n", "connecting"),
            ("wpa_state=DISCONNECTED\n", "disconnected"),
        ] {
            let system = FakeSystem { status: status.into(), ..up() };
            let mut s = settings(&system, &dir);
            s.call("setWifi", &set_wifi_params()).unwrap().unwrap();
            assert_eq!(s.call("getSettings", &Value::Null).unwrap().unwrap()["wifi"]["state"], want, "{status}");
        }
    }

    #[test]
    fn home_assistant_is_saved_with_its_token_which_is_never_reported() {
        let (system, dir) = (up(), scratch());
        let shared = SharedHa::default();
        let mut s = SystemSettings::new(&system, dir.join("wpa.conf"), dir.join("ha.json"), shared.clone());
        let got = s
            .call("setHomeAssistant", &json!({ "enabled": true, "url": "http://homeassistant.local:8123", "token": "abc.def", "event": "deck" }))
            .unwrap()
            .unwrap();
        assert_eq!(got["home_assistant"], json!({ "enabled": true, "url": "http://homeassistant.local:8123", "event": "deck", "token_set": true }));
        assert!(!got.to_string().contains("abc.def"));
        assert_eq!(shared.read().unwrap().token, "abc.def");
        // a later edit without a token keeps it, and a restart reads it back
        s.call("setHomeAssistant", &json!({ "event": "other" })).unwrap().unwrap();
        let again = settings(&system, &dir);
        assert_eq!(again.ha.read().unwrap().token, "abc.def");
        assert_eq!(again.ha.read().unwrap().event, "other");
        s.call("setHomeAssistant", &json!({ "enabled": false, "clear_token": true })).unwrap().unwrap();
        assert_eq!(shared.read().unwrap().token, "");
    }

    #[test]
    fn home_assistant_input_is_checked() {
        let (system, dir) = (up(), scratch());
        let mut s = settings(&system, &dir);
        // turning events on needs somewhere to send them
        assert!(s.call("setHomeAssistant", &json!({ "enabled": true })).unwrap().is_err());
        assert!(s.call("setHomeAssistant", &json!({ "url": "https://ha" })).unwrap().unwrap_err().contains("TLS"));
        assert!(s.call("setHomeAssistant", &json!({ "event": "bad name" })).unwrap().is_err());
        assert!(s.call("setHomeAssistant", &json!({ "token": "has space" })).unwrap().is_err());
        assert!(!dir.join("ha.json").exists());
    }

    #[test]
    fn other_methods_are_not_settings() {
        let (system, dir) = (up(), scratch());
        assert!(settings(&system, &dir).call("setBacklight", &Value::Null).is_none());
    }
}
