// ESP-IDF build integration. Runs the C/IDF build and links it into the Rust binary.
fn main() {
    embuild::espidf::sysenv::output();
}
