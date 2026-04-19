use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::model::{ParsedNode, SkippedNode, SubscriptionParseResult};

pub fn decode_subscription_body(body: &str) -> Result<String> {
    let cleaned: String = body.chars().filter(|ch| !ch.is_whitespace()).collect();

    for engine in [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD] {
        if let Ok(decoded) = engine.decode(cleaned.as_bytes()) {
            let text = String::from_utf8(decoded).context("订阅内容不是有效的 UTF-8 文本")?;
            return Ok(text);
        }
    }

    bail!("订阅内容不是可识别的 Base64 文本")
}

pub fn parse_subscription_text(text: &str) -> SubscriptionParseResult {
    let mut nodes = Vec::new();
    let mut skipped = Vec::new();

    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        match parse_node_line(line) {
            Ok(node) => nodes.push(node),
            Err(reason) => skipped.push(SkippedNode {
                source_line: line.to_string(),
                reason,
            }),
        }
    }

    SubscriptionParseResult { nodes, skipped }
}

pub fn parse_node_line(line: &str) -> std::result::Result<ParsedNode, String> {
    let scheme = line
        .split_once("://")
        .map(|(scheme, _)| scheme.to_lowercase())
        .ok_or_else(|| "缺少协议头".to_string())?;

    match scheme.as_str() {
        "vmess" => parse_vmess(line),
        "ss" => parse_ss(line),
        _ => parse_standard_url(line),
    }
}

fn parse_standard_url(line: &str) -> std::result::Result<ParsedNode, String> {
    let url = Url::parse(line).map_err(|err| format!("URL 解析失败: {err}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "缺少目标地址".to_string())?
        .to_string();
    let port = url.port().ok_or_else(|| "缺少端口".to_string())?;
    let protocol = url.scheme().to_string();
    let name = url
        .fragment()
        .map(decode_fragment)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("{host}:{port}"));

    Ok(ParsedNode {
        protocol,
        name,
        host,
        port,
        source_line: line.to_string(),
    })
}

fn parse_vmess(line: &str) -> std::result::Result<ParsedNode, String> {
    let payload = line
        .strip_prefix("vmess://")
        .ok_or_else(|| "vmess 节点缺少前缀".to_string())?;
    let decoded =
        decode_compact_base64(payload).ok_or_else(|| "vmess 节点 Base64 解码失败".to_string())?;
    let config: VmessConfig =
        serde_json::from_slice(&decoded).map_err(|err| format!("vmess JSON 解析失败: {err}"))?;
    let host = config
        .add
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "vmess 节点缺少目标地址".to_string())?;
    let port = parse_port_value(config.port).ok_or_else(|| "vmess 节点端口无效".to_string())?;
    let name = config
        .ps
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{host}:{port}"));

    Ok(ParsedNode {
        protocol: "vmess".to_string(),
        name,
        host,
        port,
        source_line: line.to_string(),
    })
}

fn parse_ss(line: &str) -> std::result::Result<ParsedNode, String> {
    if let Ok(node) = parse_standard_url(line) {
        return Ok(node);
    }

    let raw = line
        .strip_prefix("ss://")
        .ok_or_else(|| "ss 节点缺少前缀".to_string())?;

    let (payload, fragment) = match raw.split_once('#') {
        Some((payload, fragment)) => (payload, Some(fragment)),
        None => (raw, None),
    };

    let payload = payload
        .split_once('?')
        .map(|(payload, _)| payload)
        .unwrap_or(payload)
        .trim_end_matches('/');

    let decoded =
        decode_compact_base64(payload).ok_or_else(|| "ss 节点 Base64 解码失败".to_string())?;
    let decoded_text = String::from_utf8(decoded).map_err(|_| "ss 节点不是有效文本".to_string())?;
    let rebuilt = if fragment.is_some() {
        format!("ss://{decoded_text}#{}", fragment.unwrap_or_default())
    } else {
        format!("ss://{decoded_text}")
    };

    parse_standard_url(&rebuilt)
}

fn decode_compact_base64(value: &str) -> Option<Vec<u8>> {
    let cleaned: String = value.chars().filter(|ch| !ch.is_whitespace()).collect();
    for engine in [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(cleaned.as_bytes()) {
            return Some(bytes);
        }
    }

    None
}

fn decode_fragment(fragment: &str) -> String {
    percent_decode_str(fragment)
        .decode_utf8_lossy()
        .into_owned()
}

fn parse_port_value(value: Value) -> Option<u16> {
    match value {
        Value::Number(number) => number.as_u64().and_then(|value| u16::try_from(value).ok()),
        Value::String(text) => text.parse::<u16>().ok(),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct VmessConfig {
    add: Option<String>,
    port: Value,
    ps: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{decode_subscription_body, parse_node_line, parse_subscription_text};
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    #[test]
    fn decodes_base64_subscription_text() {
        let encoded = STANDARD.encode("vless://uuid@example.com:443#Tokyo");

        let decoded = decode_subscription_body(&encoded).expect("should decode subscription");

        assert_eq!(decoded, "vless://uuid@example.com:443#Tokyo");
    }

    #[test]
    fn parses_vmess_node_from_embedded_json() {
        let vmess_json = r#"{"v":"2","ps":"HK-A","add":"1.2.3.4","port":"443"}"#;
        let encoded = STANDARD.encode(vmess_json);
        let line = format!("vmess://{encoded}");

        let node = parse_node_line(&line).expect("should parse vmess");

        assert_eq!(node.protocol, "vmess");
        assert_eq!(node.name, "HK-A");
        assert_eq!(node.host, "1.2.3.4");
        assert_eq!(node.port, 443);
    }

    #[test]
    fn parses_standard_proxy_url_node() {
        let node = parse_node_line("vless://uuid@example.com:8443?security=tls#Tokyo%20Edge")
            .expect("should parse vless");

        assert_eq!(node.protocol, "vless");
        assert_eq!(node.name, "Tokyo Edge");
        assert_eq!(node.host, "example.com");
        assert_eq!(node.port, 8443);
    }

    #[test]
    fn parses_shadowsocks_base64_node() {
        let encoded_payload = STANDARD.encode("aes-256-gcm:password@example.com:8388");
        let line = format!("ss://{encoded_payload}#HK-SS");

        let node = parse_node_line(&line).expect("should parse ss");

        assert_eq!(node.protocol, "ss");
        assert_eq!(node.name, "HK-SS");
        assert_eq!(node.host, "example.com");
        assert_eq!(node.port, 8388);
    }

    #[test]
    fn parses_shadowsocks_node_with_plugin_query() {
        let encoded_payload = STANDARD.encode("aes-256-gcm:password@example.com:8388");
        let line = format!(
            "ss://{encoded_payload}/?plugin=obfs-local%3Bobfs%3Dhttp%3Bobfs-host%3Dcdn.example.com#HK-Plugin"
        );

        let node = parse_node_line(&line).expect("should parse ss with plugin query");

        assert_eq!(node.protocol, "ss");
        assert_eq!(node.name, "HK-Plugin");
        assert_eq!(node.host, "example.com");
        assert_eq!(node.port, 8388);
    }

    #[test]
    fn separates_unparseable_lines() {
        let result = parse_subscription_text(
            "vless://uuid@example.com:443#Good\nnot-a-node\nvmess://bad-base64",
        );

        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.skipped.len(), 2);
    }
}
