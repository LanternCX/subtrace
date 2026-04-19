use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::net::IpAddr;
use tokio::process::Command;

use crate::model::{TraceHop, TraceProbe, TracerouteResult};

pub async fn run_traceroute(host: &str, max_hops: u8) -> Result<TracerouteResult> {
    let output = Command::new("traceroute")
        .args(build_traceroute_args(host, max_hops))
        .output()
        .await
        .with_context(|| format!("无法执行 traceroute: {host}"))?;

    let raw_output = merge_output(&output.stdout, &output.stderr);
    finalize_traceroute_result(output.status.success(), &raw_output)
        .map_err(|message| anyhow::anyhow!(message))
}

pub fn parse_traceroute_output(output: &str) -> TracerouteResult {
    let hops = output
        .lines()
        .filter_map(parse_hop_line)
        .collect::<Vec<_>>();

    TracerouteResult {
        raw_output: output.trim().to_string(),
        hops,
    }
}

pub fn collect_unique_ips(result: &TracerouteResult) -> BTreeSet<IpAddr> {
    result
        .hops
        .iter()
        .flat_map(|hop| hop.probes.iter().filter_map(|probe| probe.ip))
        .collect()
}

fn build_traceroute_args(host: &str, max_hops: u8) -> Vec<String> {
    vec![
        "-I".to_string(),
        "-n".to_string(),
        "-q".to_string(),
        "3".to_string(),
        "-w".to_string(),
        "1".to_string(),
        "-m".to_string(),
        max_hops.to_string(),
        host.to_string(),
    ]
}

fn finalize_traceroute_result(
    command_success: bool,
    output: &str,
) -> std::result::Result<TracerouteResult, String> {
    let parsed = parse_traceroute_output(output);
    if command_success || !parsed.hops.is_empty() {
        return Ok(parsed);
    }

    let message = output.trim();
    if message.is_empty() {
        Err("traceroute 执行失败，且没有返回任何输出".to_string())
    } else {
        Err(format!("traceroute 执行失败: {message}"))
    }
}

fn parse_hop_line(line: &str) -> Option<TraceHop> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let ttl = parts.next()?.parse::<u8>().ok()?;
    let tokens = parts.collect::<Vec<_>>();

    let mut probes = Vec::new();
    let mut current_ip = None;
    let mut index = 0usize;

    while index < tokens.len() {
        let token = tokens[index];
        if token == "*" {
            probes.push(TraceProbe {
                ip: None,
                latency_ms: None,
            });
            index += 1;
            continue;
        }

        if let Ok(ip) = token.parse::<IpAddr>() {
            current_ip = Some(ip);
            index += 1;
            continue;
        }

        if let Ok(latency) = token.parse::<f64>() {
            if tokens.get(index + 1).copied() == Some("ms") {
                probes.push(TraceProbe {
                    ip: current_ip,
                    latency_ms: Some(latency),
                });
                index += 2;
                continue;
            }
        }

        index += 1;
    }

    Some(TraceHop {
        ttl,
        raw_line: trimmed.to_string(),
        probes,
    })
}

fn merge_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut chunks = Vec::new();

    if !stdout.is_empty() {
        chunks.push(String::from_utf8_lossy(stdout).into_owned());
    }

    if !stderr.is_empty() {
        chunks.push(String::from_utf8_lossy(stderr).into_owned());
    }

    chunks.join("\n")
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::{
        build_traceroute_args, collect_unique_ips, finalize_traceroute_result,
        parse_traceroute_output,
    };

    #[test]
    fn builds_icmp_traceroute_arguments() {
        assert_eq!(
            build_traceroute_args("1.1.1.1", 5),
            vec!["-I", "-n", "-q", "3", "-w", "1", "-m", "5", "1.1.1.1"]
        );
    }

    #[test]
    fn parses_timeout_and_latency_lines() {
        let sample = r#"traceroute to 1.1.1.1 (1.1.1.1), 20 hops max, 52 byte packets
 1  192.168.1.1  1.106 ms  0.847 ms  0.884 ms
 2  * * *
 3  10.0.0.1  12.100 ms  11.998 ms  12.301 ms"#;

        let parsed = parse_traceroute_output(sample);

        assert_eq!(parsed.hops.len(), 3);
        assert_eq!(parsed.hops[1].ttl, 2);
        assert_eq!(parsed.hops[1].probes.len(), 3);
        assert!(parsed.hops[1].probes.iter().all(|probe| probe.ip.is_none()));
        assert_eq!(parsed.hops[2].probes[0].latency_ms, Some(12.1));
    }

    #[test]
    fn keeps_multiple_ips_inside_single_hop() {
        let sample = r#"traceroute to 8.8.8.8 (8.8.8.8), 20 hops max, 52 byte packets
 1  10.0.0.1  1.000 ms  10.0.0.2  2.000 ms  10.0.0.1  1.200 ms"#;

        let parsed = parse_traceroute_output(sample);
        let ips = collect_unique_ips(&parsed);

        assert_eq!(parsed.hops[0].probes.len(), 3);
        assert_eq!(ips.len(), 2);
        assert!(ips.contains(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(ips.contains(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2))));
    }

    #[test]
    fn does_not_drop_first_hop_when_header_is_last() {
        let sample = r#" 1  * * *
 2  10.0.0.1  12.100 ms  11.998 ms  12.301 ms
traceroute to 1.1.1.1 (1.1.1.1), 20 hops max, 52 byte packets"#;

        let parsed = parse_traceroute_output(sample);

        assert_eq!(parsed.hops.len(), 2);
        assert_eq!(parsed.hops[0].ttl, 1);
        assert_eq!(parsed.hops[1].ttl, 2);
    }

    #[test]
    fn reports_command_failure_when_no_hop_is_available() {
        let error = finalize_traceroute_result(false, "traceroute: unknown host not-found.invalid")
            .expect_err("missing hops should become an explicit error");

        assert!(error.contains("traceroute 执行失败"));
        assert!(error.contains("unknown host"));
    }
}
