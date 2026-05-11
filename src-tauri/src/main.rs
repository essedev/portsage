// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if portsage_lib::is_headless_argv(&args) {
        portsage_lib::run_headless();
    } else {
        portsage_lib::run();
    }
}
