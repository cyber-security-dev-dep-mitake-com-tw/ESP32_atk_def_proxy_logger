//! Node1 — Packet Monitor + PCAP source (ESP32-S3, native USB).
//!
//! Puts the ESP32 WiFi into promiscuous mode, and for every captured 802.11 frame
//! emits a base64 NDJSON `packet` event (radiotap header + frame) over the native
//! USB serial console. The PC-side Go agent wraps these into a PCAP file.
//!
//! It also reads NDJSON commands from the same serial link:
//!   {"cmd":"set_channel","ch":6}
//!   {"cmd":"start_hop","dwell_ms":250,"channels":[1,6,11]}
//!   {"cmd":"stop_hop"}
//!   {"cmd":"get_stats"}
//!
//! The hardware-independent pieces (radiotap building, channel hopping) live in the
//! `common` crate and are unit-tested on the host.

use std::io::BufRead;
use std::sync::mpsc;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine;
use common::hopper::Hopper;
use common::radiotap;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::{
    esp, esp_wifi_set_channel, esp_wifi_set_promiscuous, esp_wifi_set_promiscuous_filter,
    esp_wifi_set_promiscuous_rx_cb, wifi_promiscuous_filter_t, wifi_promiscuous_pkt_t,
    wifi_promiscuous_pkt_type_t, wifi_second_chan_t_WIFI_SECOND_CHAN_NONE,
    WIFI_PROMIS_FILTER_MASK_DATA, WIFI_PROMIS_FILTER_MASK_MGMT,
};
use esp_idf_svc::wifi::{ClientConfiguration, Configuration, EspWifi};

/// One frame handed from the RX callback to the printing loop.
struct Captured {
    ch: u8,
    rssi: i8,
    frame: Vec<u8>,
}

/// Bounded channel from the WiFi RX callback to main. Bounded so a burst can't grow
/// the heap without limit — excess frames are dropped and counted.
static TX: OnceLock<Mutex<mpsc::SyncSender<Captured>>> = OnceLock::new();

/// Total frames captured and frames dropped (queue full), for `get_stats`.
static CAPTURED: AtomicU32 = AtomicU32::new(0);
static DROPPED: AtomicU32 = AtomicU32::new(0);

/// Desired channel the control thread should apply, and a generation counter the
/// control thread bumps whenever the channel/hop config changes.
static WANT_CHANNEL: AtomicU8 = AtomicU8::new(6);

/// The promiscuous RX callback. Runs in the WiFi task context (not an ISR), so heap
/// allocation is allowed. It only copies the frame and hands it off — all formatting
/// happens on the main thread to keep this fast.
unsafe extern "C" fn rx_cb(buf: *mut core::ffi::c_void, _pkt_type: wifi_promiscuous_pkt_type_t) {
    // The promiscuous filter is set to MGMT|DATA, so MISC frames never arrive here.
    if buf.is_null() {
        return;
    }
    let pkt = buf as *const wifi_promiscuous_pkt_t;
    let rx_ctrl = (*pkt).rx_ctrl;
    let sig_len = rx_ctrl.sig_len() as usize;
    if sig_len == 0 {
        return;
    }
    let payload = core::slice::from_raw_parts((*pkt).payload.as_ptr(), sig_len);
    let cap = Captured {
        ch: rx_ctrl.channel() as u8,
        rssi: rx_ctrl.rssi() as i8,
        frame: payload.to_vec(),
    };
    if let Some(tx) = TX.get() {
        if let Ok(tx) = tx.lock() {
            // try_send: never block the WiFi task; drop + count if the consumer lags.
            if tx.try_send(cap).is_err() {
                DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Apply a 2.4 GHz channel to the radio.
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

    let (tx, rx) = mpsc::sync_channel::<Captured>(256);
    let _ = TX.set(Mutex::new(tx));

    // Bring up WiFi via the high-level driver (handles NVS, netif, event loop), then
    // drop to the raw promiscuous API on top of the running radio.
    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = EspWifi::new(peripherals.modem, sysloop, Some(nvs))?;
    // A default (unconnected) client config just powers the radio on; we never call
    // connect() — promiscuous mode captures everything on the current channel.
    wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;
    wifi.start()?;
    log::info!("wifi started; enabling promiscuous mode");

    unsafe {
        let filter = wifi_promiscuous_filter_t {
            filter_mask: WIFI_PROMIS_FILTER_MASK_MGMT | WIFI_PROMIS_FILTER_MASK_DATA,
        };
        esp!(esp_wifi_set_promiscuous_filter(&filter))?;
        esp!(esp_wifi_set_promiscuous_rx_cb(Some(rx_cb)))?;
        esp!(esp_wifi_set_promiscuous(true))?;
    }
    apply_channel(WANT_CHANNEL.load(Ordering::Relaxed));
    log::info!("promiscuous monitor running on channel {}", WANT_CHANNEL.load(Ordering::Relaxed));

    spawn_command_thread();
    spawn_control_thread();
    spawn_stats_thread();

    // Main loop: format captured frames as NDJSON packet events on stdout (USB serial).
    let engine = base64::engine::general_purpose::STANDARD;
    let mut line = String::with_capacity(2048);
    for cap in rx {
        CAPTURED.fetch_add(1, Ordering::Relaxed);
        let rt = radiotap::build(cap.ch, cap.rssi);
        let mut framed = Vec::with_capacity(rt.len() + cap.frame.len());
        framed.extend_from_slice(&rt);
        framed.extend_from_slice(&cap.frame);
        let b64 = engine.encode(&framed);

        line.clear();
        use std::fmt::Write;
        let _ = write!(
            line,
            "{{\"ev\":\"packet\",\"ch\":{},\"rssi\":{},\"len\":{},\"raw\":\"{}\"}}",
            cap.ch,
            cap.rssi,
            cap.frame.len(),
            b64
        );
        println!("{line}");
    }
    Ok(())
}

/// Reads NDJSON command lines from stdin (the USB serial link) and applies them.
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
                Some("set_channel") => {
                    if let Some(ch) = v.get("ch").and_then(|c| c.as_u64()) {
                        set_hop(None);
                        WANT_CHANNEL.store(ch as u8, Ordering::Relaxed);
                        emit_log("info", &format!("set_channel {ch}"));
                    }
                }
                Some("start_hop") => {
                    let dwell = v.get("dwell_ms").and_then(|d| d.as_u64()).unwrap_or(250) as u32;
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
                Some("start_monitor") | Some("stop_monitor") => {
                    // Monitoring is always on for this node; acknowledge for symmetry.
                    emit_log("info", "monitor is always active on node1");
                }
                Some("get_stats") => emit_stats(),
                _ => {}
            }
        }
    });
}

/// Optional channel hopper shared between the command and control threads.
static HOPPER: OnceLock<Mutex<Option<Hopper>>> = OnceLock::new();

fn hopper_cell() -> &'static Mutex<Option<Hopper>> {
    HOPPER.get_or_init(|| Mutex::new(None))
}

fn set_hop(h: Option<Hopper>) {
    if let Ok(mut guard) = hopper_cell().lock() {
        *guard = h;
    }
}

/// Applies the desired channel and advances the hopper on a timer.
fn spawn_control_thread() {
    std::thread::spawn(|| {
        let start = Instant::now();
        let mut last_applied = 0u8;
        loop {
            let now_ms = start.elapsed().as_millis() as u64;
            // Advance the hopper (if any) and reflect its channel into WANT_CHANNEL.
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

/// Emits a periodic stats event so the PC sees liveness + packet rate.
fn spawn_stats_thread() {
    std::thread::spawn(|| {
        let mut last = 0u32;
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let total = CAPTURED.load(Ordering::Relaxed);
            let pps = (total.wrapping_sub(last)) / 5;
            last = total;
            emit_stats_with_pps(pps);
        }
    });
}

fn emit_stats() {
    emit_stats_with_pps(0);
}

fn emit_stats_with_pps(pps: u32) {
    let heap = unsafe { esp_idf_svc::sys::esp_get_free_heap_size() };
    println!(
        "{{\"ev\":\"stats\",\"pps\":{},\"dropped\":{},\"heap\":{}}}",
        pps,
        DROPPED.load(Ordering::Relaxed),
        heap
    );
}

fn emit_log(level: &str, msg: &str) {
    // Keep it valid JSON even if msg contains quotes by escaping minimally.
    let safe: String = msg.chars().filter(|c| *c != '"' && *c != '\\').collect();
    println!("{{\"ev\":\"log\",\"level\":\"{level}\",\"msg\":\"{safe}\"}}");
}
