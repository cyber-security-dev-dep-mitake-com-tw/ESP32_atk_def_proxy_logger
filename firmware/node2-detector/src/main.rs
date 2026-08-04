//! Node2 — Deauth detector + alert (ESP32-S3, WiFi WebSocket).
//!
//! Joins WiFi as a station, dials the backend over a WebSocket, puts the radio into
//! promiscuous mode (management frames) on the associated AP channel, runs the
//! sliding-window deauth detector from `common`, and emits `deauth_alert` NDJSON
//! events over the WebSocket. A blue WS2812 LED identifies this node.
//!
//! Compile-time config (set via .wifi.env + flash-node2.sh):
//!   WIFI_SSID, WIFI_PASSWORD  — lab network (2.4 GHz)
//!   WS_URL                    — e.g. ws://172.16.200.60:8080/ws/node/node2
//!
//! Note: channel hopping while associated will drop the STA/WS link, so the default
//! is to stay on the AP's channel. `set_channel` / `start_hop` are still accepted
//! but may disconnect WiFi until the next reconnect cycle.
//!
//! Commands over WebSocket (same NDJSON as serial):
//!   {"cmd":"start_deauth_detect","threshold":5,"window_ms":1000}
//!   {"cmd":"set_channel","ch":6}
//!   {"cmd":"start_hop","dwell_ms":300,"channels":[1,6,11]}
//!   {"cmd":"stop_hop"}  {"cmd":"get_stats"}

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use common::detector::{Config, Detector};
use common::dot11::Frame;
use common::hopper::Hopper;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::rmt::config::TransmitConfig;
use esp_idf_svc::hal::rmt::{FixedLengthSignal, PinState, Pulse, TxRmtDriver};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::{
    esp, esp_wifi_set_channel, esp_wifi_set_promiscuous, esp_wifi_set_promiscuous_filter,
    esp_wifi_set_promiscuous_rx_cb, wifi_promiscuous_filter_t, wifi_promiscuous_pkt_t,
    wifi_promiscuous_pkt_type_t, wifi_second_chan_t_WIFI_SECOND_CHAN_NONE,
    WIFI_PROMIS_FILTER_MASK_MGMT,
};
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use esp_idf_svc::ws::client::{EspWebSocketClient, EspWebSocketClientConfig, WebSocketEventType};
use esp_idf_svc::ws::FrameType;

const WIFI_SSID: Option<&str> = option_env!("WIFI_SSID");
const WIFI_PASSWORD: Option<&str> = option_env!("WIFI_PASSWORD");
const WS_URL: &str = match option_env!("WS_URL") {
    Some(u) => u,
    None => "ws://172.16.200.60:8080/ws/node/node2",
};

static THRESHOLD: AtomicU32 = AtomicU32::new(5);
static WINDOW_MS: AtomicU32 = AtomicU32::new(1000);
static WANT_CHANNEL: AtomicU8 = AtomicU8::new(6);
static DEAUTH_SEEN: AtomicU32 = AtomicU32::new(0);
static ALERTS: AtomicU32 = AtomicU32::new(0);
static WANT_STATS: AtomicBool = AtomicBool::new(false);

/// BSSIDs of deauth-like frames from the promiscuous callback → main.
static DEAUTH_TX: OnceLock<Mutex<mpsc::SyncSender<[u8; 6]>>> = OnceLock::new();
/// Outbound NDJSON text frames → WS sender in the session loop.
static OUT_TX: OnceLock<Mutex<mpsc::SyncSender<String>>> = OnceLock::new();

unsafe extern "C" fn rx_cb(buf: *mut core::ffi::c_void, _t: wifi_promiscuous_pkt_type_t) {
    if buf.is_null() {
        return;
    }
    let pkt = buf as *const wifi_promiscuous_pkt_t;
    let sig_len = (*pkt).rx_ctrl.sig_len() as usize;
    if sig_len < 24 {
        return;
    }
    let payload = core::slice::from_raw_parts((*pkt).payload.as_ptr(), sig_len);
    if let Some(f) = Frame::parse(payload) {
        if f.is_deauth_like() {
            if let Some(tx) = DEAUTH_TX.get() {
                if let Ok(tx) = tx.lock() {
                    let _ = tx.try_send(f.addr3);
                }
            }
        }
    }
}

fn apply_channel(ch: u8) {
    unsafe {
        let _ = esp!(esp_wifi_set_channel(
            ch,
            wifi_second_chan_t_WIFI_SECOND_CHAN_NONE
        ));
    }
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("node2-detector starting: WiFi WebSocket deauth detector");

    let peripherals = Peripherals::take()?;

    let mut led = TxRmtDriver::new(
        peripherals.rmt.channel0,
        peripherals.pins.gpio48,
        &TransmitConfig::new().clock_divider(1),
    )?;
    set_led(&mut led, 0, 0, 0);

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
    loop {
        match wifi.connect().and_then(|_| wifi.wait_netif_up()) {
            Ok(_) => break,
            Err(e) => {
                log::warn!("wifi connect retry: {e}");
                blink_once(&mut led, 0, 0, 60, 200);
                std::thread::sleep(Duration::from_millis(800));
            }
        }
    }
    let ip = wifi.wifi().sta_netif().get_ip_info()?;
    log::info!("WiFi up, IP {:?} — dialing {WS_URL}", ip.ip);

    let (deauth_tx, deauth_rx) = mpsc::sync_channel::<[u8; 6]>(128);
    let _ = DEAUTH_TX.set(Mutex::new(deauth_tx));
    let (out_tx, out_rx) = mpsc::sync_channel::<String>(64);
    let _ = OUT_TX.set(Mutex::new(out_tx));

    unsafe {
        let filter = wifi_promiscuous_filter_t {
            filter_mask: WIFI_PROMIS_FILTER_MASK_MGMT,
        };
        esp!(esp_wifi_set_promiscuous_filter(&filter))?;
        esp!(esp_wifi_set_promiscuous_rx_cb(Some(rx_cb)))?;
        esp!(esp_wifi_set_promiscuous(true))?;
    }
    // Stay on the associated AP channel by default (hopping would drop STA/WS).
    set_hop(None);
    apply_channel(WANT_CHANNEL.load(Ordering::Relaxed));
    log::info!("promiscuous deauth detector armed");

    spawn_control_thread();
    spawn_stats_thread();

    // Reconnect loop: if the backend is down, keep retrying.
    loop {
        if let Err(e) = run_ws_session(&mut led, &deauth_rx, &out_rx) {
            log::warn!("websocket session ended: {e}");
        }
        emit_log("warn", "websocket reconnecting");
        blink_once(&mut led, 0, 0, 80, 150);
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn run_ws_session(
    led: &mut TxRmtDriver<'_>,
    deauth_rx: &mpsc::Receiver<[u8; 6]>,
    out_rx: &mpsc::Receiver<String>,
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
    let mut detector = Detector::new(Config {
        threshold: THRESHOLD.load(Ordering::Relaxed),
        window_ms: WINDOW_MS.load(Ordering::Relaxed) as u64,
    });
    let mut last_cfg = (
        THRESHOLD.load(Ordering::Relaxed),
        WINDOW_MS.load(Ordering::Relaxed),
    );
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
                    b"{\"ev\":\"log\",\"level\":\"info\",\"msg\":\"node2 online\"}",
                );
                hello_sent = true;
            }
        } else if saw_connected {
            anyhow::bail!("websocket disconnected");
        } else if session_start.elapsed() > Duration::from_secs(15) {
            anyhow::bail!("websocket connect timeout");
        }

        let cfg = (
            THRESHOLD.load(Ordering::Relaxed),
            WINDOW_MS.load(Ordering::Relaxed),
        );
        if cfg != last_cfg {
            detector = Detector::new(Config {
                threshold: cfg.0,
                window_ms: cfg.1 as u64,
            });
            last_cfg = cfg;
        }

        while let Ok(bssid) = deauth_rx.try_recv() {
            DEAUTH_SEEN.fetch_add(1, Ordering::Relaxed);
            if let Some(alert) = detector.observe(bssid, now_ms) {
                ALERTS.fetch_add(1, Ordering::Relaxed);
                emit_alert(&alert.bssid, alert.count);
                flash_until = now_ms + 400;
            }
        }

        if WANT_STATS.swap(false, Ordering::Relaxed) {
            emit_stats();
        }

        while let Ok(msg) = out_rx.try_recv() {
            if client.is_connected() {
                let _ = client.send(FrameType::Text(false), msg.as_bytes());
            }
        }

        if now_ms < flash_until {
            set_led(led, 0, 0, 255);
        } else if now_ms.saturating_sub(last_blink) >= 500 {
            last_blink = now_ms;
            led_on = !led_on;
            if led_on {
                set_led(led, 0, 0, 40);
            } else {
                set_led(led, 0, 0, 0);
            }
        }

        std::thread::sleep(Duration::from_millis(20));
    }
}

fn handle_command(txt: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(txt) else {
        return;
    };
    match v.get("cmd").and_then(|c| c.as_str()) {
        Some("start_deauth_detect") => {
            if let Some(t) = v.get("threshold").and_then(|x| x.as_u64()) {
                THRESHOLD.store(t as u32, Ordering::Relaxed);
            }
            if let Some(w) = v.get("window_ms").and_then(|x| x.as_u64()) {
                WINDOW_MS.store(w as u32, Ordering::Relaxed);
            }
            emit_log("info", "deauth detect configured");
        }
        Some("set_channel") => {
            if let Some(ch) = v.get("ch").and_then(|c| c.as_u64()) {
                set_hop(None);
                WANT_CHANNEL.store(ch as u8, Ordering::Relaxed);
                emit_log("warn", "set_channel may drop WiFi association");
            }
        }
        Some("start_hop") => {
            let dwell = v.get("dwell_ms").and_then(|d| d.as_u64()).unwrap_or(300) as u32;
            let channels: Vec<u8> = v
                .get("channels")
                .and_then(|c| c.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_u64().map(|n| n as u8))
                        .collect()
                })
                .unwrap_or_else(|| vec![1, 6, 11]);
            set_hop(Some(Hopper::new(&channels, dwell)));
            emit_log("warn", "start_hop may drop WiFi association");
        }
        Some("stop_hop") => {
            set_hop(None);
            emit_log("info", "stop_hop");
        }
        Some("get_stats") => {
            WANT_STATS.store(true, Ordering::Relaxed);
        }
        _ => {}
    }
}

static HOPPER: OnceLock<Mutex<Option<Hopper>>> = OnceLock::new();

fn hopper_cell() -> &'static Mutex<Option<Hopper>> {
    HOPPER.get_or_init(|| Mutex::new(None))
}

fn set_hop(h: Option<Hopper>) {
    if let Ok(mut guard) = hopper_cell().lock() {
        *guard = h;
    }
}

fn spawn_control_thread() {
    std::thread::spawn(|| {
        let start = Instant::now();
        let mut last_applied = 0u8;
        loop {
            let now_ms = start.elapsed().as_millis() as u64;
            if let Ok(mut guard) = hopper_cell().lock() {
                if let Some(h) = guard.as_mut() {
                    if let Some(ch) = h.tick(now_ms) {
                        WANT_CHANNEL.store(ch, Ordering::Relaxed);
                    }
                }
            }
            let want = WANT_CHANNEL.load(Ordering::Relaxed);
            if want != last_applied {
                apply_channel(want);
                last_applied = want;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
}

fn spawn_stats_thread() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(5));
        WANT_STATS.store(true, Ordering::Relaxed);
    });
}

fn enqueue_out(msg: String) {
    if let Some(tx) = OUT_TX.get() {
        if let Ok(tx) = tx.lock() {
            let _ = tx.try_send(msg);
        }
    }
}

fn emit_stats() {
    let heap = unsafe { esp_idf_svc::sys::esp_get_free_heap_size() };
    enqueue_out(format!(
        "{{\"ev\":\"stats\",\"pps\":{},\"dropped\":0,\"heap\":{}}}",
        DEAUTH_SEEN.load(Ordering::Relaxed),
        heap
    ));
}

fn emit_alert(bssid: &[u8; 6], count: u32) {
    let b = common::dot11::fmt_mac(bssid);
    enqueue_out(format!(
        "{{\"ev\":\"deauth_alert\",\"bssid\":\"{}\",\"count\":{}}}",
        b.as_str(),
        count
    ));
}

fn emit_log(level: &str, msg: &str) {
    let safe: String = msg.chars().filter(|c| *c != '"' && *c != '\\').collect();
    enqueue_out(format!(
        "{{\"ev\":\"log\",\"level\":\"{level}\",\"msg\":\"{safe}\"}}"
    ));
}

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

fn blink_once(led: &mut TxRmtDriver, r: u8, g: u8, b: u8, ms: u64) {
    set_led(led, r, g, b);
    std::thread::sleep(Duration::from_millis(ms));
    set_led(led, 0, 0, 0);
}
