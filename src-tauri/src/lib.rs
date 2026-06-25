mod capture;

use capture::{
    commands::{
        capture_add_marker, capture_check_availability, capture_get_flows, capture_get_markers,
        capture_get_status, capture_list_devices, capture_start, capture_stop,
    },
    manager::CaptureManager,
};
use tauri::Manager;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(CaptureManager::new())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            capture_check_availability,
            capture_list_devices,
            capture_start,
            capture_stop,
            capture_get_status,
            capture_get_flows,
            capture_add_marker,
            capture_get_markers
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                if let Some(manager) = window.app_handle().try_state::<CaptureManager>() {
                    let _ = manager.stop_capture();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
