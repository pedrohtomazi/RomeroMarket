use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CaptureDevice {
    pub name: String,
    pub description: Option<String>,
    pub addresses: Vec<String>,
    pub is_loopback: bool,
    pub has_ipv4: bool,
    pub is_virtual: bool,
    pub is_suggested: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CaptureStatus {
    pub available: bool,
    pub running: bool,
    pub session_id: Option<String>,
    pub selected_device: Option<String>,
    pub filter: Option<String>,
    pub packets_total: u64,
    pub bytes_total: u64,
    pub packets_per_second: f64,
    pub bytes_per_second: f64,
    pub unclassified_packets: u64,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub duration_seconds: Option<f64>,
    pub last_packet_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CaptureStartRequest {
    pub device_name: String,
    pub filter: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CaptureFlowRequest {
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CaptureMarkerRequest {
    pub description: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConversationRequest {
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConversationDetailsRequest {
    pub session_id: String,
    pub conversation_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RecentDatagramsRequest {
    pub session_id: String,
    pub conversation_id: String,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DatagramPayloadRequest {
    pub session_id: String,
    pub conversation_id: String,
    pub datagram_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NetworkProtocol {
    #[serde(rename = "UDP")]
    Udp,
    #[serde(rename = "TCP")]
    Tcp,
    #[serde(rename = "OTHER")]
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FlowDirection {
    Outbound,
    Inbound,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NetworkFlow {
    pub protocol: NetworkProtocol,
    pub direction: FlowDirection,
    pub source_ip: String,
    pub source_port: Option<u16>,
    pub destination_ip: String,
    pub destination_port: Option<u16>,
    pub packets: u64,
    pub bytes: u64,
    pub packets_per_second: f64,
    pub bytes_per_second: f64,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub active_now: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NetworkConversation {
    pub id: String,
    pub protocol: NetworkProtocol,
    pub local_ip: String,
    pub local_port: u16,
    pub remote_ip: String,
    pub remote_port: u16,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_total: u64,
    pub bytes_total: u64,
    pub packets_per_second: f64,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub active: bool,
    pub active_after_latest_marker: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConversationDetails {
    pub conversation: NetworkConversation,
    pub duration_seconds: f64,
    pub min_datagram_size: u64,
    pub average_datagram_size: f64,
    pub max_datagram_size: u64,
    pub common_sizes: Vec<DatagramSizeBucket>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DatagramSizeBucket {
    pub size: u64,
    pub count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RecentDatagram {
    pub id: String,
    pub session_id: String,
    pub conversation_id: String,
    pub sequence_number: u64,
    pub timestamp: String,
    pub direction: FlowDirection,
    pub captured_size: u64,
    pub original_size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DatagramPayload {
    pub datagram_id: String,
    pub ethernet_header_hex: String,
    pub ip_header_hex: String,
    pub transport_header_hex: String,
    pub application_payload_hex: String,
    pub payload_truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CaptureSession {
    pub session_id: String,
    pub started_at: String,
    pub stopped_at: Option<String>,
    pub duration_seconds: Option<f64>,
    pub selected_device: String,
    pub filter: Option<String>,
    pub packets_total: u64,
    pub bytes_total: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CaptureMarker {
    pub session_id: String,
    pub timestamp: String,
    pub description: String,
    pub packets_total: u64,
    pub bytes_total: u64,
    pub conversation_diffs: Vec<ConversationMarkerDiff>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConversationMarkerDiff {
    pub conversation_id: String,
    pub endpoint: String,
    pub packets_added: u64,
    pub bytes_added: u64,
    pub is_new: bool,
    pub became_active: bool,
}

impl CaptureStatus {
    pub fn initial() -> Self {
        Self {
            available: false,
            running: false,
            session_id: None,
            selected_device: None,
            filter: None,
            packets_total: 0,
            bytes_total: 0,
            packets_per_second: 0.0,
            bytes_per_second: 0.0,
            unclassified_packets: 0,
            started_at: None,
            stopped_at: None,
            duration_seconds: None,
            last_packet_at: None,
            last_error: None,
        }
    }
}
