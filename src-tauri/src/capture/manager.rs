use std::{
    net::IpAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(not(test))]
use pcap::{Capture, Device, Error as PcapError};

use super::{
    analyzer::FlowAggregator,
    models::{CaptureDevice, CaptureMarker, CaptureSession, CaptureStatus, NetworkFlow},
};

#[cfg(not(test))]
const SNAPLEN: i32 = 160;
#[cfg(not(test))]
const TIMEOUT_MS: i32 = 500;
const MAX_FLOW_LIMIT: usize = 100;

#[derive(Debug)]
pub struct CaptureManager {
    status: Arc<Mutex<CaptureStatus>>,
    flows: Arc<Mutex<FlowAggregator>>,
    markers: Arc<Mutex<Vec<CaptureMarker>>>,
    sessions: Arc<Mutex<Vec<CaptureSession>>>,
    running: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Default for CaptureManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(CaptureStatus::initial())),
            flows: Arc::new(Mutex::new(FlowAggregator::new(Vec::new()))),
            markers: Arc::new(Mutex::new(Vec::new())),
            sessions: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(false)),
            worker: Mutex::new(None),
        }
    }

    pub fn check_availability(&self) -> CaptureStatus {
        let mut status = self.status_snapshot();
        match list_raw_devices() {
            Ok(_) => {
                status.available = true;
                status.last_error = None;
            }
            Err(error) => {
                status.available = false;
                status.last_error = Some(error);
            }
        }
        self.replace_status(status.clone());
        status
    }

    pub fn list_devices(&self) -> Result<Vec<CaptureDevice>, String> {
        let devices = list_raw_devices()?;
        let mut mapped = devices.into_iter().map(map_device).collect::<Vec<_>>();
        mark_suggested_device(&mut mapped);

        let mut status = self.status_snapshot();
        status.available = true;
        status.last_error = if mapped.is_empty() {
            Some("Nenhuma interface de rede foi encontrada pelo Npcap.".to_string())
        } else {
            None
        };
        self.replace_status(status);

        Ok(mapped)
    }

    #[cfg(not(test))]
    pub fn start_capture(&self, device_name: String, filter: Option<String>) -> Result<(), String> {
        let filter = normalize_filter(filter);

        if device_name.trim().is_empty() {
            return Err("Escolha uma interface antes de iniciar a captura.".to_string());
        }

        if self.running.load(Ordering::SeqCst) {
            return Err("A captura ja esta iniciada.".to_string());
        }

        let device = Device::list()
            .map_err(|error| format_pcap_error("Nao foi possivel consultar interfaces", &error))?
            .into_iter()
            .find(|device| device.name == device_name)
            .ok_or_else(|| {
                "Interface inexistente. Atualize a lista e tente novamente.".to_string()
            })?;

        let local_addresses = device
            .addresses
            .iter()
            .map(|address| address.addr.to_string())
            .collect::<Vec<_>>();

        let mut capture = Capture::from_device(device)
            .map_err(|error| format_pcap_error("Erro ao preparar a interface", &error))?
            .promisc(false)
            .snaplen(SNAPLEN)
            .timeout(TIMEOUT_MS)
            .open()
            .map_err(|error| format_pcap_error("Erro ao abrir a interface", &error))?;

        if let Some(program) = filter.as_deref() {
            capture
                .filter(program, true)
                .map_err(|error| format_pcap_error("Filtro BPF invalido", &error))?;
        }

        self.begin_session(device_name, filter, local_addresses)?;

        let running = Arc::clone(&self.running);
        let status = Arc::clone(&self.status);
        let flows = Arc::clone(&self.flows);
        let handle = std::thread::spawn(move || {
            capture_loop(capture, running, status, flows);
        });

        let mut worker = self
            .worker
            .lock()
            .map_err(|_| "Falha ao acessar a thread de captura.".to_string())?;
        *worker = Some(handle);

        Ok(())
    }

    #[cfg(test)]
    pub fn start_capture(&self, device_name: String, filter: Option<String>) -> Result<(), String> {
        self.begin_session(device_name, filter, vec!["192.168.0.10".to_string()])
    }

    pub fn stop_capture(&self) -> Result<CaptureStatus, String> {
        if !self.running.load(Ordering::SeqCst) {
            let mut status = self.status_snapshot();
            status.running = false;
            status.packets_per_second = 0.0;
            status.bytes_per_second = 0.0;
            return Ok(status);
        }

        self.running.store(false, Ordering::SeqCst);

        let handle = self
            .worker
            .lock()
            .map_err(|_| "Falha ao acessar a thread de captura.".to_string())?
            .take();

        if let Some(handle) = handle {
            handle
                .join()
                .map_err(|_| "A thread de captura encerrou com erro.".to_string())?;
        }

        let mut status = self.status_snapshot();
        finish_status(&mut status);
        self.update_current_session(&status);
        self.replace_status(status.clone());

        Ok(status)
    }

    pub fn status_snapshot(&self) -> CaptureStatus {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_else(|_| CaptureStatus {
                last_error: Some("Falha ao acessar o estado da captura.".to_string()),
                ..CaptureStatus::initial()
            })
    }

    pub fn list_flows(&self, limit: Option<usize>) -> Vec<NetworkFlow> {
        let limit = limit.unwrap_or(50).clamp(1, MAX_FLOW_LIMIT);
        let now = now_seconds();
        self.flows
            .lock()
            .map(|flows| flows.list_flows(limit, now))
            .unwrap_or_default()
    }

    pub fn add_marker(&self, description: String) -> Result<CaptureMarker, String> {
        let description = description.trim();
        if description.is_empty() {
            return Err("Informe uma descricao para o marcador.".to_string());
        }

        let status = self.status_snapshot();
        let session_id = status
            .session_id
            .clone()
            .ok_or_else(|| "Inicie uma captura antes de adicionar marcadores.".to_string())?;
        let marker = CaptureMarker {
            session_id,
            timestamp: timestamp(),
            description: description.to_string(),
            packets_total: status.packets_total,
            bytes_total: status.bytes_total,
        };

        self.markers
            .lock()
            .map_err(|_| "Falha ao acessar marcadores.".to_string())?
            .push(marker.clone());

        Ok(marker)
    }

    pub fn list_markers(&self) -> Vec<CaptureMarker> {
        let session_id = self.status_snapshot().session_id;
        self.markers
            .lock()
            .map(|markers| {
                markers
                    .iter()
                    .filter(|marker| Some(&marker.session_id) == session_id.as_ref())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn begin_session(
        &self,
        device_name: String,
        filter: Option<String>,
        local_addresses: Vec<String>,
    ) -> Result<(), String> {
        let filter = normalize_filter(filter);

        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("A captura ja esta iniciada.".to_string());
        }

        let session_id = new_session_id();
        let started_at = timestamp();

        if let Ok(mut flows) = self.flows.lock() {
            flows.reset(local_addresses);
        }

        if let Ok(mut markers) = self.markers.lock() {
            markers.retain(|marker| marker.session_id != session_id);
        }

        let status = CaptureStatus {
            available: true,
            running: true,
            session_id: Some(session_id.clone()),
            selected_device: Some(device_name.clone()),
            filter: filter.clone(),
            packets_total: 0,
            bytes_total: 0,
            packets_per_second: 0.0,
            bytes_per_second: 0.0,
            unclassified_packets: 0,
            started_at: Some(started_at.clone()),
            stopped_at: None,
            duration_seconds: None,
            last_packet_at: None,
            last_error: None,
        };

        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.push(CaptureSession {
                session_id,
                started_at,
                stopped_at: None,
                duration_seconds: None,
                selected_device: device_name,
                filter,
                packets_total: 0,
                bytes_total: 0,
            });
        }

        self.replace_status(status);
        Ok(())
    }

    fn replace_status(&self, new_status: CaptureStatus) {
        if let Ok(mut status) = self.status.lock() {
            *status = new_status;
        }
    }

    fn update_current_session(&self, status: &CaptureStatus) {
        let Some(session_id) = status.session_id.as_ref() else {
            return;
        };

        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(session) = sessions
                .iter_mut()
                .find(|session| &session.session_id == session_id)
            {
                session.stopped_at = status.stopped_at.clone();
                session.duration_seconds = status.duration_seconds;
                session.packets_total = status.packets_total;
                session.bytes_total = status.bytes_total;
            }
        }
    }

    #[cfg(test)]
    fn record_packet_for_test(&self, data: &[u8], bytes: u64, elapsed: Duration) {
        let now = now_seconds();
        let fake_started_at = now - elapsed.as_secs_f64();
        if let Ok(mut flows) = self.flows.lock() {
            if !flows.record_packet(data, bytes, now) {
                if let Ok(mut status) = self.status.lock() {
                    status.unclassified_packets = status.unclassified_packets.saturating_add(1);
                }
            }
        }

        if let Ok(mut status) = self.status.lock() {
            let mut rate = RateWindow::new();
            rate.started_at = Instant::now()
                .checked_sub(elapsed)
                .unwrap_or_else(Instant::now);
            record_packet(&mut status, bytes, &mut rate);
            status.started_at = Some(format!("{fake_started_at:.3}"));
        }
    }
}

impl Drop for CaptureManager {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(handle) = worker.take() {
                let _ = handle.join();
            }
        }
    }
}

#[derive(Debug)]
struct RateWindow {
    started_at: Instant,
    packets: u64,
    bytes: u64,
}

impl RateWindow {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            packets: 0,
            bytes: 0,
        }
    }
}

#[cfg(not(test))]
fn capture_loop(
    mut capture: pcap::Capture<pcap::Active>,
    running: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
    flows: Arc<Mutex<FlowAggregator>>,
) {
    let mut rate = RateWindow::new();

    while running.load(Ordering::SeqCst) {
        match capture.next_packet() {
            Ok(packet) => {
                let bytes = u64::from(packet.header.len);
                let now = now_seconds();
                let classified = flows
                    .lock()
                    .map(|mut flows| flows.record_packet(packet.data, bytes, now))
                    .unwrap_or(false);

                if let Ok(mut status) = status.lock() {
                    record_packet(&mut status, bytes, &mut rate);
                    if !classified {
                        status.unclassified_packets = status.unclassified_packets.saturating_add(1);
                    }
                }
            }
            Err(PcapError::TimeoutExpired) => {
                refresh_rates(&status, &mut rate);
            }
            Err(error) => {
                if let Ok(mut status) = status.lock() {
                    status.last_error = Some(format_pcap_error("Erro durante a captura", &error));
                    finish_status(&mut status);
                }
                running.store(false, Ordering::SeqCst);
            }
        }
    }

    if let Ok(mut status) = status.lock() {
        finish_status(&mut status);
    }
}

fn record_packet(status: &mut CaptureStatus, bytes: u64, rate: &mut RateWindow) {
    status.packets_total = status.packets_total.saturating_add(1);
    status.bytes_total = status.bytes_total.saturating_add(bytes);
    status.last_packet_at = Some(timestamp());

    rate.packets = rate.packets.saturating_add(1);
    rate.bytes = rate.bytes.saturating_add(bytes);

    let elapsed = rate.started_at.elapsed();
    if elapsed >= Duration::from_secs(1) {
        let seconds = elapsed.as_secs_f64().max(1.0);
        status.packets_per_second = finite_or_zero(rate.packets as f64 / seconds);
        status.bytes_per_second = finite_or_zero(rate.bytes as f64 / seconds);
        rate.started_at = Instant::now();
        rate.packets = 0;
        rate.bytes = 0;
    }
}

#[cfg(not(test))]
fn refresh_rates(status: &Arc<Mutex<CaptureStatus>>, rate: &mut RateWindow) {
    if rate.started_at.elapsed() >= Duration::from_secs(1) {
        if let Ok(mut status) = status.lock() {
            status.packets_per_second = 0.0;
            status.bytes_per_second = 0.0;
        }
        rate.started_at = Instant::now();
        rate.packets = 0;
        rate.bytes = 0;
    }
}

fn finish_status(status: &mut CaptureStatus) {
    status.running = false;
    status.packets_per_second = 0.0;
    status.bytes_per_second = 0.0;
    status.stopped_at = status.stopped_at.clone().or_else(|| Some(timestamp()));
    status.duration_seconds = duration_from_started(status.started_at.as_deref());
}

#[cfg(not(test))]
fn list_raw_devices() -> Result<Vec<Device>, String> {
    Device::list().map_err(|error| format_pcap_error("Nao foi possivel listar interfaces", &error))
}

#[cfg(test)]
fn list_raw_devices() -> Result<Vec<TestDevice>, String> {
    Ok(vec![
        TestDevice {
            name: "virtualbox".to_string(),
            desc: Some("VirtualBox Host-Only Ethernet Adapter".to_string()),
            addresses: vec!["192.168.56.1".parse().expect("valid test IP")],
        },
        TestDevice {
            name: "realtek".to_string(),
            desc: Some("Realtek PCIe GbE Family Controller".to_string()),
            addresses: vec!["192.168.0.10".parse().expect("valid test IP")],
        },
    ])
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct TestDevice {
    name: String,
    desc: Option<String>,
    addresses: Vec<IpAddr>,
}

trait CaptureDeviceSource {
    fn name(&self) -> &str;
    fn description(&self) -> Option<&String>;
    fn addresses(&self) -> Vec<IpAddr>;
}

#[cfg(not(test))]
impl CaptureDeviceSource for Device {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> Option<&String> {
        self.desc.as_ref()
    }

    fn addresses(&self) -> Vec<IpAddr> {
        self.addresses.iter().map(|address| address.addr).collect()
    }
}

#[cfg(test)]
impl CaptureDeviceSource for TestDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> Option<&String> {
        self.desc.as_ref()
    }

    fn addresses(&self) -> Vec<IpAddr> {
        self.addresses.clone()
    }
}

fn map_device(device: impl CaptureDeviceSource) -> CaptureDevice {
    let addresses = device
        .addresses()
        .into_iter()
        .map(|address| address.to_string())
        .collect::<Vec<_>>();
    let has_ipv4 = addresses
        .iter()
        .filter_map(|address| address.parse::<IpAddr>().ok())
        .any(|address| matches!(address, IpAddr::V4(ip) if !ip.is_loopback()));
    let description = device.description().cloned();
    let text = format!(
        "{} {}",
        device.name().to_lowercase(),
        description.clone().unwrap_or_default().to_lowercase()
    );

    CaptureDevice {
        name: device.name().to_string(),
        description,
        addresses,
        is_loopback: text.contains("loopback"),
        has_ipv4,
        is_virtual: is_virtual_adapter_text(&text),
        is_suggested: false,
    }
}

fn mark_suggested_device(devices: &mut [CaptureDevice]) {
    let suggested_name = devices
        .iter()
        .find(|device| device.has_ipv4 && !device.is_loopback && !device.is_virtual)
        .or_else(|| {
            devices
                .iter()
                .find(|device| device.has_ipv4 && !device.is_loopback)
        })
        .map(|device| device.name.clone());

    if let Some(name) = suggested_name {
        for device in devices {
            device.is_suggested = device.name == name;
        }
    }
}

fn is_virtual_adapter_text(text: &str) -> bool {
    [
        "virtualbox",
        "vmware",
        "hyper-v",
        "hyper v",
        "tap",
        "npcap loopback",
        "loopback",
        "vethernet",
        "wsl",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn normalize_filter(filter: Option<String>) -> Option<String> {
    filter
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn new_session_id() -> String {
    format!("session-{}", timestamp().replace('.', "-"))
}

fn timestamp() -> String {
    format!("{:.3}", now_seconds())
}

fn now_seconds() -> f64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(_) => 0.0,
    }
}

fn duration_from_started(started_at: Option<&str>) -> Option<f64> {
    let started = started_at?.parse::<f64>().ok()?;
    Some(finite_or_zero(now_seconds() - started).max(0.0))
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(not(test))]
fn format_pcap_error(context: &str, error: &PcapError) -> String {
    let details = error.to_string();
    let lower = details.to_lowercase();
    let hint = if lower.contains("permission")
        || lower.contains("access")
        || lower.contains("denied")
        || lower.contains("admin")
    {
        " Talvez seja necessario executar o aplicativo como administrador, dependendo da configuracao do Npcap."
    } else if lower.contains("packet.dll") || lower.contains("wpcap") || lower.contains("npcap") {
        " Verifique se o Npcap esta instalado e se a DLL esta disponivel no sistema."
    } else {
        ""
    };

    format!("{context}: {details}.{hint}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const UDP_PACKET: &[u8] = &[
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08, 0x00, 0x45, 0, 0, 40, 0, 0, 0, 0, 64, 17, 0, 0,
        192, 168, 0, 10, 8, 8, 8, 8, 0xd6, 0xd8, 0x13, 0xbf, 0, 8, 0, 0,
    ];

    #[test]
    fn initial_state_is_not_running() {
        let manager = CaptureManager::new();
        let status = manager.status_snapshot();

        assert!(!status.available);
        assert!(!status.running);
        assert_eq!(status.packets_total, 0);
    }

    #[test]
    fn start_creates_new_session_and_stop_finishes_it() {
        let manager = CaptureManager::new();

        manager
            .start_capture("device-a".to_string(), Some(" udp ".to_string()))
            .expect("test state should start");
        let status = manager.status_snapshot();
        assert!(status.running);
        assert!(status.session_id.is_some());
        assert_eq!(status.filter.as_deref(), Some("udp"));

        manager.stop_capture().expect("stop should work");
        let stopped = manager.status_snapshot();
        assert!(!stopped.running);
        assert!(stopped.stopped_at.is_some());
        assert!(stopped.duration_seconds.is_some());
    }

    #[test]
    fn cannot_start_twice() {
        let manager = CaptureManager::new();

        manager
            .start_capture("device-a".to_string(), Some("udp".to_string()))
            .expect("first start should work");
        let second = manager.start_capture("device-a".to_string(), Some("udp".to_string()));

        assert!(second.is_err());
    }

    #[test]
    fn records_packet_counters_and_flow() {
        let manager = CaptureManager::new();

        manager
            .start_capture("device-a".to_string(), None)
            .expect("test state should start");
        manager.record_packet_for_test(UDP_PACKET, 64, Duration::from_secs(2));

        let status = manager.status_snapshot();
        assert_eq!(status.packets_total, 1);
        assert_eq!(status.bytes_total, 64);
        assert!(status.packets_per_second > 0.0);
        assert_eq!(manager.list_flows(Some(10)).len(), 1);
    }

    #[test]
    fn new_session_resets_counters_and_flows() {
        let manager = CaptureManager::new();
        manager
            .start_capture("device-a".to_string(), None)
            .expect("first start should work");
        manager.record_packet_for_test(UDP_PACKET, 64, Duration::from_secs(1));
        manager.stop_capture().expect("stop should work");

        manager
            .start_capture("device-a".to_string(), None)
            .expect("second start should work");

        let status = manager.status_snapshot();
        assert_eq!(status.packets_total, 0);
        assert!(manager.list_flows(Some(10)).is_empty());
    }

    #[test]
    fn list_flows_respects_limit() {
        let manager = CaptureManager::new();
        manager
            .start_capture("device-a".to_string(), None)
            .expect("start should work");
        manager.record_packet_for_test(UDP_PACKET, 64, Duration::from_secs(1));

        assert_eq!(manager.list_flows(Some(1)).len(), 1);
        assert_eq!(manager.list_flows(Some(0)).len(), 1);
    }

    #[test]
    fn virtual_adapter_is_not_preferred_when_physical_exists() {
        let manager = CaptureManager::new();
        let devices = manager.list_devices().expect("test devices should list");

        let realtek = devices
            .iter()
            .find(|device| device.name == "realtek")
            .expect("realtek test device exists");
        let virtualbox = devices
            .iter()
            .find(|device| device.name == "virtualbox")
            .expect("virtualbox test device exists");

        assert!(realtek.is_suggested);
        assert!(!virtualbox.is_suggested);
        assert!(virtualbox.is_virtual);
    }

    #[test]
    fn markers_are_in_memory_for_current_session() {
        let manager = CaptureManager::new();
        manager
            .start_capture("device-a".to_string(), None)
            .expect("start should work");

        let marker = manager
            .add_marker("Albion aberto".to_string())
            .expect("marker should be added");

        assert_eq!(marker.description, "Albion aberto");
        assert_eq!(manager.list_markers().len(), 1);
    }
}
