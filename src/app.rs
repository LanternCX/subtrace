use anyhow::{Context, Result, bail};
use clap::Parser;
use futures::StreamExt;
use reqwest::Client;
use std::collections::{BTreeSet, HashMap};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;

use crate::ip_meta::lookup_ip_annotation;
use crate::model::{IpAnnotation, NodeProbeResult, NodeReport, ParsedNode};
use crate::ping::run_ping;
use crate::progress::{
    annotation_progress_message, annotation_stage_started_message, fetch_started_message,
    node_finished_message, parse_summary_message, probe_stage_started_message,
    report_write_message,
};
use crate::report::{default_report_path, render_report};
use crate::subscription::{decode_subscription_body, parse_subscription_text};
use crate::traceroute::{collect_unique_ips, run_traceroute};

#[derive(Debug, Parser)]
#[command(name = "subroute")]
#[command(about = "从 Base64 订阅生成 macOS 节点去程路由 Markdown 报告")]
pub struct Cli {
    pub url: String,

    #[arg(short, long)]
    pub output: Option<PathBuf>,

    #[arg(long, default_value_t = 0)]
    pub concurrency: usize,

    #[arg(long, default_value_t = 20)]
    pub max_hops: u8,
}

pub async fn run(cli: Cli) -> Result<PathBuf> {
    eprintln!("{}", fetch_started_message(&cli.url));

    let client = Client::builder()
        .user_agent("subroute/0.1.0")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .context("无法创建 HTTP 客户端")?;

    let response_text = client
        .get(&cli.url)
        .send()
        .await
        .with_context(|| format!("无法拉取订阅: {}", cli.url))?
        .error_for_status()
        .context("订阅请求返回错误状态")?
        .text()
        .await
        .context("无法读取订阅响应")?;

    let decoded = decode_subscription_body(&response_text)?;
    let parsed = parse_subscription_text(&decoded);
    eprintln!(
        "{}",
        parse_summary_message(
            parsed.nodes.len() + parsed.skipped.len(),
            parsed.nodes.len(),
            parsed.skipped.len()
        )
    );
    if parsed.nodes.is_empty() {
        bail!("订阅中没有可测试节点")
    }

    let nodes = parsed.nodes.clone();
    let probe_concurrency = resolve_concurrency(cli.concurrency, nodes.len());
    eprintln!(
        "{}",
        probe_stage_started_message(nodes.len(), probe_concurrency, cli.max_hops)
    );
    let reports = probe_nodes(&nodes, probe_concurrency, cli.max_hops).await;
    let annotations = fetch_annotations(&reports).await;
    let reports = attach_annotations(reports, &annotations);

    let output_path = cli
        .output
        .unwrap_or_else(|| PathBuf::from(default_report_path()));
    let html = render_report(&cli.url, chrono::Local::now(), &reports, &parsed.skipped);
    tokio::fs::write(&output_path, html)
        .await
        .with_context(|| format!("无法写入报告: {}", output_path.display()))?;
    eprintln!("{}", report_write_message(&output_path));

    Ok(output_path)
}

async fn probe_nodes(
    nodes: &[ParsedNode],
    concurrency: usize,
    max_hops: u8,
) -> Vec<NodeProbeResult> {
    let semaphore = Arc::new(Semaphore::new(concurrency.max(1)));
    let completed = Arc::new(AtomicUsize::new(0));
    let total = nodes.len();

    futures::stream::iter(nodes.iter().cloned())
        .map(|node| {
            let semaphore = Arc::clone(&semaphore);
            let completed = Arc::clone(&completed);
            async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .expect("semaphore available");
                let result = probe_single_node(node, max_hops).await;
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                eprintln!(
                    "{}",
                    node_finished_message(
                        done,
                        total,
                        &result.node,
                        result.ping.is_ok(),
                        result.traceroute.is_ok()
                    )
                );
                result
            }
        })
        .buffer_unordered(concurrency.max(1))
        .collect()
        .await
}

async fn probe_single_node(node: ParsedNode, max_hops: u8) -> NodeProbeResult {
    let host = node.host.clone();
    let ping_future = run_ping(&host);
    let traceroute_future = run_traceroute(&host, max_hops);
    let (ping, traceroute) = tokio::join!(ping_future, traceroute_future);

    let ping = ping.map_err(|err| err.to_string());
    let traceroute = traceroute.map_err(|err| err.to_string());

    NodeProbeResult {
        node,
        ping,
        traceroute,
    }
}

async fn fetch_annotations(reports: &[NodeProbeResult]) -> HashMap<IpAddr, IpAnnotation> {
    let ips = reports
        .iter()
        .filter_map(|report| report.traceroute.as_ref().ok())
        .fold(BTreeSet::new(), |mut acc, result| {
            acc.extend(collect_unique_ips(result));
            acc
        });

    let total = ips.len();
    let concurrency = resolve_metadata_concurrency(total);
    eprintln!("{}", annotation_stage_started_message(total, concurrency));

    let semaphore = Arc::new(Semaphore::new(concurrency));
    let completed = Arc::new(AtomicUsize::new(0));
    futures::stream::iter(ips)
        .map(|ip| {
            let semaphore = Arc::clone(&semaphore);
            let completed = Arc::clone(&completed);
            async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .expect("semaphore available");
                let annotation = lookup_ip_annotation(ip).await;
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                eprintln!("{}", annotation_progress_message(done, total));
                (ip, annotation)
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect()
}

fn resolve_metadata_concurrency(total: usize) -> usize {
    total.max(1)
}

fn resolve_concurrency(requested: usize, total: usize) -> usize {
    if requested > 0 {
        return requested;
    }

    total.max(1)
}

#[cfg(test)]
mod tests {
    use super::{resolve_concurrency, resolve_metadata_concurrency};

    #[test]
    fn uses_all_work_as_default_concurrency() {
        assert_eq!(resolve_concurrency(0, 93), 93);
    }

    #[test]
    fn keeps_manual_concurrency_when_provided() {
        assert_eq!(resolve_concurrency(8, 93), 8);
    }

    #[test]
    fn falls_back_to_one_when_no_work_exists() {
        assert_eq!(resolve_concurrency(0, 0), 1);
    }

    #[test]
    fn caps_metadata_concurrency_for_large_annotation_batches() {
        assert_eq!(resolve_metadata_concurrency(199), 199);
    }

    #[test]
    fn uses_all_annotation_work_by_default() {
        assert_eq!(resolve_metadata_concurrency(3), 3);
    }

    #[test]
    fn falls_back_to_one_for_empty_annotation_batches() {
        assert_eq!(resolve_metadata_concurrency(0), 1);
    }
}

fn attach_annotations(
    reports: Vec<NodeProbeResult>,
    annotations: &HashMap<IpAddr, IpAnnotation>,
) -> Vec<NodeReport> {
    reports
        .into_iter()
        .map(|probe| {
            let mut related = HashMap::new();
            if let Ok(result) = &probe.traceroute {
                for ip in collect_unique_ips(result) {
                    if let Some(annotation) = annotations.get(&ip) {
                        related.insert(ip, annotation.clone());
                    }
                }
            }

            NodeReport {
                probe,
                annotations: related,
            }
        })
        .collect()
}
