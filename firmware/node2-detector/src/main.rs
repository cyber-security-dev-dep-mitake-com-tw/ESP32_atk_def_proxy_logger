//! Node2 — Deauth detector + alert (ESP32-S3, native USB).
//!
//! Puts the ESP32 WiFi into promiscuous mode filtered to management frames, runs
//! the sliding-window deauth detector from `common`, and emits `deauth_alert`
//! NDJSON events over the native USB serial. A blue WS2812 RGB LED blinks as a
//! heartbeat so this board is easy to identify as Node2, and flashes brighter for
//! a moment on each detected attack.
//!
//! Commands accepted over the same serial link:
//!   {"cmd":"start_deauth_detect","threshold":5,"window_ms":1000}
//!   {"cmd":"set_channel","ch":6}
//!   {"cmd":"start_hop","dwell_ms":300,"channels":[1,6,11]}
//!   {"cmd":"stop_hop"}  {"cmd":"get_stats"}

use std::io::BufRead;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use common::detector::{Config, Detector};
use common::dot11::Frame;
use common::hopper::Hopper;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::rmt::config::TransmitConfig;
use esp_idf_svc::hal::rmt::{FixedLengthSignal, PinState, Pulse, TxRmtDriver};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::{
    esp, esp_wifi_set_channel, esp_wifi_set_promiscuous, esp_wifi_set_promiscuous_filter,
    esp_wifi_set_promiscuous_rx_cb, wifi_promiscuous_filter_t, wifi_promiscuous_pkt_t,
    wifi_promiscuous_pkt_type_t, wifi_second_chan_t_WIFI_SECOND_CHAN_NONE,
    WIFI_PROMIS_FILTER_MASK_MGMT,
};
use esp_idf_svc::wifi::{ClientConfiguration, Configuration, EspWifi};

/// GPIO of the addressable RGB LED on the ESP32-S3-DevKitC-1 (WS2812). If your board
/// uses GPIO38 instead, change this and rebuild.
const RGB_LED_GPIO: u32 = 48;

/// Detector config, updated by the start_deauth_detect command. (Xtensa has no
/// 64-bit atomics, so window is stored as u32 ms.)
static THRESHOLD: AtomicU32 = AtomicU32::new(5);
static WINDOW_MS: AtomicU32 = AtomicU32::new(1000);

/// Desired channel (applied by the control thread) and counters for get_stats.
static WANT_CHANNEL: AtomicU8 = AtomicU8::new(6);
static DEAUTH_SEEN: AtomicU32 = AtomicU32::new(0);
static ALERTS: AtomicU32 = AtomicU32::new(0);

/// Channel from the RX callback (WiFi task) to the main thread carrying the BSSID of
/// each deauth/disassoc frame. Bounded so a flood can't grow the heap unbounded.
static TX: OnceLock<Mutex<mpsc::SyncSender<[u8; 6]>>> = OnceLock::new();

/// Promiscuous RX callback: forward deauth-like frames' BSSIDs to main.
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
            if let Some(tx) = TX.get() {
                if let Ok(tx) = tx.lock() {
                    let _ = tx.try_send(f.addr3); // addr3 = BSSID
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
    log::info!("node2-detector starting: deauth detector");

    let peripherals = Peripherals::take()?;

    // --- RGB LED (WS2812) on the RMT peripheral ---
    let mut led = TxRmtDriver::new(
        peripherals.rmt.channel0,
        peripherals.pins.gpio48,
        &TransmitConfig::new().clock_divider(1),
    )?;
    let _ = RGB_LED_GPIO; // documented pin; gpio48 is bound above
    set_led(&mut led, 0, 0, 0); // off

    // --- WiFi promiscuous, management frames only ---
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = EspWifi::new(peripherals.modem, sysloop, Some(nvs))?;
    wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;
    wifi.start()?;

    let (tx, rx) = mpsc::sync_channel::<[u8; 6]>(128);
    let _ = TX.set(Mutex::new(tx));

    unsafe {
        let filter = wifi_promiscuous_filter_t {
            filter_mask: WIFI_PROMIS_FILTER_MASK_MGMT,
        };
        esp!(esp_wifi_set_promiscuous_filter(&filter))?;
        esp!(esp_wifi_set_promiscuous_rx_cb(Some(rx_cb)))?;
        esp!(esp_wifi_set_promiscuous(true))?;
    }
    // Hop the common channels so an attack on any of them is seen.
    set_hop(Some(Hopper::new(&[1, 6, 11], 300)));
    apply_channel(WANT_CHANNEL.load(Ordering::Relaxed));
    log::info!("promiscuous deauth detector running");

    spawn_command_thread();
    spawn_control_thread();
    spawn_stats_thread();

    // Main loop: run the detector on incoming BSSIDs, emit alerts, and drive the
    // blue heartbeat LED. We poll the channel with a timeout so the LED keeps
    // blinking even when no frames arrive.
    let start = Instant::now();
    let mut detector = Detector::new(Config {
        threshold: THRESHOLD.load(Ordering::Relaxed),
        window_ms: WINDOW_MS.load(Ordering::Relaxed) as u64,
    });
    let mut last_cfg = (THRESHOLD.load(Ordering::Relaxed), WINDOW_MS.load(Ordering::Relaxed));
    let mut led_on = false;
    let mut last_blink = 0u64;
    let mut flash_until = 0u64; // bright-flash deadline; owned by this thread

    loop {
        let now_ms = start.elapsed().as_millis() as u64;

        // Rebuild the detector if the command thread changed the config.
        let cfg = (THRESHOLD.load(Ordering::Relaxed), WINDOW_MS.load(Ordering::Relaxed));
        if cfg != last_cfg {
            detector = Detector::new(Config { threshold: cfg.0, window_ms: cfg.1 as u64 });
            last_cfg = cfg;
        }

        // Drain any pending deauth frames (non-blocking) and run detection.
        while let Ok(bssid) = rx.try_recv() {
            DEAUTH_SEEN.fetch_add(1, Ordering::Relaxed);
            if let Some(alert) = detector.observe(bssid, now_ms) {
                ALERTS.fetch_add(1, Ordering::Relaxed);
                emit_alert(&alert.bssid, alert.count);
                flash_until = now_ms + 400; // bright flash 400ms
            }
        }

        // LED: bright steady blue during an alert flash, else a gentle blue heartbeat.
        if now_ms < flash_until {
            set_led(&mut led, 0, 0, 255); // full blue
        } else {
            // Heartbeat: toggle dim blue every 500ms.
            if now_ms.saturating_sub(last_blink) >= 500 {
                last_blink = now_ms;
                led_on = !led_on;
                if led_on {
                    set_led(&mut led, 0, 0, 40); // dim blue
                } else {
                    set_led(&mut led, 0, 0, 0); // off
                }
            }
        }

        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Write one GRB color to the WS2812 via RMT. Colors are (r,g,b); WS2812 wants GRB.
fn set_led(led: &mut TxRmtDriver, r: u8, g: u8, b: u8) {
    let color: u32 = ((g as u32) << 16) | ((r as u32) << 8) | (b as u32);
    let ticks_hz = match led.counter_clock() {
        Ok(h) => h,
        Err(_) => return,
    };
    // WS2812 bit timings.
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

/// Reads NDJSON commands from stdin (USB serial).
fn spawn_command_thread() {
    std::thread::spawn(|| {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { continue };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
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
                        emit_log("info", "set_channel");
                    }
                }
                Some("start_hop") => {
                    let dwell = v.get("dwell_ms").and_then(|d| d.as_u64()).unwrap_or(300) as u32;
                    let channels: Vec<u8> = v
                        .get("channels")
                        .and_then(|c| c.as_array())
                        .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u8)).collect())
                        .unwrap_or_else(|| vec![1, 6, 11]);
                    set_hop(Some(Hopper::new(&channels, dwell)));
                    emit_log("info", "start_hop");
                }
                Some("stop_hop") => {
                    set_hop(None);
                    emit_log("info", "stop_hop");
                }
                Some("get_stats") => emit_stats(),
                _ => {}
            }
        }
    });
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
        emit_stats();
    });
}

fn emit_stats() {
    let heap = unsafe { esp_idf_svc::sys::esp_get_free_heap_size() };
    println!(
        "{{\"ev\":\"stats\",\"pps\":{},\"dropped\":0,\"heap\":{}}}",
        DEAUTH_SEEN.load(Ordering::Relaxed),
        heap
    );
}

fn emit_alert(bssid: &[u8; 6], count: u32) {
    let b = common::dot11::fmt_mac(bssid);
    println!(
        "{{\"ev\":\"deauth_alert\",\"bssid\":\"{}\",\"count\":{}}}",
        b.as_str(),
        count
    );
}

fn emit_log(level: &str, msg: &str) {
    let safe: String = msg.chars().filter(|c| *c != '"' && *c != '\\').collect();
    println!("{{\"ev\":\"log\",\"level\":\"{level}\",\"msg\":\"{safe}\"}}");
}
