use tauri::State;

use super::{
    manager::CaptureManager,
    models::{
        CaptureDevice, CaptureFlowRequest, CaptureMarker, CaptureMarkerRequest,
        CaptureStartRequest, CaptureStatus, ConversationDetails, ConversationDetailsRequest,
        ConversationRequest, DatagramPayload, DatagramPayloadRequest, NetworkConversation,
        NetworkFlow, RecentDatagram, RecentDatagramsRequest,
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

#[tauri::command]
pub fn capture_get_conversations(
    manager: State<'_, CaptureManager>,
    request: Option<ConversationRequest>,
) -> Vec<NetworkConversation> {
    manager.list_conversations(request.and_then(|request| request.limit))
}

#[tauri::command]
pub fn capture_get_conversation_details(
    manager: State<'_, CaptureManager>,
    request: ConversationDetailsRequest,
) -> Result<ConversationDetails, String> {
    manager.conversation_details(request.session_id, request.conversation_id)
}

#[tauri::command]
pub fn capture_get_recent_datagrams(
    manager: State<'_, CaptureManager>,
    request: RecentDatagramsRequest,
) -> Result<Vec<RecentDatagram>, String> {
    manager.recent_datagrams(request.session_id, request.conversation_id, request.limit)
}

#[tauri::command]
pub fn capture_get_datagram_payload(
    manager: State<'_, CaptureManager>,
    request: DatagramPayloadRequest,
) -> Result<DatagramPayload, String> {
    manager.datagram_payload(
        request.session_id,
        request.conversation_id,
        request.datagram_id,
    )
}

#[tauri::command]
pub fn capture_clear_session_data(manager: State<'_, CaptureManager>) -> CaptureStatus {
    manager.clear_session_data()
}
