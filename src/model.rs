use std::collections::HashMap;
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNode {
    pub protocol: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub source_line: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedNode {
    pub source_line: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionParseResult {
    pub nodes: Vec<ParsedNode>,
    pub skipped: Vec<SkippedNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PingResult {
    pub raw_output: String,
    pub times_ms: Vec<f64>,
    pub transmitted: Option<u32>,
    pub received: Option<u32>,
    pub loss_percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceProbe {
    pub ip: Option<IpAddr>,
    pub latency_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceHop {
    pub ttl: u8,
    pub raw_line: String,
    pub probes: Vec<TraceProbe>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TracerouteResult {
    pub raw_output: String,
    pub hops: Vec<TraceHop>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpAnnotation {
    pub ip: IpAddr,
    pub asn: Option<String>,
    pub org: Option<String>,
    pub location: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeProbeResult {
    pub node: ParsedNode,
    pub ping: Result<PingResult, String>,
    pub traceroute: Result<TracerouteResult, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeReport {
    pub probe: NodeProbeResult,
    pub annotations: HashMap<IpAddr, IpAnnotation>,
}
