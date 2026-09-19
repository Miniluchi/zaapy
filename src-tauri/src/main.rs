// Zaapy is a background app: no console window should flash on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    zaapy_lib::run()
}
