use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CaptureDevice {
    pub name: String,
    pub description: Option<String>,
    pub addresses: Vec<String>,
    pub is_loopback: bool,
    pub has_ipv4: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CaptureStatus {
    pub available: bool,
    pub running: bool,
    pub selected_device: Option<String>,
    pub filter: Option<String>,
    pub packets_total: u64,
    pub bytes_total: u64,
    pub packets_per_second: f64,
    pub bytes_per_second: f64,
    pub started_at: Option<String>,
    pub last_packet_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CaptureStartRequest {
    pub device_name: String,
    pub filter: Option<String>,
}

impl CaptureStatus {
    pub fn initial() -> Self {
        Self {
            available: false,
            running: false,
            selected_device: None,
            filter: None,
            packets_total: 0,
            bytes_total: 0,
            packets_per_second: 0.0,
            bytes_per_second: 0.0,
            started_at: None,
            last_packet_at: None,
            last_error: None,
        }
    }
}
