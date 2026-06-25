use std::{
    collections::{HashMap, VecDeque},
    net::{Ipv4Addr, Ipv6Addr},
    ops::Range,
};

use super::models::{
    ConversationDetails, DatagramPayload, DatagramSizeBucket, FlowDirection, NetworkConversation,
    NetworkFlow, NetworkProtocol, RecentDatagram,
};

const ETHERTYPE_IPV4: u16 = 0x0800;
const ETHERTYPE_IPV6: u16 = 0x86DD;
const ETHERTYPE_VLAN: u16 = 0x8100;
const ETHERTYPE_QINQ: u16 = 0x88A8;
const IP_PROTO_TCP: u8 = 6;
const IP_PROTO_UDP: u8 = 17;
const ACTIVE_SECONDS: f64 = 3.0;
const RATE_WINDOW_SECONDS: f64 = 3.0;
const MAX_GLOBAL_DATAGRAMS: usize = 2_000;
const MAX_CONVERSATION_DATAGRAMS: usize = 500;
const MAX_PAYLOAD_BYTES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FlowKey {
    protocol: NetworkProtocol,
    source_ip: String,
    source_port: Option<u16>,
    destination_ip: String,
    destination_port: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ConversationKey {
    protocol: NetworkProtocol,
    local_ip: String,
    local_port: u16,
    remote_ip: String,
    remote_port: u16,
}

#[derive(Clone, Debug)]
pub struct ParsedPacket {
    protocol: NetworkProtocol,
    source_ip: String,
    source_port: Option<u16>,
    destination_ip: String,
    destination_port: Option<u16>,
    ethernet_range: Range<usize>,
    ip_range: Range<usize>,
    transport_range: Range<usize>,
    payload_range: Range<usize>,
}

#[derive(Clone, Debug)]
struct RateSample {
    at_seconds: f64,
    bytes: u64,
}

#[derive(Clone, Debug)]
struct FlowEntry {
    key: FlowKey,
    direction: FlowDirection,
    packets: u64,
    bytes: u64,
    first_seen_seconds: f64,
    last_seen_seconds: f64,
    samples: VecDeque<RateSample>,
}

#[derive(Clone, Debug)]
struct ConversationEntry {
    key: ConversationKey,
    id: String,
    packets_sent: u64,
    packets_received: u64,
    bytes_sent: u64,
    bytes_received: u64,
    first_seen_seconds: f64,
    last_seen_seconds: f64,
    samples: VecDeque<RateSample>,
    min_size: u64,
    max_size: u64,
    size_sum: u64,
    size_count: u64,
    size_buckets: HashMap<u64, u64>,
    datagram_ids: VecDeque<String>,
}

#[derive(Clone, Debug)]
struct StoredDatagram {
    id: String,
    session_id: String,
    conversation_id: String,
    sequence_number: u64,
    timestamp_seconds: f64,
    direction: FlowDirection,
    captured_size: u64,
    original_size: u64,
    ethernet_header: Vec<u8>,
    ip_header: Vec<u8>,
    transport_header: Vec<u8>,
    application_payload: Vec<u8>,
    payload_truncated: bool,
}

struct DatagramBuildInput<'a> {
    session_id: String,
    conversation_id: String,
    sequence_number: u64,
    direction: FlowDirection,
    original_size: u64,
    data: &'a [u8],
    parsed: &'a ParsedPacket,
    now_seconds: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConversationSnapshot {
    pub packets_total: u64,
    pub bytes_total: u64,
    pub last_seen_at: String,
}

#[derive(Clone, Debug)]
pub struct FlowAggregator {
    flows: HashMap<FlowKey, FlowEntry>,
    conversations: HashMap<ConversationKey, ConversationEntry>,
    datagrams: VecDeque<StoredDatagram>,
    local_addresses: Vec<String>,
    session_id: Option<String>,
    sequence_number: u64,
    latest_marker_seconds: Option<f64>,
    max_global_datagrams: usize,
    max_conversation_datagrams: usize,
}

impl Default for FlowAggregator {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl FlowAggregator {
    pub fn new(local_addresses: Vec<String>) -> Self {
        Self {
            flows: HashMap::new(),
            conversations: HashMap::new(),
            datagrams: VecDeque::new(),
            local_addresses,
            session_id: None,
            sequence_number: 0,
            latest_marker_seconds: None,
            max_global_datagrams: MAX_GLOBAL_DATAGRAMS,
            max_conversation_datagrams: MAX_CONVERSATION_DATAGRAMS,
        }
    }

    pub fn reset_session(&mut self, session_id: String, local_addresses: Vec<String>) {
        self.flows.clear();
        self.conversations.clear();
        self.datagrams.clear();
        self.local_addresses = local_addresses;
        self.session_id = Some(session_id);
        self.sequence_number = 0;
        self.latest_marker_seconds = None;
    }

    pub fn clear_session_data(&mut self) {
        self.flows.clear();
        self.conversations.clear();
        self.datagrams.clear();
        self.sequence_number = 0;
        self.latest_marker_seconds = None;
    }

    #[cfg(test)]
    pub fn with_limits(
        local_addresses: Vec<String>,
        max_global_datagrams: usize,
        max_conversation_datagrams: usize,
    ) -> Self {
        Self {
            max_global_datagrams,
            max_conversation_datagrams,
            ..Self::new(local_addresses)
        }
    }

    pub fn record_packet(&mut self, data: &[u8], original_size: u64, now_seconds: f64) -> bool {
        let Some(parsed) = parse_packet(data) else {
            return false;
        };

        self.record_flow(&parsed, original_size, now_seconds);

        if parsed.protocol == NetworkProtocol::Other {
            return true;
        }

        if let Some((key, direction)) = self.conversation_key(&parsed) {
            self.record_conversation(key, direction, &parsed, data, original_size, now_seconds);
        }

        true
    }

    pub fn list_flows(&self, limit: usize, now_seconds: f64) -> Vec<NetworkFlow> {
        let capped_limit = limit.clamp(1, 100);
        let mut flows = self
            .flows
            .values()
            .map(|entry| entry.to_model(now_seconds))
            .collect::<Vec<_>>();

        flows.sort_by(|a, b| {
            b.bytes
                .cmp(&a.bytes)
                .then_with(|| b.packets.cmp(&a.packets))
                .then_with(|| b.last_seen_at.cmp(&a.last_seen_at))
        });
        flows.truncate(capped_limit);
        flows
    }

    pub fn list_conversations(&self, limit: usize, now_seconds: f64) -> Vec<NetworkConversation> {
        let capped_limit = limit.clamp(1, 100);
        let mut conversations = self
            .conversations
            .values()
            .map(|entry| self.conversation_to_model(entry, now_seconds))
            .collect::<Vec<_>>();

        conversations.sort_by(|a, b| {
            b.bytes_total
                .cmp(&a.bytes_total)
                .then_with(|| b.packets_total.cmp(&a.packets_total))
                .then_with(|| b.last_seen_at.cmp(&a.last_seen_at))
        });
        conversations.truncate(capped_limit);
        conversations
    }

    pub fn conversation_details(
        &self,
        session_id: &str,
        conversation_id: &str,
        now_seconds: f64,
    ) -> Result<ConversationDetails, String> {
        self.ensure_session(session_id)?;
        let entry = self
            .conversations
            .values()
            .find(|entry| entry.id == conversation_id)
            .ok_or_else(|| "Conversa nao encontrada na sessao atual.".to_string())?;

        let duration_seconds = (entry.last_seen_seconds - entry.first_seen_seconds).max(0.0);
        let average = if entry.size_count > 0 {
            entry.size_sum as f64 / entry.size_count as f64
        } else {
            0.0
        };
        let mut common_sizes = entry
            .size_buckets
            .iter()
            .map(|(size, count)| DatagramSizeBucket {
                size: *size,
                count: *count,
            })
            .collect::<Vec<_>>();
        common_sizes.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.size.cmp(&b.size)));
        common_sizes.truncate(8);

        Ok(ConversationDetails {
            conversation: self.conversation_to_model(entry, now_seconds),
            duration_seconds,
            min_datagram_size: if entry.size_count > 0 {
                entry.min_size
            } else {
                0
            },
            average_datagram_size: average,
            max_datagram_size: entry.max_size,
            common_sizes,
        })
    }

    pub fn recent_datagrams(
        &self,
        session_id: &str,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<RecentDatagram>, String> {
        self.ensure_session(session_id)?;
        let capped_limit = limit.clamp(1, 100);
        let entry = self
            .conversations
            .values()
            .find(|entry| entry.id == conversation_id)
            .ok_or_else(|| "Conversa nao encontrada na sessao atual.".to_string())?;

        Ok(entry
            .datagram_ids
            .iter()
            .rev()
            .filter_map(|id| self.datagrams.iter().find(|datagram| &datagram.id == id))
            .take(capped_limit)
            .map(StoredDatagram::to_recent)
            .collect())
    }

    pub fn datagram_payload(
        &self,
        session_id: &str,
        conversation_id: &str,
        datagram_id: &str,
    ) -> Result<DatagramPayload, String> {
        self.ensure_session(session_id)?;
        let datagram = self
            .datagrams
            .iter()
            .find(|datagram| {
                datagram.id == datagram_id && datagram.conversation_id == conversation_id
            })
            .ok_or_else(|| {
                "Datagrama nao encontrado. Ele pode ter sido removido pelo buffer circular."
                    .to_string()
            })?;

        Ok(DatagramPayload {
            datagram_id: datagram.id.clone(),
            ethernet_header_hex: to_hex(&datagram.ethernet_header),
            ip_header_hex: to_hex(&datagram.ip_header),
            transport_header_hex: to_hex(&datagram.transport_header),
            application_payload_hex: to_hex(&datagram.application_payload),
            payload_truncated: datagram.payload_truncated,
        })
    }

    pub fn conversation_snapshot(&self) -> HashMap<String, ConversationSnapshot> {
        self.conversations
            .values()
            .map(|entry| {
                (
                    entry.id.clone(),
                    ConversationSnapshot {
                        packets_total: entry.packets_total(),
                        bytes_total: entry.bytes_total(),
                        last_seen_at: format_seconds(entry.last_seen_seconds),
                    },
                )
            })
            .collect()
    }

    pub fn set_latest_marker(&mut self, marker_seconds: f64) {
        self.latest_marker_seconds = Some(marker_seconds);
    }

    #[cfg(test)]
    pub fn flow_count(&self) -> usize {
        self.flows.len()
    }

    #[cfg(test)]
    pub fn conversation_count(&self) -> usize {
        self.conversations.len()
    }

    #[cfg(test)]
    pub fn datagram_count(&self) -> usize {
        self.datagrams.len()
    }

    fn ensure_session(&self, session_id: &str) -> Result<(), String> {
        if self.session_id.as_deref() == Some(session_id) {
            Ok(())
        } else {
            Err("Sessao de captura nao encontrada ou ja encerrada.".to_string())
        }
    }

    fn record_flow(&mut self, parsed: &ParsedPacket, bytes: u64, now_seconds: f64) {
        let key = FlowKey {
            protocol: parsed.protocol.clone(),
            source_ip: parsed.source_ip.clone(),
            source_port: parsed.source_port,
            destination_ip: parsed.destination_ip.clone(),
            destination_port: parsed.destination_port,
        };
        let direction = infer_direction(&key.source_ip, &key.destination_ip, &self.local_addresses);

        let entry = self.flows.entry(key.clone()).or_insert_with(|| FlowEntry {
            key,
            direction,
            packets: 0,
            bytes: 0,
            first_seen_seconds: now_seconds,
            last_seen_seconds: now_seconds,
            samples: VecDeque::new(),
        });

        entry.direction = infer_direction(
            &entry.key.source_ip,
            &entry.key.destination_ip,
            &self.local_addresses,
        );
        entry.packets = entry.packets.saturating_add(1);
        entry.bytes = entry.bytes.saturating_add(bytes);
        entry.last_seen_seconds = now_seconds;
        entry.samples.push_back(RateSample {
            at_seconds: now_seconds,
            bytes,
        });
        trim_samples(&mut entry.samples, now_seconds);
    }

    fn conversation_key(&self, parsed: &ParsedPacket) -> Option<(ConversationKey, FlowDirection)> {
        let source_port = parsed.source_port?;
        let destination_port = parsed.destination_port?;
        let direction = infer_direction(
            &parsed.source_ip,
            &parsed.destination_ip,
            &self.local_addresses,
        );

        match direction {
            FlowDirection::Outbound => Some((
                ConversationKey {
                    protocol: parsed.protocol.clone(),
                    local_ip: parsed.source_ip.clone(),
                    local_port: source_port,
                    remote_ip: parsed.destination_ip.clone(),
                    remote_port: destination_port,
                },
                direction,
            )),
            FlowDirection::Inbound => Some((
                ConversationKey {
                    protocol: parsed.protocol.clone(),
                    local_ip: parsed.destination_ip.clone(),
                    local_port: destination_port,
                    remote_ip: parsed.source_ip.clone(),
                    remote_port: source_port,
                },
                direction,
            )),
            FlowDirection::Unknown => None,
        }
    }

    fn record_conversation(
        &mut self,
        key: ConversationKey,
        direction: FlowDirection,
        parsed: &ParsedPacket,
        data: &[u8],
        original_size: u64,
        now_seconds: f64,
    ) {
        let id = conversation_id(&key);
        let entry = self
            .conversations
            .entry(key.clone())
            .or_insert_with(|| ConversationEntry::new(key, id.clone(), now_seconds));

        entry.record(direction.clone(), original_size, now_seconds);
        self.sequence_number = self.sequence_number.saturating_add(1);

        let datagram = StoredDatagram::from_packet(DatagramBuildInput {
            session_id: self.session_id.clone().unwrap_or_default(),
            conversation_id: id.clone(),
            sequence_number: self.sequence_number,
            direction,
            original_size,
            data,
            parsed,
            now_seconds,
        });
        entry.datagram_ids.push_back(datagram.id.clone());
        self.datagrams.push_back(datagram);
        self.enforce_conversation_limit(&id);
        self.enforce_global_limit();
    }

    fn conversation_to_model(
        &self,
        entry: &ConversationEntry,
        now_seconds: f64,
    ) -> NetworkConversation {
        let window_start = now_seconds - RATE_WINDOW_SECONDS;
        let packets_in_window = entry
            .samples
            .iter()
            .filter(|sample| sample.at_seconds >= window_start)
            .count() as u64;
        let packets_per_second = packets_in_window as f64 / RATE_WINDOW_SECONDS.max(1.0);
        let active = now_seconds - entry.last_seen_seconds <= ACTIVE_SECONDS;
        let active_after_latest_marker = self
            .latest_marker_seconds
            .is_some_and(|marker| entry.last_seen_seconds >= marker);

        NetworkConversation {
            id: entry.id.clone(),
            protocol: entry.key.protocol.clone(),
            local_ip: entry.key.local_ip.clone(),
            local_port: entry.key.local_port,
            remote_ip: entry.key.remote_ip.clone(),
            remote_port: entry.key.remote_port,
            packets_sent: entry.packets_sent,
            packets_received: entry.packets_received,
            bytes_sent: entry.bytes_sent,
            bytes_received: entry.bytes_received,
            packets_total: entry.packets_total(),
            bytes_total: entry.bytes_total(),
            packets_per_second,
            first_seen_at: format_seconds(entry.first_seen_seconds),
            last_seen_at: format_seconds(entry.last_seen_seconds),
            active,
            active_after_latest_marker,
        }
    }

    fn enforce_conversation_limit(&mut self, conversation_id: &str) {
        let overflow = self
            .conversations
            .values_mut()
            .find(|entry| entry.id == conversation_id)
            .map(|entry| {
                let mut removed = Vec::new();
                while entry.datagram_ids.len() > self.max_conversation_datagrams {
                    if let Some(id) = entry.datagram_ids.pop_front() {
                        removed.push(id);
                    }
                }
                removed
            })
            .unwrap_or_default();

        for id in overflow {
            self.remove_datagram(&id);
        }
    }

    fn enforce_global_limit(&mut self) {
        while self.datagrams.len() > self.max_global_datagrams {
            if let Some(datagram) = self.datagrams.pop_front() {
                self.remove_datagram_id_from_conversation(&datagram.conversation_id, &datagram.id);
            }
        }
    }

    fn remove_datagram(&mut self, datagram_id: &str) {
        if let Some(index) = self
            .datagrams
            .iter()
            .position(|datagram| datagram.id == datagram_id)
        {
            self.datagrams.remove(index);
        }
    }

    fn remove_datagram_id_from_conversation(&mut self, conversation_id: &str, datagram_id: &str) {
        if let Some(entry) = self
            .conversations
            .values_mut()
            .find(|entry| entry.id == conversation_id)
        {
            entry.datagram_ids.retain(|id| id != datagram_id);
        }
    }
}

impl ConversationEntry {
    fn new(key: ConversationKey, id: String, now_seconds: f64) -> Self {
        Self {
            key,
            id,
            packets_sent: 0,
            packets_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            first_seen_seconds: now_seconds,
            last_seen_seconds: now_seconds,
            samples: VecDeque::new(),
            min_size: u64::MAX,
            max_size: 0,
            size_sum: 0,
            size_count: 0,
            size_buckets: HashMap::new(),
            datagram_ids: VecDeque::new(),
        }
    }

    fn record(&mut self, direction: FlowDirection, bytes: u64, now_seconds: f64) {
        match direction {
            FlowDirection::Outbound => {
                self.packets_sent = self.packets_sent.saturating_add(1);
                self.bytes_sent = self.bytes_sent.saturating_add(bytes);
            }
            FlowDirection::Inbound => {
                self.packets_received = self.packets_received.saturating_add(1);
                self.bytes_received = self.bytes_received.saturating_add(bytes);
            }
            FlowDirection::Unknown => {}
        }

        self.last_seen_seconds = now_seconds;
        self.samples.push_back(RateSample {
            at_seconds: now_seconds,
            bytes,
        });
        trim_samples(&mut self.samples, now_seconds);
        self.min_size = self.min_size.min(bytes);
        self.max_size = self.max_size.max(bytes);
        self.size_sum = self.size_sum.saturating_add(bytes);
        self.size_count = self.size_count.saturating_add(1);
        *self.size_buckets.entry(bytes).or_insert(0) += 1;
    }

    fn packets_total(&self) -> u64 {
        self.packets_sent.saturating_add(self.packets_received)
    }

    fn bytes_total(&self) -> u64 {
        self.bytes_sent.saturating_add(self.bytes_received)
    }
}

impl FlowEntry {
    fn to_model(&self, now_seconds: f64) -> NetworkFlow {
        let window_start = now_seconds - RATE_WINDOW_SECONDS;
        let packets_in_window = self
            .samples
            .iter()
            .filter(|sample| sample.at_seconds >= window_start)
            .count() as u64;
        let bytes_in_window = self
            .samples
            .iter()
            .filter(|sample| sample.at_seconds >= window_start)
            .map(|sample| sample.bytes)
            .sum::<u64>();
        let divisor = RATE_WINDOW_SECONDS.max(1.0);

        NetworkFlow {
            protocol: self.key.protocol.clone(),
            direction: self.direction.clone(),
            source_ip: self.key.source_ip.clone(),
            source_port: self.key.source_port,
            destination_ip: self.key.destination_ip.clone(),
            destination_port: self.key.destination_port,
            packets: self.packets,
            bytes: self.bytes,
            packets_per_second: packets_in_window as f64 / divisor,
            bytes_per_second: bytes_in_window as f64 / divisor,
            first_seen_at: format_seconds(self.first_seen_seconds),
            last_seen_at: format_seconds(self.last_seen_seconds),
            active_now: now_seconds - self.last_seen_seconds <= ACTIVE_SECONDS,
        }
    }
}

impl StoredDatagram {
    fn from_packet(input: DatagramBuildInput<'_>) -> Self {
        let payload = slice_range(input.data, input.parsed.payload_range.clone());
        let payload_truncated = payload.len() > MAX_PAYLOAD_BYTES;

        Self {
            id: format!("{}-{}", input.session_id, input.sequence_number),
            session_id: input.session_id,
            conversation_id: input.conversation_id,
            sequence_number: input.sequence_number,
            timestamp_seconds: input.now_seconds,
            direction: input.direction,
            captured_size: input.data.len() as u64,
            original_size: input.original_size,
            ethernet_header: slice_range(input.data, input.parsed.ethernet_range.clone()).to_vec(),
            ip_header: slice_range(input.data, input.parsed.ip_range.clone()).to_vec(),
            transport_header: slice_range(input.data, input.parsed.transport_range.clone())
                .to_vec(),
            application_payload: payload
                .iter()
                .take(MAX_PAYLOAD_BYTES)
                .copied()
                .collect::<Vec<_>>(),
            payload_truncated,
        }
    }

    fn to_recent(&self) -> RecentDatagram {
        RecentDatagram {
            id: self.id.clone(),
            session_id: self.session_id.clone(),
            conversation_id: self.conversation_id.clone(),
            sequence_number: self.sequence_number,
            timestamp: format_seconds(self.timestamp_seconds),
            direction: self.direction.clone(),
            captured_size: self.captured_size,
            original_size: self.original_size,
        }
    }
}

pub fn parse_packet(data: &[u8]) -> Option<ParsedPacket> {
    let (ether_type, ethernet_range, offset) = parse_ether_type(data)?;

    match ether_type {
        ETHERTYPE_IPV4 => parse_ipv4(data, ethernet_range, offset),
        ETHERTYPE_IPV6 => parse_ipv6(data, ethernet_range, offset),
        _ => None,
    }
}

fn parse_ether_type(data: &[u8]) -> Option<(u16, Range<usize>, usize)> {
    if data.len() < 14 {
        return None;
    }

    let mut ether_type = u16::from_be_bytes([data[12], data[13]]);
    let mut offset = 14;

    while ether_type == ETHERTYPE_VLAN || ether_type == ETHERTYPE_QINQ {
        if data.len() < offset + 4 {
            return None;
        }
        ether_type = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
        offset += 4;
    }

    Some((ether_type, 0..offset, offset))
}

fn parse_ipv4(data: &[u8], ethernet_range: Range<usize>, offset: usize) -> Option<ParsedPacket> {
    if data.len() < offset + 20 {
        return None;
    }

    let version = data[offset] >> 4;
    let ihl = usize::from(data[offset] & 0x0f) * 4;
    if version != 4 || ihl < 20 || data.len() < offset + ihl {
        return None;
    }

    let protocol = data[offset + 9];
    let source_ip = Ipv4Addr::new(
        data[offset + 12],
        data[offset + 13],
        data[offset + 14],
        data[offset + 15],
    )
    .to_string();
    let destination_ip = Ipv4Addr::new(
        data[offset + 16],
        data[offset + 17],
        data[offset + 18],
        data[offset + 19],
    )
    .to_string();

    parse_transport(
        data,
        ethernet_range,
        offset..offset + ihl,
        offset + ihl,
        protocol,
        source_ip,
        destination_ip,
    )
}

fn parse_ipv6(data: &[u8], ethernet_range: Range<usize>, offset: usize) -> Option<ParsedPacket> {
    if data.len() < offset + 40 || data[offset] >> 4 != 6 {
        return None;
    }

    let protocol = data[offset + 6];
    let source_ip =
        Ipv6Addr::from(<[u8; 16]>::try_from(&data[offset + 8..offset + 24]).ok()?).to_string();
    let destination_ip =
        Ipv6Addr::from(<[u8; 16]>::try_from(&data[offset + 24..offset + 40]).ok()?).to_string();

    parse_transport(
        data,
        ethernet_range,
        offset..offset + 40,
        offset + 40,
        protocol,
        source_ip,
        destination_ip,
    )
}

fn parse_transport(
    data: &[u8],
    ethernet_range: Range<usize>,
    ip_range: Range<usize>,
    offset: usize,
    protocol: u8,
    source_ip: String,
    destination_ip: String,
) -> Option<ParsedPacket> {
    match protocol {
        IP_PROTO_UDP => {
            if data.len() < offset + 8 {
                return None;
            }
            Some(ParsedPacket {
                protocol: NetworkProtocol::Udp,
                source_ip,
                source_port: Some(u16::from_be_bytes([data[offset], data[offset + 1]])),
                destination_ip,
                destination_port: Some(u16::from_be_bytes([data[offset + 2], data[offset + 3]])),
                ethernet_range,
                ip_range,
                transport_range: offset..offset + 8,
                payload_range: offset + 8..data.len(),
            })
        }
        IP_PROTO_TCP => {
            if data.len() < offset + 20 {
                return None;
            }
            let tcp_header_len = usize::from(data[offset + 12] >> 4) * 4;
            if tcp_header_len < 20 || data.len() < offset + tcp_header_len {
                return None;
            }
            Some(ParsedPacket {
                protocol: NetworkProtocol::Tcp,
                source_ip,
                source_port: Some(u16::from_be_bytes([data[offset], data[offset + 1]])),
                destination_ip,
                destination_port: Some(u16::from_be_bytes([data[offset + 2], data[offset + 3]])),
                ethernet_range,
                ip_range,
                transport_range: offset..offset + tcp_header_len,
                payload_range: offset + tcp_header_len..data.len(),
            })
        }
        _ => Some(ParsedPacket {
            protocol: NetworkProtocol::Other,
            source_ip,
            source_port: None,
            destination_ip,
            destination_port: None,
            ethernet_range,
            ip_range,
            transport_range: offset..offset,
            payload_range: offset..offset,
        }),
    }
}

fn infer_direction(
    source_ip: &str,
    destination_ip: &str,
    local_addresses: &[String],
) -> FlowDirection {
    let source_local = local_addresses.iter().any(|address| address == source_ip);
    let destination_local = local_addresses
        .iter()
        .any(|address| address == destination_ip);

    match (source_local, destination_local) {
        (true, false) => FlowDirection::Outbound,
        (false, true) => FlowDirection::Inbound,
        _ => FlowDirection::Unknown,
    }
}

fn trim_samples(samples: &mut VecDeque<RateSample>, now_seconds: f64) {
    let cutoff = now_seconds - RATE_WINDOW_SECONDS;
    while samples
        .front()
        .is_some_and(|sample| sample.at_seconds < cutoff)
    {
        samples.pop_front();
    }
}

fn conversation_id(key: &ConversationKey) -> String {
    format!(
        "{}|{}:{}|{}:{}",
        protocol_label(&key.protocol),
        key.local_ip,
        key.local_port,
        key.remote_ip,
        key.remote_port
    )
}

fn protocol_label(protocol: &NetworkProtocol) -> &'static str {
    match protocol {
        NetworkProtocol::Udp => "UDP",
        NetworkProtocol::Tcp => "TCP",
        NetworkProtocol::Other => "OTHER",
    }
}

fn format_seconds(seconds: f64) -> String {
    if seconds.is_finite() {
        format!("{seconds:.3}")
    } else {
        "0.000".to_string()
    }
}

fn slice_range(data: &[u8], range: Range<usize>) -> &[u8] {
    data.get(range).unwrap_or(&[])
}

fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ipv4_udp() {
        let packet = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);
        let parsed = parse_packet(&packet).expect("packet should parse");

        assert_eq!(parsed.protocol, NetworkProtocol::Udp);
        assert_eq!(parsed.source_ip, "192.168.0.10");
        assert_eq!(parsed.source_port, Some(55000));
        assert_eq!(parsed.destination_port, Some(5055));
    }

    #[test]
    fn parses_ipv4_tcp() {
        let packet = ipv4_packet(IP_PROTO_TCP, [10, 0, 0, 2], [1, 1, 1, 1], 44300, 443);
        let parsed = parse_packet(&packet).expect("packet should parse");

        assert_eq!(parsed.protocol, NetworkProtocol::Tcp);
        assert_eq!(parsed.destination_port, Some(443));
    }

    #[test]
    fn parses_ipv6_udp() {
        let packet = ipv6_udp_packet();
        let parsed = parse_packet(&packet).expect("packet should parse");

        assert_eq!(parsed.protocol, NetworkProtocol::Udp);
        assert_eq!(parsed.source_port, Some(40000));
        assert_eq!(parsed.destination_port, Some(5055));
    }

    #[test]
    fn truncated_packet_is_unclassified() {
        assert!(parse_packet(&[0, 1, 2]).is_none());
    }

    #[test]
    fn unknown_protocol_is_other() {
        let packet = ipv4_packet(1, [192, 168, 0, 10], [8, 8, 8, 8], 0, 0);
        let parsed = parse_packet(&packet).expect("packet should parse");

        assert_eq!(parsed.protocol, NetworkProtocol::Other);
    }

    #[test]
    fn groups_same_flow_and_splits_different_flows() {
        let mut aggregator = FlowAggregator::new(vec!["192.168.0.10".to_string()]);
        aggregator.reset_session("s1".to_string(), vec!["192.168.0.10".to_string()]);
        let first = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);
        let second = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);
        let third = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 4, 4], 55000, 5055);

        assert!(aggregator.record_packet(&first, 60, 1.0));
        assert!(aggregator.record_packet(&second, 70, 2.0));
        assert!(aggregator.record_packet(&third, 80, 2.5));

        assert_eq!(aggregator.flow_count(), 2);
        let flows = aggregator.list_flows(10, 3.0);
        assert_eq!(flows.iter().map(|flow| flow.packets).sum::<u64>(), 3);
        assert_eq!(flows.iter().map(|flow| flow.bytes).sum::<u64>(), 210);
    }

    #[test]
    fn groups_inbound_and_outbound_as_one_conversation() {
        let mut aggregator = FlowAggregator::new(vec!["192.168.0.10".to_string()]);
        aggregator.reset_session("s1".to_string(), vec!["192.168.0.10".to_string()]);
        let outbound = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);
        let inbound = ipv4_packet(IP_PROTO_UDP, [8, 8, 8, 8], [192, 168, 0, 10], 5055, 55000);

        assert!(aggregator.record_packet(&outbound, 60, 1.0));
        assert!(aggregator.record_packet(&inbound, 70, 2.0));

        let conversations = aggregator.list_conversations(10, 2.0);
        assert_eq!(aggregator.conversation_count(), 1);
        assert_eq!(conversations[0].packets_sent, 1);
        assert_eq!(conversations[0].packets_received, 1);
        assert_eq!(conversations[0].bytes_sent, 60);
        assert_eq!(conversations[0].bytes_received, 70);
    }

    #[test]
    fn does_not_mix_different_ports_or_protocols() {
        let mut aggregator = FlowAggregator::new(vec!["192.168.0.10".to_string()]);
        aggregator.reset_session("s1".to_string(), vec!["192.168.0.10".to_string()]);
        let udp_a = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);
        let udp_b = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55001, 5055);
        let tcp = ipv4_packet(IP_PROTO_TCP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);

        assert!(aggregator.record_packet(&udp_a, 60, 1.0));
        assert!(aggregator.record_packet(&udp_b, 60, 1.0));
        assert!(aggregator.record_packet(&tcp, 60, 1.0));

        assert_eq!(aggregator.conversation_count(), 3);
    }

    #[test]
    fn calculates_datagram_sizes() {
        let mut aggregator = FlowAggregator::new(vec!["192.168.0.10".to_string()]);
        aggregator.reset_session("s1".to_string(), vec!["192.168.0.10".to_string()]);
        let packet = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);

        assert!(aggregator.record_packet(&packet, 60, 1.0));
        assert!(aggregator.record_packet(&packet, 90, 2.0));
        let conversation = aggregator.list_conversations(1, 2.0)[0].clone();
        let details = aggregator
            .conversation_details("s1", &conversation.id, 2.0)
            .expect("details should exist");

        assert_eq!(details.min_datagram_size, 60);
        assert_eq!(details.max_datagram_size, 90);
        assert_eq!(details.average_datagram_size, 75.0);
    }

    #[test]
    fn circular_buffer_limits_global_and_per_conversation() {
        let mut aggregator = FlowAggregator::with_limits(vec!["192.168.0.10".to_string()], 3, 2);
        aggregator.reset_session("s1".to_string(), vec!["192.168.0.10".to_string()]);
        let packet = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);

        for index in 0..5 {
            assert!(aggregator.record_packet(&packet, 60, f64::from(index)));
        }

        assert_eq!(aggregator.datagram_count(), 2);
        let conversation = aggregator.list_conversations(1, 5.0)[0].clone();
        let datagrams = aggregator
            .recent_datagrams("s1", &conversation.id, 10)
            .expect("recent datagrams should exist");
        assert_eq!(datagrams.len(), 2);
        assert_eq!(datagrams[0].sequence_number, 5);
    }

    #[test]
    fn payload_is_truncated_and_removed_datagram_errors() {
        let mut aggregator = FlowAggregator::with_limits(vec!["192.168.0.10".to_string()], 1, 1);
        aggregator.reset_session("s1".to_string(), vec!["192.168.0.10".to_string()]);
        let packet = ipv4_udp_packet_with_payload(300);

        assert!(aggregator.record_packet(&packet, packet.len() as u64, 1.0));
        let conversation = aggregator.list_conversations(1, 1.0)[0].clone();
        let datagram = aggregator
            .recent_datagrams("s1", &conversation.id, 1)
            .expect("datagrams should exist")[0]
            .clone();
        let payload = aggregator
            .datagram_payload("s1", &conversation.id, &datagram.id)
            .expect("payload should exist");
        assert!(payload.payload_truncated);

        assert!(aggregator.record_packet(&packet, packet.len() as u64, 2.0));
        assert!(aggregator
            .datagram_payload("s1", &conversation.id, &datagram.id)
            .is_err());
    }

    #[test]
    fn active_state_changes_with_time_and_session_clear() {
        let mut aggregator = FlowAggregator::new(vec!["192.168.0.10".to_string()]);
        aggregator.reset_session("s1".to_string(), vec!["192.168.0.10".to_string()]);
        let packet = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);
        assert!(aggregator.record_packet(&packet, 60, 1.0));
        assert!(aggregator.list_conversations(1, 2.0)[0].active);
        assert!(!aggregator.list_conversations(1, 10.0)[0].active);

        aggregator.reset_session("s2".to_string(), vec!["192.168.0.10".to_string()]);
        assert!(aggregator.list_conversations(10, 10.0).is_empty());
        assert!(aggregator.datagram_count() == 0);
    }

    fn ethernet_header(ether_type: u16) -> Vec<u8> {
        let mut packet = vec![0; 12];
        packet.extend_from_slice(&ether_type.to_be_bytes());
        packet
    }

    fn ipv4_packet(
        protocol: u8,
        source: [u8; 4],
        destination: [u8; 4],
        source_port: u16,
        destination_port: u16,
    ) -> Vec<u8> {
        let mut packet = ethernet_header(ETHERTYPE_IPV4);
        packet.extend_from_slice(&[
            0x45,
            0,
            0,
            40,
            0,
            0,
            0,
            0,
            64,
            protocol,
            0,
            0,
            source[0],
            source[1],
            source[2],
            source[3],
            destination[0],
            destination[1],
            destination[2],
            destination[3],
        ]);
        packet.extend_from_slice(&source_port.to_be_bytes());
        packet.extend_from_slice(&destination_port.to_be_bytes());
        if protocol == IP_PROTO_TCP {
            packet.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0x50, 0, 0, 0, 0, 0, 0, 0]);
        } else {
            packet.extend_from_slice(&[0, 8, 0, 0]);
        }
        packet
    }

    fn ipv4_udp_packet_with_payload(payload_size: usize) -> Vec<u8> {
        let mut packet = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);
        packet.extend(vec![0xAB; payload_size]);
        packet
    }

    fn ipv6_udp_packet() -> Vec<u8> {
        let mut packet = ethernet_header(ETHERTYPE_IPV6);
        packet.extend_from_slice(&[0x60, 0, 0, 0, 0, 8, IP_PROTO_UDP, 64]);
        packet.extend_from_slice(&Ipv6Addr::LOCALHOST.octets());
        packet.extend_from_slice(&Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 0x1111).octets());
        packet.extend_from_slice(&40000u16.to_be_bytes());
        packet.extend_from_slice(&5055u16.to_be_bytes());
        packet.extend_from_slice(&[0, 8, 0, 0]);
        packet
    }
}
