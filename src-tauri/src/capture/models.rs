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
