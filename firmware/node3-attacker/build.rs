// ESP-IDF build integration + rebuild when compile-time WiFi/WS config changes.
fn main() {
    for k in ["WIFI_SSID", "WIFI_PASSWORD", "OWN_BSSID", "WS_URL"] {
        println!("cargo:rerun-if-env-changed={k}");
    }
    embuild::espidf::sysenv::output();
}
