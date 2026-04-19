use std::path::Path;

use crate::model::ParsedNode;

pub fn fetch_started_message(_url: &str) -> String {
    format!("[1/5] 正在拉取订阅: {_url}")
}

pub fn parse_summary_message(total: usize, runnable: usize, skipped: usize) -> String {
    format!("[2/5] 订阅解析完成: 共 {total} 条，可测 {runnable} 条，跳过 {skipped} 条")
}

pub fn probe_stage_started_message(total: usize, concurrency: usize, max_hops: u8) -> String {
    format!("[3/5] 开始测试节点: {total} 个，并发 {concurrency}，最大跳数 {max_hops}")
}

pub fn node_finished_message(
    done: usize,
    total: usize,
    node: &ParsedNode,
    ping_ok: bool,
    traceroute_ok: bool,
) -> String {
    let ping_status = if ping_ok { "成功" } else { "失败" };
    let traceroute_status = if traceroute_ok { "成功" } else { "失败" };

    format!(
        "[节点 {done}/{total}] {} ({}:{}) ping={} traceroute={}",
        node.name, node.host, node.port, ping_status, traceroute_status
    )
}

pub fn annotation_stage_started_message(total_ips: usize, concurrency: usize) -> String {
    format!("[4/5] 开始补全每跳 AS 信息: {total_ips} 个 IP，并发 {concurrency}")
}

pub fn annotation_progress_message(done: usize, total: usize) -> String {
    format!("[4/5] 正在补全每跳 AS 信息: {done}/{total}")
}

pub fn report_write_message(path: &Path) -> String {
    format!("[5/5] 报告已写入: {}", path.display())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::model::ParsedNode;

    use super::{
        annotation_progress_message, fetch_started_message, node_finished_message,
        parse_summary_message, probe_stage_started_message, report_write_message,
    };

    #[test]
    fn renders_fetch_and_parse_messages() {
        assert_eq!(
            fetch_started_message("https://example.com/sub"),
            "[1/5] 正在拉取订阅: https://example.com/sub"
        );
        assert_eq!(
            parse_summary_message(12, 10, 2),
            "[2/5] 订阅解析完成: 共 12 条，可测 10 条，跳过 2 条"
        );
    }

    #[test]
    fn renders_probe_stage_and_node_messages() {
        let node = ParsedNode {
            protocol: "vless".to_string(),
            name: "Tokyo Edge".to_string(),
            host: "example.com".to_string(),
            port: 443,
            source_line: "vless://...".to_string(),
        };

        assert_eq!(
            probe_stage_started_message(10, 4, 20),
            "[3/5] 开始测试节点: 10 个，并发 4，最大跳数 20"
        );
        assert_eq!(
            node_finished_message(3, 10, &node, true, false),
            "[节点 3/10] Tokyo Edge (example.com:443) ping=成功 traceroute=失败"
        );
    }

    #[test]
    fn renders_annotation_and_output_messages() {
        assert_eq!(
            annotation_progress_message(7, 28),
            "[4/5] 正在补全每跳 AS 信息: 7/28"
        );
        assert_eq!(
            report_write_message(Path::new("report.md")),
            "[5/5] 报告已写入: report.md"
        );
    }
}
