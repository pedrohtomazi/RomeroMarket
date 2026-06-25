use tauri::State;

use super::{
    manager::CaptureManager,
    models::{
        CaptureDevice, CaptureFlowRequest, CaptureMarker, CaptureMarkerRequest,
        CaptureStartRequest, CaptureStatus, NetworkFlow,
    },
};

#[tauri::command]
pub fn capture_check_availability(manager: State<'_, CaptureManager>) -> CaptureStatus {
    manager.check_availability()
}

#[tauri::command]
pub fn capture_list_devices(
    manager: State<'_, CaptureManager>,
) -> Result<Vec<CaptureDevice>, String> {
    manager.list_devices()
}

#[tauri::command]
pub fn capture_start(
    manager: State<'_, CaptureManager>,
    request: CaptureStartRequest,
) -> Result<(), String> {
    manager.start_capture(request.device_name, request.filter)
}

#[tauri::command]
pub fn capture_stop(manager: State<'_, CaptureManager>) -> Result<CaptureStatus, String> {
    manager.stop_capture()
}

#[tauri::command]
pub fn capture_get_status(manager: State<'_, CaptureManager>) -> CaptureStatus {
    manager.status_snapshot()
}

#[tauri::command]
pub fn capture_get_flows(
    manager: State<'_, CaptureManager>,
    request: Option<CaptureFlowRequest>,
) -> Vec<NetworkFlow> {
    manager.list_flows(request.and_then(|request| request.limit))
}

#[tauri::command]
pub fn capture_add_marker(
    manager: State<'_, CaptureManager>,
    request: CaptureMarkerRequest,
) -> Result<CaptureMarker, String> {
    manager.add_marker(request.description)
}

#[tauri::command]
pub fn capture_get_markers(manager: State<'_, CaptureManager>) -> Vec<CaptureMarker> {
    manager.list_markers()
}
