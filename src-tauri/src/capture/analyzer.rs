use std::{
    collections::{HashMap, VecDeque},
    net::{Ipv4Addr, Ipv6Addr},
};

use super::models::{FlowDirection, NetworkFlow, NetworkProtocol};

const ETHERTYPE_IPV4: u16 = 0x0800;
const ETHERTYPE_IPV6: u16 = 0x86DD;
const ETHERTYPE_VLAN: u16 = 0x8100;
const ETHERTYPE_QINQ: u16 = 0x88A8;
const IP_PROTO_TCP: u8 = 6;
const IP_PROTO_UDP: u8 = 17;
const ACTIVE_SECONDS: f64 = 3.0;
const RATE_WINDOW_SECONDS: f64 = 3.0;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FlowKey {
    protocol: NetworkProtocol,
    source_ip: String,
    source_port: Option<u16>,
    destination_ip: String,
    destination_port: Option<u16>,
}

#[derive(Clone, Debug)]
pub struct ParsedPacket {
    protocol: NetworkProtocol,
    source_ip: String,
    source_port: Option<u16>,
    destination_ip: String,
    destination_port: Option<u16>,
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

#[derive(Clone, Debug, Default)]
pub struct FlowAggregator {
    flows: HashMap<FlowKey, FlowEntry>,
    local_addresses: Vec<String>,
}

impl FlowAggregator {
    pub fn new(local_addresses: Vec<String>) -> Self {
        Self {
            flows: HashMap::new(),
            local_addresses,
        }
    }

    pub fn reset(&mut self, local_addresses: Vec<String>) {
        self.flows.clear();
        self.local_addresses = local_addresses;
    }

    pub fn record_packet(&mut self, data: &[u8], bytes: u64, now_seconds: f64) -> bool {
        let Some(parsed) = parse_packet(data) else {
            return false;
        };

        let key = FlowKey {
            protocol: parsed.protocol,
            source_ip: parsed.source_ip,
            source_port: parsed.source_port,
            destination_ip: parsed.destination_ip,
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

    #[cfg(test)]
    pub fn flow_count(&self) -> usize {
        self.flows.len()
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

pub fn parse_packet(data: &[u8]) -> Option<ParsedPacket> {
    let (ether_type, offset) = parse_ether_type(data)?;

    match ether_type {
        ETHERTYPE_IPV4 => parse_ipv4(data, offset),
        ETHERTYPE_IPV6 => parse_ipv6(data, offset),
        _ => None,
    }
}

fn parse_ether_type(data: &[u8]) -> Option<(u16, usize)> {
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

    Some((ether_type, offset))
}

fn parse_ipv4(data: &[u8], offset: usize) -> Option<ParsedPacket> {
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

    parse_transport(data, offset + ihl, protocol, source_ip, destination_ip)
}

fn parse_ipv6(data: &[u8], offset: usize) -> Option<ParsedPacket> {
    if data.len() < offset + 40 || data[offset] >> 4 != 6 {
        return None;
    }

    let protocol = data[offset + 6];
    let source_ip =
        Ipv6Addr::from(<[u8; 16]>::try_from(&data[offset + 8..offset + 24]).ok()?).to_string();
    let destination_ip =
        Ipv6Addr::from(<[u8; 16]>::try_from(&data[offset + 24..offset + 40]).ok()?).to_string();

    parse_transport(data, offset + 40, protocol, source_ip, destination_ip)
}

fn parse_transport(
    data: &[u8],
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
            })
        }
        IP_PROTO_TCP => {
            if data.len() < offset + 20 {
                return None;
            }
            Some(ParsedPacket {
                protocol: NetworkProtocol::Tcp,
                source_ip,
                source_port: Some(u16::from_be_bytes([data[offset], data[offset + 1]])),
                destination_ip,
                destination_port: Some(u16::from_be_bytes([data[offset + 2], data[offset + 3]])),
            })
        }
        _ => Some(ParsedPacket {
            protocol: NetworkProtocol::Other,
            source_ip,
            source_port: None,
            destination_ip,
            destination_port: None,
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

fn format_seconds(seconds: f64) -> String {
    if seconds.is_finite() {
        format!("{seconds:.3}")
    } else {
        "0.000".to_string()
    }
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
    fn calculates_rates_and_limits_listing() {
        let mut aggregator = FlowAggregator::new(vec!["192.168.0.10".to_string()]);
        for index in 0..5 {
            let packet = ipv4_packet(
                IP_PROTO_UDP,
                [192, 168, 0, 10],
                [8, 8, 8, index],
                55000,
                5055,
            );
            assert!(aggregator.record_packet(&packet, 90, f64::from(index)));
        }

        let flows = aggregator.list_flows(2, 5.0);
        assert_eq!(flows.len(), 2);
        assert!(flows[0].bytes_per_second.is_finite());
    }

    #[test]
    fn detects_inbound_and_outbound_direction() {
        let mut aggregator = FlowAggregator::new(vec!["192.168.0.10".to_string()]);
        let outbound = ipv4_packet(IP_PROTO_UDP, [192, 168, 0, 10], [8, 8, 8, 8], 55000, 5055);
        let inbound = ipv4_packet(IP_PROTO_UDP, [8, 8, 8, 8], [192, 168, 0, 10], 5055, 55000);

        assert!(aggregator.record_packet(&outbound, 60, 1.0));
        assert!(aggregator.record_packet(&inbound, 60, 1.0));

        let flows = aggregator.list_flows(10, 1.0);
        assert!(flows
            .iter()
            .any(|flow| flow.direction == FlowDirection::Outbound));
        assert!(flows
            .iter()
            .any(|flow| flow.direction == FlowDirection::Inbound));
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
        packet.resize(
            packet.len() + if protocol == IP_PROTO_TCP { 16 } else { 4 },
            0,
        );
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
