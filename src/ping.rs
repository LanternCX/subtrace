use anyhow::{Context, Result};
use regex::Regex;
use tokio::process::Command;

use crate::model::PingResult;

pub async fn run_ping(host: &str) -> Result<PingResult> {
    let output = Command::new("ping")
        .args(["-c", "3", host])
        .output()
        .await
        .with_context(|| format!("无法执行 ping: {host}"))?;

    let raw_output = merge_output(&output.stdout, &output.stderr);
    finalize_ping_result(output.status.success(), &raw_output)
        .map_err(|message| anyhow::anyhow!(message))
}

pub fn parse_ping_output(output: &str) -> PingResult {
    let time_regex = Regex::new(r"time=([0-9]+(?:\.[0-9]+)?)\s*ms").expect("valid ping regex");
    let stats_regex = Regex::new(
        r"(?m)(\d+) packets transmitted, (\d+) packets received, ([0-9]+(?:\.[0-9]+)?)% packet loss",
    )
    .expect("valid ping stats regex");

    let times_ms = time_regex
        .captures_iter(output)
        .filter_map(|capture| {
            capture
                .get(1)
                .and_then(|value| value.as_str().parse::<f64>().ok())
        })
        .collect::<Vec<_>>();

    let (transmitted, received, loss_percent) = stats_regex
        .captures(output)
        .map(|capture| {
            let transmitted = capture
                .get(1)
                .and_then(|value| value.as_str().parse::<u32>().ok());
            let received = capture
                .get(2)
                .and_then(|value| value.as_str().parse::<u32>().ok());
            let loss_percent = capture
                .get(3)
                .and_then(|value| value.as_str().parse::<f64>().ok());
            (transmitted, received, loss_percent)
        })
        .unwrap_or((None, None, None));

    PingResult {
        raw_output: output.trim().to_string(),
        times_ms,
        transmitted,
        received,
        loss_percent,
    }
}

fn finalize_ping_result(
    command_success: bool,
    output: &str,
) -> std::result::Result<PingResult, String> {
    let parsed = parse_ping_output(output);
    if command_success || parsed.transmitted.is_some() {
        return Ok(parsed);
    }

    let message = output.trim();
    if message.is_empty() {
        Err("ping 执行失败，且没有返回任何输出".to_string())
    } else {
        Err(format!("ping 执行失败: {message}"))
    }
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
    use super::{finalize_ping_result, parse_ping_output};

    #[test]
    fn parses_ping_times_and_statistics() {
        let sample = r#"PING 1.1.1.1 (1.1.1.1): 56 data bytes
64 bytes from 1.1.1.1: icmp_seq=0 ttl=57 time=10.054 ms
64 bytes from 1.1.1.1: icmp_seq=1 ttl=57 time=9.847 ms
64 bytes from 1.1.1.1: icmp_seq=2 ttl=57 time=9.901 ms

--- 1.1.1.1 ping statistics ---
3 packets transmitted, 3 packets received, 0.0% packet loss
round-trip min/avg/max/stddev = 9.847/9.934/10.054/0.086 ms"#;

        let parsed = parse_ping_output(sample);

        assert_eq!(parsed.times_ms.len(), 3);
        assert_eq!(parsed.transmitted, Some(3));
        assert_eq!(parsed.received, Some(3));
        assert_eq!(parsed.loss_percent, Some(0.0));
    }

    #[test]
    fn keeps_zero_response_ping_results() {
        let sample = r#"PING blocked.example (203.0.113.5): 56 data bytes

--- blocked.example ping statistics ---
3 packets transmitted, 0 packets received, 100.0% packet loss"#;

        let parsed =
            finalize_ping_result(false, sample).expect("packet loss should still be reported");

        assert!(parsed.times_ms.is_empty());
        assert_eq!(parsed.transmitted, Some(3));
        assert_eq!(parsed.received, Some(0));
        assert_eq!(parsed.loss_percent, Some(100.0));
    }

    #[test]
    fn reports_command_failure_when_statistics_are_missing() {
        let error = finalize_ping_result(
            false,
            "ping: cannot resolve not-found.invalid: Unknown host",
        )
        .expect_err("missing statistics should become an explicit error");

        assert!(error.contains("ping 执行失败"));
        assert!(error.contains("Unknown host"));
    }
}
