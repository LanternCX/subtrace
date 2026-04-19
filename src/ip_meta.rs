use anyhow::{Context, Result};
use std::net::IpAddr;
use tokio::process::Command;

use crate::model::IpAnnotation;

pub async fn lookup_ip_annotation(ip: IpAddr) -> IpAnnotation {
    if let Some(note) = special_ip_note(ip) {
        return IpAnnotation {
            ip,
            asn: None,
            org: None,
            location: None,
            note: Some(note.to_string()),
        };
    }

    match fetch_cymru_annotation(ip).await {
        Ok((asn, org, region)) => IpAnnotation {
            ip,
            asn,
            org,
            location: region,
            note: None,
        },
        Err(err) => IpAnnotation {
            ip,
            asn: None,
            org: None,
            location: None,
            note: Some(format!("AS 查询失败: {err}")),
        },
    }
}

async fn fetch_cymru_annotation(
    ip: IpAddr,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let origin = fetch_cymru_origin(ip).await?;
    let org = match origin.asn.as_deref() {
        Some(asn) => fetch_cymru_org(asn).await.ok(),
        None => None,
    };

    Ok((origin.asn, org, origin.region))
}

async fn fetch_cymru_origin(ip: IpAddr) -> Result<CymruOriginInfo> {
    let query = cymru_origin_query(ip).context("暂不支持 IPv6 AS 查询")?;
    let output = Command::new("dig")
        .args(["+short", "TXT", &query])
        .output()
        .await
        .context("无法执行 dig 查询 AS")?;

    if !output.status.success() {
        anyhow::bail!("dig 查询 AS 失败")
    }

    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(match parse_cymru_origin_response(&text) {
        Some((asn, region)) => CymruOriginInfo {
            asn: Some(asn),
            region: Some(region),
        },
        None => CymruOriginInfo {
            asn: None,
            region: None,
        },
    })
}

async fn fetch_cymru_org(asn: &str) -> Result<String> {
    let query = format!("AS{asn}.asn.cymru.com");
    let output = Command::new("dig")
        .args(["+short", "TXT", &query])
        .output()
        .await
        .context("无法执行 dig 查询 AS 名称")?;

    if !output.status.success() {
        anyhow::bail!("dig 查询 AS 名称失败")
    }

    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    parse_cymru_as_name_response(&text).context("AS 名称结果为空")
}

fn special_ip_note(ip: IpAddr) -> Option<&'static str> {
    match ip {
        IpAddr::V4(ipv4) => {
            if ipv4.is_private()
                || ipv4.is_loopback()
                || ipv4.is_link_local()
                || ipv4.is_multicast()
                || ipv4.is_unspecified()
                || ipv4.octets()[0] >= 224
            {
                return Some("本地或私网地址");
            }

            if ipv4.octets()[0] == 198 && (ipv4.octets()[1] == 18 || ipv4.octets()[1] == 19) {
                return Some("保留测试地址");
            }

            None
        }
        IpAddr::V6(ipv6) => {
            if ipv6.is_loopback()
                || ipv6.is_multicast()
                || ipv6.is_unspecified()
                || ipv6.is_unique_local()
                || ipv6.is_unicast_link_local()
            {
                return Some("本地或私网地址");
            }

            None
        }
    }
}

fn cymru_origin_query(ip: IpAddr) -> Option<String> {
    match ip {
        IpAddr::V4(ipv4) => Some(
            ipv4.octets()
                .iter()
                .rev()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(".")
                + ".origin.asn.cymru.com",
        ),
        IpAddr::V6(_) => None,
    }
}

fn parse_cymru_origin_response(output: &str) -> Option<(String, String)> {
    let line = output.lines().find(|line| !line.trim().is_empty())?;
    let cleaned = line.trim().trim_matches('"');
    let mut parts = cleaned.split('|').map(str::trim);
    let asn = parts.next()?.split_whitespace().next()?.to_string();
    let _prefix = parts.next()?;
    let country = parts.next()?;
    if asn.is_empty() || country.is_empty() {
        return None;
    }

    Some((asn, normalize_country_code(country)))
}

fn parse_cymru_as_name_response(output: &str) -> Option<String> {
    let line = output.lines().find(|line| !line.trim().is_empty())?;
    let cleaned = line.trim().trim_matches('"');
    cleaned
        .split('|')
        .nth(4)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_country_code(code: &str) -> String {
    match code.trim().to_ascii_uppercase().as_str() {
        "US" => "美国".to_string(),
        "CN" => "中国".to_string(),
        "JP" => "日本".to_string(),
        "HK" => "香港".to_string(),
        "SG" => "新加坡".to_string(),
        "TW" => "台湾".to_string(),
        "AU" => "澳大利亚".to_string(),
        "KR" => "韩国".to_string(),
        "AE" => "阿联酋".to_string(),
        "CH" => "瑞士".to_string(),
        "IN" => "印度".to_string(),
        "SE" => "瑞典".to_string(),
        other => other.to_string(),
    }
}

#[derive(Debug)]
struct CymruOriginInfo {
    asn: Option<String>,
    region: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{parse_cymru_as_name_response, parse_cymru_origin_response};

    #[test]
    fn parses_cymru_origin_answer_into_asn_and_region() {
        let parsed =
            parse_cymru_origin_response("\"46997 | 134.195.100.0/23 | US | arin | 2020-06-25\"")
                .expect("should parse cymru origin response");

        assert_eq!(parsed.0, "46997");
        assert_eq!(parsed.1, "美国");
    }

    #[test]
    fn parses_cymru_as_name_answer_into_org() {
        let parsed = parse_cymru_as_name_response(
            "\"46997 | US | arin | 2020-05-04 | NATOLAB - Black Mesa Corporation, US\"",
        )
        .expect("should parse cymru as-name response");

        assert_eq!(parsed, "NATOLAB - Black Mesa Corporation, US");
    }

    #[test]
    fn keeps_first_asn_when_origin_response_contains_multiple_asns() {
        let parsed = parse_cymru_origin_response(
            "\"3491 31713 | 103.197.71.0/24 | HK | apnic | 2022-06-08\"",
        )
        .expect("should parse first ASN from origin response");

        assert_eq!(parsed.0, "3491");
        assert_eq!(parsed.1, "香港");
    }
}
