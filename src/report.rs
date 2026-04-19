use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::IpAddr;

use chrono::{DateTime, Local};

use crate::model::{IpAnnotation, NodeReport, PingResult, SkippedNode};
use crate::topology_svg::{TopologyNodeSummary, build_topology_view, leaf_node_id};

pub fn render_report(
    source_url: &str,
    generated_at: DateTime<Local>,
    reports: &[NodeReport],
    skipped: &[SkippedNode],
) -> String {
    let topology = build_topology_view(reports);
    let summary_html = render_summary_cards(source_url, generated_at, reports, skipped);
    let region_filter_html = render_region_filter(&topology.nodes);
    let region_group_html = render_region_groups(&topology.nodes);
    let details = build_detail_map(source_url, generated_at, reports, skipped, &topology.nodes);
    let details_json = serde_json::to_string(&details).expect("detail map should serialize");

    format!(
        r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Subroute 路由关系图</title>
  <style>
    :root {{
      color-scheme: light;
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
      background: #f8fafc;
      color: #0f172a;
    }}
    body {{
      margin: 0;
      background: #f8fafc;
      color: #0f172a;
    }}
    main {{
      max-width: 1480px;
      margin: 0 auto;
      padding: 24px;
      display: grid;
      gap: 20px;
    }}
    .panel {{
      background: #ffffff;
      border: 1px solid #e2e8f0;
      border-radius: 18px;
      box-shadow: 0 8px 20px rgba(15, 23, 42, 0.04);
      padding: 18px 20px;
    }}
    .summary-grid {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 12px;
      margin-bottom: 16px;
    }}
    .summary-wide {{
      background: linear-gradient(180deg, #f8fafc 0%, #ffffff 100%);
      border: 1px solid #e2e8f0;
      border-radius: 14px;
      padding: 14px 16px;
      margin-bottom: 12px;
    }}
    .summary-wide .label {{
      display: block;
      font-size: 12px;
      color: #64748b;
      margin-bottom: 6px;
    }}
    .summary-wide .value {{
      font-size: 14px;
      font-weight: 500;
      line-height: 1.6;
      word-break: break-all;
    }}
    .summary-card {{
      background: linear-gradient(180deg, #f8fafc 0%, #ffffff 100%);
      border: 1px solid #e2e8f0;
      border-radius: 14px;
      padding: 14px 16px;
    }}
    .summary-card .label {{
      display: block;
      font-size: 12px;
      color: #64748b;
      margin-bottom: 6px;
    }}
    .summary-card .value {{
      font-size: 18px;
      font-weight: 600;
    }}
    .controls {{
      display: grid;
      gap: 16px;
    }}
    .toolbar-row {{
      display: flex;
      flex-wrap: wrap;
      gap: 12px;
      align-items: center;
    }}
    .toolbar-row input[type='search'] {{
      min-width: 240px;
      border: 1px solid #cbd5e1;
      border-radius: 12px;
      padding: 10px 12px;
      font-size: 14px;
      background: #ffffff;
    }}
    .toolbar-row button {{
      border: 1px solid #cbd5e1;
      background: #ffffff;
      border-radius: 12px;
      padding: 10px 14px;
      font-size: 14px;
      cursor: pointer;
    }}
    .region-filter {{
      display: flex;
      flex-wrap: wrap;
      gap: 10px;
      align-items: center;
    }}
    .region-filter-title {{
      font-weight: 600;
      margin-right: 4px;
    }}
    .region-filter label {{
      display: inline-flex;
      gap: 6px;
      align-items: center;
      background: #f8fafc;
      border: 1px solid #e2e8f0;
      border-radius: 999px;
      padding: 8px 12px;
      font-size: 13px;
    }}
    .region-groups {{
      display: grid;
      gap: 10px;
    }}
    .region-groups details {{
      border: 1px solid #e2e8f0;
      border-radius: 14px;
      padding: 10px 12px;
      background: #f8fafc;
    }}
    .region-groups summary {{
      cursor: pointer;
      font-weight: 600;
    }}
    .object-columns {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 12px;
      margin-top: 10px;
    }}
    .object-column h4 {{
      margin: 0 0 8px;
      font-size: 13px;
      color: #475569;
    }}
    .object-list {{
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }}
    .object-link {{
      border: 1px solid #cbd5e1;
      background: #ffffff;
      color: #0f172a;
      border-radius: 999px;
      padding: 7px 10px;
      font-size: 12px;
      cursor: pointer;
    }}
    .object-link.is-selected {{
      border-color: #f97316;
      color: #c2410c;
      background: #fff7ed;
    }}
    .graph-panel {{
      display: grid;
      gap: 14px;
    }}
    .graph-header {{
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 12px;
      flex-wrap: wrap;
    }}
    .graph-header h2 {{
      margin: 0;
      font-size: 18px;
    }}
    .graph-viewport {{
      overflow: auto;
      border: 1px solid #e2e8f0;
      border-radius: 16px;
      background: #ffffff;
      padding: 12px;
      min-height: 520px;
    }}
    #graph-root {{
      transform-origin: top center;
      transition: transform 0.15s ease;
    }}
    .graph-node.is-dimmed {{
      opacity: 0.15;
    }}
    .graph-edge.is-dimmed {{
      opacity: 0.08;
    }}
    .detail-shell h2 {{
      margin: 0 0 12px;
      font-size: 18px;
    }}
    #detail-panel {{
      display: grid;
      gap: 14px;
    }}
    .detail-card {{
      border: 1px solid #e2e8f0;
      border-radius: 14px;
      background: #ffffff;
      padding: 16px;
    }}
    .detail-card h3 {{
      margin: 0 0 10px;
      font-size: 16px;
    }}
    .detail-meta {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 10px;
      margin-bottom: 12px;
    }}
    .detail-meta div {{
      background: #f8fafc;
      border-radius: 12px;
      padding: 10px 12px;
    }}
    .detail-meta .label {{
      display: block;
      font-size: 12px;
      color: #64748b;
      margin-bottom: 4px;
    }}
    table {{
      width: 100%;
      border-collapse: collapse;
      font-size: 13px;
    }}
    th, td {{
      border-bottom: 1px solid #e2e8f0;
      padding: 9px 10px;
      text-align: left;
      vertical-align: top;
    }}
    th {{
      color: #475569;
      font-weight: 600;
      background: #f8fafc;
    }}
    .detail-list {{
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin: 0;
      padding: 0;
      list-style: none;
    }}
    .detail-list li {{
      background: #f8fafc;
      border: 1px solid #e2e8f0;
      border-radius: 999px;
      padding: 7px 10px;
      font-size: 12px;
    }}
  </style>
</head>
<body>
  <main>
    <section class="panel controls">
      {summary_html}
      <div class="toolbar-row">
        <input id="search-input" type="search" placeholder="搜索 AS 或节点名称" />
        <button type="button" id="zoom-in">放大</button>
        <button type="button" id="zoom-out">缩小</button>
        <button type="button" id="zoom-reset">重置缩放</button>
        <button type="button" id="filter-reset">重置筛选</button>
      </div>
      {region_filter_html}
    </section>

    <section class="panel graph-panel">
      <div class="graph-header">
        <h2>路由关系图</h2>
      </div>
      <div class="graph-viewport" id="graph-viewport">
        <div id="graph-root">{}</div>
      </div>
    </section>

    <section class="panel detail-shell">
      <h2>详细信息</h2>
      <div id="detail-panel"></div>
    </section>

    <section class="panel">
      <h2>地区分组</h2>
      {region_group_html}
    </section>
  </main>

  <script id="detail-data" type="application/json">{details_json}</script>
  <script>
    const details = JSON.parse(document.getElementById('detail-data').textContent)
    const graphNodes = Array.from(document.querySelectorAll('.graph-node'))
    const graphEdges = Array.from(document.querySelectorAll('.graph-edge'))
    const regionToggles = Array.from(document.querySelectorAll('.region-filter .region-toggle'))
    const detailPanel = document.getElementById('detail-panel')
    const searchInput = document.getElementById('search-input')
    const graphRoot = document.getElementById('graph-root')

    let scale = 1
    let selectedNodeId = 'root:local'

    function activeRegions() {{
      return new Set(regionToggles.filter((toggle) => toggle.checked).map((toggle) => toggle.value))
    }}

    function matchesFilter(region, label, kind) {{
      if (kind === 'root') return true
      const query = searchInput.value.trim().toLowerCase()
      const regions = activeRegions()
      const regionOk = regions.has(region)
      const searchOk = !query || label.toLowerCase().includes(query)
      return regionOk && searchOk
    }}

    function applyFilters() {{
      const visible = new Set()
      graphNodes.forEach((node) => {{
        const match = matchesFilter(node.dataset.region || '未知', node.dataset.label || '', node.dataset.kind || '')
        node.classList.toggle('is-dimmed', !match)
        if (match || node.dataset.kind === 'root') visible.add(node.dataset.nodeId)
      }})
      graphEdges.forEach((edge) => {{
        const show = visible.has(edge.dataset.source) && visible.has(edge.dataset.target)
        edge.classList.toggle('is-dimmed', !show)
      }})
      document.querySelectorAll('.object-link').forEach((link) => {{
        const match = matchesFilter(link.dataset.region || '未知', link.dataset.label || '', link.dataset.kind || '')
        link.hidden = !match
      }})
    }}

    function renderDetail(nodeId) {{
      detailPanel.innerHTML = details[nodeId] || details['root:local'] || '<div class="detail-card"><p>没有可展示的详情。</p></div>'
    }}

    function updateSelection(nodeId) {{
      selectedNodeId = nodeId
      graphNodes.forEach((node) => node.classList.toggle('is-selected', node.dataset.nodeId === nodeId))
      renderDetail(nodeId)
      document.querySelectorAll('.object-link').forEach((link) => {{
        link.classList.toggle('is-selected', link.dataset.nodeId === nodeId)
      }})
    }}

    function setScale(next) {{
      scale = Math.min(2.2, Math.max(0.55, next))
      graphRoot.style.transform = 'scale(' + scale + ')'
    }}

    graphNodes.forEach((node) => {{
      node.addEventListener('click', () => updateSelection(node.dataset.nodeId))
    }})
    document.addEventListener('click', (event) => {{
      const link = event.target.closest('.object-link')
      if (!link) return
      updateSelection(link.dataset.nodeId)
    }})
    regionToggles.forEach((toggle) => toggle.addEventListener('change', applyFilters))
    searchInput.addEventListener('input', applyFilters)
    document.getElementById('graph-viewport').addEventListener('wheel', (event) => {{
      if (!event.metaKey && !event.ctrlKey) return
      event.preventDefault()
      const delta = event.deltaY < 0 ? 0.12 : -0.12
      setScale(scale + delta)
    }}, {{ passive: false }})
    document.getElementById('zoom-in').addEventListener('click', () => setScale(scale + 0.12))
    document.getElementById('zoom-out').addEventListener('click', () => setScale(scale - 0.12))
    document.getElementById('zoom-reset').addEventListener('click', () => setScale(1))
    document.getElementById('filter-reset').addEventListener('click', () => {{
      regionToggles.forEach((toggle) => {{ toggle.checked = true }})
      searchInput.value = ''
      applyFilters()
      updateSelection('root:local')
    }})

    applyFilters()
    updateSelection(selectedNodeId)
  </script>
</body>
</html>"##,
        topology.svg
    )
}

pub fn default_report_path() -> String {
    format!(
        "subroute-report-{}.html",
        Local::now().format("%Y%m%d-%H%M%S")
    )
}

fn render_summary_cards(
    source_url: &str,
    generated_at: DateTime<Local>,
    reports: &[NodeReport],
    skipped: &[SkippedNode],
) -> String {
    format!(
        r##"<div class="summary-wide"><span class="label">订阅地址</span><span class="value">{}</span></div>
<div class="summary-grid">
  <div class="summary-card"><span class="label">生成时间</span><span class="value">{}</span></div>
  <div class="summary-card"><span class="label">总条目数</span><span class="value">{}</span></div>
  <div class="summary-card"><span class="label">可测节点数</span><span class="value">{}</span></div>
  <div class="summary-card"><span class="label">无法解析条目数</span><span class="value">{}</span></div>
</div>"##,
        escape_html(source_url),
        escape_html(&generated_at.format("%Y-%m-%d %H:%M:%S %z").to_string()),
        reports.len() + skipped.len(),
        reports.len(),
        skipped.len(),
    )
}

fn render_region_filter(nodes: &[TopologyNodeSummary]) -> String {
    let mut regions = nodes
        .iter()
        .filter(|node| node.kind != "root")
        .map(|node| node.region.clone())
        .collect::<Vec<_>>();
    regions.sort();
    regions.dedup();

    let filters = regions
        .iter()
        .map(|region| {
            format!(
                r##"<label><input class="region-toggle" type="checkbox" value="{}" checked /> {}</label>"##,
                escape_html(region),
                escape_html(region)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r##"<div class="region-filter"><span class="region-filter-title">地区筛选</span>{filters}</div>"##
    )
}

fn render_region_groups(nodes: &[TopologyNodeSummary]) -> String {
    let mut groups = BTreeMap::<String, RegionGroup>::new();

    for node in nodes
        .iter()
        .filter(|node| node.kind == "as" || node.kind == "leaf")
    {
        let group = groups.entry(node.region.clone()).or_default();
        let item = ObjectLink {
            id: node.id.clone(),
            label: if node.kind == "as" {
                node.asn
                    .as_ref()
                    .map(|asn| format!("AS{asn}"))
                    .unwrap_or_else(|| node.label.clone())
            } else {
                node.label.clone()
            },
            kind: node.kind.clone(),
            region: node.region.clone(),
        };
        if node.kind == "as" {
            group.as_nodes.push(item);
        } else {
            group.leaf_nodes.push(item);
        }
    }

    let mut output = String::new();
    output.push_str(r##"<div class="region-groups">"##);
    for (region, mut group) in groups {
        group
            .as_nodes
            .sort_by(|left, right| left.label.cmp(&right.label));
        group
            .leaf_nodes
            .sort_by(|left, right| left.label.cmp(&right.label));

        output.push_str(&format!(
            r##"<details><summary>{}</summary><div class="object-columns">"##,
            escape_html(&region)
        ));
        output.push_str(&render_object_column("AS", &group.as_nodes));
        output.push_str(&render_object_column("机场节点", &group.leaf_nodes));
        output.push_str("</div></details>");
    }
    output.push_str("</div>");
    output
}

fn render_object_column(title: &str, items: &[ObjectLink]) -> String {
    let content = if items.is_empty() {
        "<span>无</span>".to_string()
    } else {
        items
            .iter()
            .map(|item| {
                format!(
                    r##"<button type="button" class="object-link" data-node-id="{}" data-kind="{}" data-region="{}" data-label="{}">{}</button>"##,
                    escape_html(&item.id),
                    escape_html(&item.kind),
                    escape_html(&item.region),
                    escape_html(&item.label),
                    escape_html(&item.label)
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        r##"<div class="object-column"><h4>{}</h4><div class="object-list">{}</div></div>"##,
        escape_html(title),
        content
    )
}

fn build_detail_map(
    source_url: &str,
    generated_at: DateTime<Local>,
    reports: &[NodeReport],
    skipped: &[SkippedNode],
    nodes: &[TopologyNodeSummary],
) -> BTreeMap<String, String> {
    let mut details = BTreeMap::new();
    let node_lookup = nodes
        .iter()
        .map(|node| (node.id.clone(), node.clone()))
        .collect::<HashMap<_, _>>();
    let report_lookup = reports
        .iter()
        .map(|report| (leaf_node_id(&report.probe.node), report))
        .collect::<HashMap<_, _>>();

    details.insert(
        "root:local".to_string(),
        render_root_detail(source_url, generated_at, reports, skipped),
    );

    for node in nodes {
        if node.kind == "root" {
            continue;
        }

        let html = if node.kind == "as" {
            render_as_detail(node, &node_lookup)
        } else if node.kind == "group" {
            render_group_detail(node, &node_lookup)
        } else {
            render_leaf_detail(node, report_lookup.get(&node.id).copied())
        };
        details.insert(node.id.clone(), html);
    }

    details
}

fn render_root_detail(
    source_url: &str,
    generated_at: DateTime<Local>,
    reports: &[NodeReport],
    skipped: &[SkippedNode],
) -> String {
    format!(
        r##"<div class="detail-card"><h3>总览</h3><div class="detail-meta"><div><span class="label">订阅地址</span>{}</div><div><span class="label">生成时间</span>{}</div><div><span class="label">可测节点数</span>{}</div><div><span class="label">无法解析条目数</span>{}</div></div><p>点击上方关系图中的 AS、末端组节点或具体机场节点，可以在这里查看对应的详细信息。</p></div>"##,
        escape_html(source_url),
        escape_html(&generated_at.format("%Y-%m-%d %H:%M:%S %z").to_string()),
        reports.len(),
        skipped.len(),
    )
}

fn render_as_detail(
    node: &TopologyNodeSummary,
    node_lookup: &HashMap<String, TopologyNodeSummary>,
) -> String {
    let upstream_labels = labels_from_ids(&node.upstream_ids, node_lookup);
    let downstream_labels = labels_from_ids(&node.downstream_ids, node_lookup);
    let related_leaf_labels = labels_from_ids(&node.related_leaf_ids, node_lookup);

    format!(
        r##"<div class="detail-card"><h3>{}</h3><div class="detail-meta"><div><span class="label">地区</span>{}</div><div><span class="label">组织</span>{}</div><div><span class="label">上游</span>{}</div><div><span class="label">下游</span>{}</div></div><h4>关联机场节点</h4>{}</div>"##,
        escape_html(&node.label),
        escape_html(&node.region),
        escape_html(node.org.as_deref().unwrap_or("未知")),
        escape_html(&upstream_labels.join("、")),
        escape_html(&downstream_labels.join("、")),
        render_detail_list(&related_leaf_labels),
    )
}

fn render_group_detail(
    node: &TopologyNodeSummary,
    node_lookup: &HashMap<String, TopologyNodeSummary>,
) -> String {
    let upstream_labels = labels_from_ids(&node.upstream_ids, node_lookup);
    let member_nodes = node
        .related_leaf_ids
        .iter()
        .filter_map(|id| node_lookup.get(id))
        .collect::<Vec<_>>();

    format!(
        r##"<div class="detail-card"><h3>{}</h3><div class="detail-meta"><div><span class="label">地区</span>{}</div><div><span class="label">组织</span>{}</div><div><span class="label">上游</span>{}</div><div><span class="label">节点数</span>{}</div></div><h4>组内机场节点</h4>{}</div>"##,
        escape_html(&node.label),
        escape_html(&node.region),
        escape_html(node.org.as_deref().unwrap_or("未知")),
        escape_html(&upstream_labels.join("、")),
        member_nodes.len(),
        render_detail_button_list(&member_nodes),
    )
}

fn render_leaf_detail(node: &TopologyNodeSummary, report: Option<&NodeReport>) -> String {
    let Some(report) = report else {
        return format!(
            r##"<div class="detail-card"><h3>{}</h3><p>没有找到该节点的详细路由信息。</p></div>"##,
            escape_html(&node.label)
        );
    };

    let rows = collect_route_rows(report);
    let rows_html = if rows.is_empty() {
        "<tr><td colspan=\"4\">没有可展示的路径信息</td></tr>".to_string()
    } else {
        rows.into_iter()
            .map(|row| {
                format!(
                    r##"<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>"##,
                    escape_html(&row.ip),
                    escape_html(&row.region),
                    escape_html(&row.asn),
                    escape_html(&row.org)
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        r##"<div class="detail-card"><h3>{}</h3><div class="detail-meta"><div><span class="label">协议</span>{}</div><div><span class="label">入口</span>{}</div><div><span class="label">端口</span>{}</div><div><span class="label">Ping</span>{}</div></div><table><thead><tr><th>IP</th><th>地区</th><th>AS 号</th><th>组织</th></tr></thead><tbody>{}</tbody></table></div>"##,
        escape_html(&report.probe.node.name),
        escape_html(&report.probe.node.protocol),
        escape_html(&report.probe.node.host),
        report.probe.node.port,
        escape_html(&render_ping_summary(&report.probe.ping)),
        rows_html,
    )
}

fn render_detail_list(items: &[String]) -> String {
    if items.is_empty() {
        return "<p>无</p>".to_string();
    }
    let items_html = items
        .iter()
        .map(|item| format!(r##"<li>{}</li>"##, escape_html(item)))
        .collect::<Vec<_>>()
        .join("");
    format!(r##"<ul class="detail-list">{items_html}</ul>"##)
}

fn render_detail_button_list(items: &[&TopologyNodeSummary]) -> String {
    if items.is_empty() {
        return "<p>无</p>".to_string();
    }

    let buttons = items
        .iter()
        .map(|item| {
            format!(
                r##"<button type="button" class="object-link" data-node-id="{}" data-kind="{}" data-region="{}" data-label="{}">{}</button>"##,
                escape_html(&item.id),
                escape_html(&item.kind),
                escape_html(&item.region),
                escape_html(&item.label),
                escape_html(&item.label)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(r##"<div class="object-list">{buttons}</div>"##)
}

fn labels_from_ids(
    ids: &[String],
    node_lookup: &HashMap<String, TopologyNodeSummary>,
) -> Vec<String> {
    let mut labels = ids
        .iter()
        .filter_map(|id| node_lookup.get(id))
        .map(|node| node.label.clone())
        .collect::<Vec<_>>();
    labels.sort();
    labels.dedup();
    labels
}

fn render_ping_summary(result: &Result<PingResult, String>) -> String {
    let result = match result {
        Ok(result) => result,
        Err(err) => return format!("失败: {err}"),
    };

    let delivery = match (result.transmitted, result.received, result.loss_percent) {
        (Some(transmitted), Some(received), Some(loss)) => {
            format!("{received}/{transmitted} 收到，丢包 {loss:.1}%")
        }
        _ => "统计缺失".to_string(),
    };

    if result.times_ms.is_empty() {
        return delivery;
    }

    let times = result
        .times_ms
        .iter()
        .map(|value| format!("{value:.2}"))
        .collect::<Vec<_>>()
        .join(", ");

    format!("{delivery}，往返 [{times}] ms")
}

fn collect_route_rows(report: &NodeReport) -> Vec<RouteRow> {
    let Ok(result) = &report.probe.traceroute else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    let mut seen_ips = HashSet::new();
    for hop in &result.hops {
        for ip in hop.probes.iter().filter_map(|probe| probe.ip) {
            if !seen_ips.insert(ip) {
                continue;
            }
            rows.push(build_route_row(ip, report.annotations.get(&ip)));
        }
    }
    rows
}

fn build_route_row(ip: IpAddr, annotation: Option<&IpAnnotation>) -> RouteRow {
    match annotation {
        Some(annotation) => RouteRow {
            ip: ip.to_string(),
            region: annotation_region(annotation),
            asn: annotation
                .asn
                .as_ref()
                .map(|value| format!("AS{value}"))
                .unwrap_or_else(|| "未知".to_string()),
            org: annotation.org.clone().unwrap_or_else(|| "未知".to_string()),
        },
        None => RouteRow {
            ip: ip.to_string(),
            region: "未知".to_string(),
            asn: "未知".to_string(),
            org: "未知".to_string(),
        },
    }
}

fn annotation_region(annotation: &IpAnnotation) -> String {
    annotation
        .location
        .clone()
        .or_else(|| {
            annotation
                .note
                .as_ref()
                .filter(|note| !note.contains("AS 查询失败"))
                .cloned()
        })
        .unwrap_or_else(|| "未知".to_string())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Default)]
struct RegionGroup {
    as_nodes: Vec<ObjectLink>,
    leaf_nodes: Vec<ObjectLink>,
}

struct ObjectLink {
    id: String,
    label: String,
    kind: String,
    region: String,
}

struct RouteRow {
    ip: String,
    region: String,
    asn: String,
    org: String,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};
    use std::path::Path;

    use chrono::Local;

    use crate::model::{
        IpAnnotation, NodeProbeResult, NodeReport, ParsedNode, PingResult, SkippedNode, TraceHop,
        TraceProbe, TracerouteResult,
    };

    use super::{default_report_path, render_report};

    #[test]
    fn renders_html_page_with_inline_graph_and_detail_panel() {
        let ip = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let mut annotations = HashMap::new();
        annotations.insert(
            ip,
            IpAnnotation {
                ip,
                asn: Some("13335".to_string()),
                org: Some("Cloudflare".to_string()),
                location: Some("美国".to_string()),
                note: None,
            },
        );

        let report = NodeReport {
            probe: NodeProbeResult {
                node: ParsedNode {
                    protocol: "vless".to_string(),
                    name: "US Node".to_string(),
                    host: "example.com".to_string(),
                    port: 443,
                    source_line: "vless://...".to_string(),
                },
                ping: Ok(PingResult {
                    raw_output: String::new(),
                    times_ms: vec![10.1, 10.2],
                    transmitted: Some(2),
                    received: Some(2),
                    loss_percent: Some(0.0),
                }),
                traceroute: Ok(TracerouteResult {
                    raw_output: String::new(),
                    hops: vec![TraceHop {
                        ttl: 5,
                        raw_line: "5  1.1.1.1  10.1 ms".to_string(),
                        probes: vec![TraceProbe {
                            ip: Some(ip),
                            latency_ms: Some(10.1),
                        }],
                    }],
                }),
            },
            annotations,
        };

        let html = render_report(
            "https://example.com/sub.txt",
            Local::now(),
            &[report],
            &[SkippedNode {
                source_line: "bad-line".to_string(),
                reason: "缺少协议头".to_string(),
            }],
        );

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Subroute 路由关系图"));
        assert!(html.contains("id=\"graph-root\""));
        assert!(html.contains("id=\"detail-panel\""));
        assert!(html.contains("US Node"));
        assert!(html.contains("AS13335"));
        assert!(html.contains("Cloudflare"));
        assert!(html.contains("地区筛选"));
        assert!(html.contains("graph-node"));
    }

    #[test]
    fn default_report_path_uses_html_extension() {
        let path = default_report_path();
        assert!(
            Path::new(&path)
                .extension()
                .is_some_and(|ext| ext == "html")
        );
        assert!(path.starts_with("subroute-report-"));
    }
}
