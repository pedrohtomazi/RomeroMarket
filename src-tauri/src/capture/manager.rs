use std::{
    net::IpAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use pcap::{Capture, Device, Error as PcapError};

use super::models::{CaptureDevice, CaptureStatus};

const SNAPLEN: i32 = 160;
const TIMEOUT_MS: i32 = 500;

#[derive(Debug)]
pub struct CaptureManager {
    status: Arc<Mutex<CaptureStatus>>,
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
            running: Arc::new(AtomicBool::new(false)),
            worker: Mutex::new(None),
        }
    }

    pub fn check_availability(&self) -> CaptureStatus {
        let mut status = self.status_snapshot();
        match Device::list() {
            Ok(_) => {
                status.available = true;
                status.last_error = None;
            }
            Err(error) => {
                status.available = false;
                status.last_error = Some(format_pcap_error("Npcap indisponivel", &error));
            }
        }
        self.replace_status(status.clone());
        status
    }

    pub fn list_devices(&self) -> Result<Vec<CaptureDevice>, String> {
        let devices = Device::list()
            .map_err(|error| format_pcap_error("Nao foi possivel listar interfaces", &error))?;

        let mapped = devices.into_iter().map(map_device).collect::<Vec<_>>();
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

        self.begin_status(device_name, filter)?;

        let running = Arc::clone(&self.running);
        let status = Arc::clone(&self.status);
        let handle = thread::spawn(move || {
            capture_loop(capture, running, status);
        });

        let mut worker = self
            .worker
            .lock()
            .map_err(|_| "Falha ao acessar a thread de captura.".to_string())?;
        *worker = Some(handle);

        Ok(())
    }

    pub fn stop_capture(&self) -> Result<CaptureStatus, String> {
        if !self.running.load(Ordering::SeqCst) {
            let mut status = self.status_snapshot();
            status.running = false;
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
        status.running = false;
        status.packets_per_second = 0.0;
        status.bytes_per_second = 0.0;
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

    fn begin_status(&self, device_name: String, filter: Option<String>) -> Result<(), String> {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("A captura ja esta iniciada.".to_string());
        }

        let mut status = self.status_snapshot();
        status.available = true;
        status.running = true;
        status.selected_device = Some(device_name);
        status.filter = filter;
        status.packets_total = 0;
        status.bytes_total = 0;
        status.packets_per_second = 0.0;
        status.bytes_per_second = 0.0;
        status.started_at = Some(timestamp());
        status.last_packet_at = None;
        status.last_error = None;
        self.replace_status(status);
        Ok(())
    }

    fn replace_status(&self, new_status: CaptureStatus) {
        if let Ok(mut status) = self.status.lock() {
            *status = new_status;
        }
    }

    #[cfg(test)]
    fn begin_status_for_test(
        &self,
        device_name: String,
        filter: Option<String>,
    ) -> Result<(), String> {
        self.begin_status(device_name, normalize_filter(filter))
    }

    #[cfg(test)]
    fn record_packet_for_test(&self, bytes: u32, elapsed: Duration) {
        let mut rate = RateWindow::new();
        rate.started_at = Instant::now()
            .checked_sub(elapsed)
            .unwrap_or_else(Instant::now);
        if let Ok(mut status) = self.status.lock() {
            record_packet(&mut status, bytes, &mut rate);
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

fn capture_loop(
    mut capture: pcap::Capture<pcap::Active>,
    running: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) {
    let mut rate = RateWindow::new();

    while running.load(Ordering::SeqCst) {
        match capture.next_packet() {
            Ok(packet) => {
                if let Ok(mut status) = status.lock() {
                    record_packet(&mut status, packet.header.len, &mut rate);
                }
            }
            Err(PcapError::TimeoutExpired) => {
                refresh_rates(&status, &mut rate);
            }
            Err(error) => {
                if let Ok(mut status) = status.lock() {
                    status.last_error = Some(format_pcap_error("Erro durante a captura", &error));
                    status.running = false;
                }
                running.store(false, Ordering::SeqCst);
            }
        }
    }

    if let Ok(mut status) = status.lock() {
        status.running = false;
        status.packets_per_second = 0.0;
        status.bytes_per_second = 0.0;
    }
}

fn record_packet(status: &mut CaptureStatus, bytes: u32, rate: &mut RateWindow) {
    status.packets_total = status.packets_total.saturating_add(1);
    status.bytes_total = status.bytes_total.saturating_add(u64::from(bytes));
    status.last_packet_at = Some(timestamp());

    rate.packets = rate.packets.saturating_add(1);
    rate.bytes = rate.bytes.saturating_add(u64::from(bytes));

    let elapsed = rate.started_at.elapsed();
    if elapsed >= Duration::from_secs(1) {
        let seconds = elapsed.as_secs_f64();
        status.packets_per_second = rate.packets as f64 / seconds;
        status.bytes_per_second = rate.bytes as f64 / seconds;
        rate.started_at = Instant::now();
        rate.packets = 0;
        rate.bytes = 0;
    }
}

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

fn map_device(device: Device) -> CaptureDevice {
    let addresses = device
        .addresses
        .iter()
        .map(|address| address.addr.to_string())
        .collect::<Vec<_>>();
    let has_ipv4 = device
        .addresses
        .iter()
        .any(|address| matches!(address.addr, IpAddr::V4(ip) if !ip.is_loopback()));
    let description = device.desc.clone();
    let text = format!(
        "{} {}",
        device.name.to_lowercase(),
        description.clone().unwrap_or_default().to_lowercase()
    );

    CaptureDevice {
        name: device.name,
        description,
        addresses,
        is_loopback: text.contains("loopback"),
        has_ipv4,
    }
}

fn normalize_filter(filter: Option<String>) -> Option<String> {
    filter
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn timestamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{}.{:03}", duration.as_secs(), duration.subsec_millis()),
        Err(_) => "0.000".to_string(),
    }
}

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

    #[test]
    fn initial_state_is_not_running() {
        let manager = CaptureManager::new();
        let status = manager.status_snapshot();

        assert!(!status.available);
        assert!(!status.running);
        assert_eq!(status.packets_total, 0);
    }

    #[test]
    fn begin_status_moves_to_running() {
        let manager = CaptureManager::new();

        manager
            .begin_status_for_test("device-a".to_string(), Some(" udp ".to_string()))
            .expect("test state should start");

        let status = manager.status_snapshot();
        assert!(status.running);
        assert_eq!(status.selected_device.as_deref(), Some("device-a"));
        assert_eq!(status.filter.as_deref(), Some("udp"));
    }

    #[test]
    fn cannot_start_twice() {
        let manager = CaptureManager::new();

        manager
            .begin_status_for_test("device-a".to_string(), Some("udp".to_string()))
            .expect("first start should work");
        let second = manager.begin_status_for_test("device-a".to_string(), Some("udp".to_string()));

        assert!(second.is_err());
    }

    #[test]
    fn stop_updates_running_flag() {
        let manager = CaptureManager::new();

        manager
            .begin_status_for_test("device-a".to_string(), None)
            .expect("test state should start");
        manager.stop_capture().expect("stop should work");

        assert!(!manager.status_snapshot().running);
    }

    #[test]
    fn records_packet_counters_and_rates() {
        let manager = CaptureManager::new();

        manager
            .begin_status_for_test("device-a".to_string(), None)
            .expect("test state should start");
        manager.record_packet_for_test(500, Duration::from_secs(2));

        let status = manager.status_snapshot();
        assert_eq!(status.packets_total, 1);
        assert_eq!(status.bytes_total, 500);
        assert!(status.packets_per_second > 0.0);
        assert!(status.bytes_per_second > 0.0);
    }

    #[test]
    fn empty_filter_is_removed() {
        assert_eq!(normalize_filter(Some("   ".to_string())), None);
        assert_eq!(
            normalize_filter(Some(" udp ".to_string())),
            Some("udp".to_string())
        );
    }

    #[test]
    fn models_are_serializable() {
        let device = CaptureDevice {
            name: "dev".to_string(),
            description: Some("Adapter".to_string()),
            addresses: vec!["192.168.0.2".to_string()],
            is_loopback: false,
            has_ipv4: true,
        };

        let json = serde_json::to_string(&device).expect("device should serialize");

        assert!(json.contains("Adapter"));
    }
}
