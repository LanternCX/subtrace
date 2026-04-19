use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use serde::Serialize;

use crate::model::{IpAnnotation, NodeReport, ParsedNode};

const COLUMN_GAP: f32 = 120.0;
const ROW_GAP: f32 = 28.0;
const TOP_MARGIN: f32 = 40.0;
const SIDE_MARGIN: f32 = 36.0;
const NODE_PADDING_Y: f32 = 12.0;
const LINE_HEIGHT: f32 = 18.0;

pub fn render_topology_svg(reports: &[NodeReport]) -> String {
    build_topology_view(reports).svg
}

pub fn leaf_node_id(node: &ParsedNode) -> String {
    format!(
        "leaf:{}::{}@{}:{}",
        node.name, node.protocol, node.host, node.port
    )
}

pub fn build_topology_view(reports: &[NodeReport]) -> TopologyView {
    let graph = build_graph(reports);
    let svg = render_svg(&graph);
    let nodes = build_node_summaries(&graph);

    TopologyView { svg, nodes }
}

#[derive(Debug, Clone)]
pub struct TopologyView {
    pub svg: String,
    pub nodes: Vec<TopologyNodeSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopologyNodeSummary {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub region: String,
    pub asn: Option<String>,
    pub org: Option<String>,
    pub upstream_ids: Vec<String>,
    pub downstream_ids: Vec<String>,
    pub related_leaf_ids: Vec<String>,
}

fn build_node_summaries(graph: &GraphData) -> Vec<TopologyNodeSummary> {
    let upstreams = build_neighbors(&graph.edges, true);
    let downstreams = build_neighbors(&graph.edges, false);

    let mut summaries = graph
        .nodes
        .iter()
        .map(|(node_id, meta)| TopologyNodeSummary {
            id: node_id.to_string(),
            kind: meta.kind.as_str().to_string(),
            label: meta.label.clone(),
            region: meta.region.clone(),
            asn: meta.asn.clone(),
            org: meta.org.clone(),
            upstream_ids: upstreams
                .get(node_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|value| value.to_string())
                .collect(),
            downstream_ids: downstreams
                .get(node_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|value| value.to_string())
                .collect(),
            related_leaf_ids: meta.related_leaf_ids.iter().cloned().collect(),
        })
        .chain(
            graph
                .leaf_nodes
                .iter()
                .map(|(node_id, meta)| TopologyNodeSummary {
                    id: node_id.to_string(),
                    kind: meta.kind.as_str().to_string(),
                    label: meta.label.clone(),
                    region: meta.region.clone(),
                    asn: None,
                    org: None,
                    upstream_ids: meta.upstream_ids.iter().cloned().collect(),
                    downstream_ids: Vec::new(),
                    related_leaf_ids: Vec::new(),
                }),
        )
        .collect::<Vec<_>>();

    summaries.sort_by(|left, right| left.id.cmp(&right.id));
    summaries
}

fn render_svg(graph: &GraphData) -> String {
    if graph.nodes.len() <= 1 {
        return render_empty_svg("无可绘制路径");
    }

    let layout = layout_graph(graph);
    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{:.0}" height="{:.0}" viewBox="0 0 {:.0} {:.0}" role="img" aria-label="Subroute 路由关系图">"##,
        layout.width, layout.height, layout.width, layout.height
    ));
    svg.push_str(
        r##"<style>
.graph-bg{fill:#ffffff}
.graph-edge{fill:none;stroke:#cbd5e1;stroke-width:1.5}
.graph-edge.is-dimmed{opacity:.12}
.graph-node{cursor:pointer;transition:opacity .15s ease}
.graph-node.is-dimmed{opacity:.18}
.graph-node.is-selected rect{stroke:#f97316;stroke-width:2.4}
.graph-node:hover rect{stroke:#fb7185;stroke-width:2}
.graph-node.root rect{fill:#f8fafc;stroke:#64748b}
.graph-node.as rect{fill:#eff6ff;stroke:#3b82f6}
.graph-node.group rect{fill:#fff7ed;stroke:#f97316}
.graph-label{fill:#0f172a;font-size:13px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif}
</style>"##,
    );
    svg.push_str(&format!(
        r##"<rect class="graph-bg" x="0" y="0" width="{:.0}" height="{:.0}"/>"##,
        layout.width, layout.height
    ));

    svg.push_str("<g>");
    for (from, to) in &graph.edges {
        let (Some(source), Some(target)) = (layout.positions.get(from), layout.positions.get(to))
        else {
            continue;
        };
        svg.push_str(&render_edge(from, to, source, target));
    }
    svg.push_str("</g><g>");
    for node_id in &layout.render_order {
        if let (Some(meta), Some(position)) =
            (graph.nodes.get(node_id), layout.positions.get(node_id))
        {
            svg.push_str(&render_node(node_id, meta, position));
        }
    }
    svg.push_str("</g></svg>");
    svg
}

fn layout_graph(graph: &GraphData) -> GraphLayout {
    let depths = compute_depths(graph);
    let mut columns = BTreeMap::<usize, Vec<NodeId>>::new();
    for node_id in graph.nodes.keys() {
        let depth = depths.get(node_id).copied().unwrap_or(0);
        columns.entry(depth).or_default().push(node_id.clone());
    }

    let mut ordered_columns = Vec::<(usize, Vec<NodeId>)>::new();
    let mut previous_column = Vec::<NodeId>::new();
    for (depth, mut node_ids) in columns {
        node_ids.sort_by(|left, right| {
            barycenter(left, &previous_column, graph)
                .partial_cmp(&barycenter(right, &previous_column, graph))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| node_sort_key(left, graph).cmp(&node_sort_key(right, graph)))
        });
        previous_column = node_ids.clone();
        ordered_columns.push((depth, node_ids));
    }

    let mut column_widths = Vec::new();
    let mut column_heights = Vec::new();
    for (_, node_ids) in &ordered_columns {
        let width = node_ids
            .iter()
            .map(|node_id| node_size(node_id, graph).width)
            .fold(0.0, f32::max);
        let height = node_ids
            .iter()
            .map(|node_id| node_size(node_id, graph).height)
            .sum::<f32>()
            + ROW_GAP * node_ids.len().saturating_sub(1) as f32;
        column_widths.push(width);
        column_heights.push(height);
    }
    let max_height = column_heights
        .iter()
        .copied()
        .fold(0.0, f32::max)
        .max(320.0);

    let mut positions = HashMap::new();
    let mut render_order = Vec::new();
    let mut current_x = SIDE_MARGIN;
    for (column_index, (_, node_ids)) in ordered_columns.iter().enumerate() {
        let column_width = column_widths[column_index];
        let total_height = column_heights[column_index];
        let mut current_y = TOP_MARGIN + (max_height - total_height) / 2.0;

        for node_id in node_ids {
            let size = node_size(node_id, graph);
            let x = current_x + (column_width - size.width) / 2.0;
            positions.insert(
                node_id.clone(),
                NodePosition {
                    x,
                    y: current_y,
                    width: size.width,
                    height: size.height,
                },
            );
            render_order.push(node_id.clone());
            current_y += size.height + ROW_GAP;
        }

        current_x += column_width + COLUMN_GAP;
    }

    GraphLayout {
        width: current_x + SIDE_MARGIN - COLUMN_GAP,
        height: max_height + TOP_MARGIN * 2.0,
        positions,
        render_order,
    }
}

fn barycenter(node_id: &NodeId, previous_column: &[NodeId], graph: &GraphData) -> f32 {
    if previous_column.is_empty() {
        return f32::MAX / 2.0;
    }

    let mut indices = graph
        .edges
        .iter()
        .filter_map(|(from, to)| (to == node_id).then_some(from))
        .filter_map(|parent| {
            previous_column
                .iter()
                .position(|candidate| candidate == parent)
        })
        .collect::<Vec<_>>();
    if indices.is_empty() {
        return f32::MAX / 2.0;
    }
    indices.sort_unstable();
    indices.iter().sum::<usize>() as f32 / indices.len() as f32
}

fn compute_depths(graph: &GraphData) -> HashMap<NodeId, usize> {
    let mut depths = HashMap::new();
    let mut queue = VecDeque::new();
    depths.insert(NodeId::Root, 0usize);
    queue.push_back(NodeId::Root);

    while let Some(node_id) = queue.pop_front() {
        let depth = depths.get(&node_id).copied().unwrap_or(0);
        for (from, to) in &graph.edges {
            if *from != node_id {
                continue;
            }
            let next_depth = depth + 1;
            let should_update = depths
                .get(to)
                .map(|existing| next_depth < *existing)
                .unwrap_or(true);
            if should_update {
                depths.insert(to.clone(), next_depth);
                queue.push_back(to.clone());
            }
        }
    }

    depths
}

fn render_edge(from: &NodeId, to: &NodeId, source: &NodePosition, target: &NodePosition) -> String {
    let start_x = source.x + source.width;
    let start_y = source.y + source.height / 2.0;
    let end_x = target.x;
    let end_y = target.y + target.height / 2.0;
    let control_x = ((end_x - start_x) / 2.0).max(40.0);

    format!(
        r##"<path class="graph-edge" data-source="{}" data-target="{}" d="M {:.1} {:.1} C {:.1} {:.1}, {:.1} {:.1}, {:.1} {:.1}"/>"##,
        from,
        to,
        start_x,
        start_y,
        start_x + control_x,
        start_y,
        end_x - control_x,
        end_y,
        end_x,
        end_y,
    )
}

fn render_node(node_id: &NodeId, meta: &NodeMeta, position: &NodePosition) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        r##"<g class="graph-node {}" data-node-id="{}" data-kind="{}" data-region="{}" data-label="{}">"##,
        meta.kind.as_str(),
        node_id,
        meta.kind.as_str(),
        escape_xml(&meta.region),
        escape_xml(&meta.label),
    ));
    output.push_str(&format!(
        r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="12" ry="12"/>"##,
        position.x, position.y, position.width, position.height,
    ));

    let center_x = position.x + position.width / 2.0;
    let mut current_y = position.y + NODE_PADDING_Y + LINE_HEIGHT - 4.0;
    for line in node_lines(meta) {
        output.push_str(&format!(
            r##"<text class="graph-label" x="{:.1}" y="{:.1}" text-anchor="middle">{}</text>"##,
            center_x,
            current_y,
            escape_xml(&line),
        ));
        current_y += LINE_HEIGHT;
    }

    output.push_str("</g>");
    output
}

fn render_empty_svg(message: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="220" viewBox="0 0 800 220" role="img" aria-label="Subroute 路由关系图"><rect x="0" y="0" width="800" height="220" fill="#ffffff"/><rect x="180" y="60" width="440" height="100" rx="14" ry="14" fill="#f8fafc" stroke="#94a3b8" stroke-width="1.4"/><text x="400" y="120" text-anchor="middle" fill="#0f172a" font-size="18" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif">{}</text></svg>"##,
        escape_xml(message)
    )
}

fn build_neighbors(
    edges: &BTreeSet<(NodeId, NodeId)>,
    reverse: bool,
) -> HashMap<NodeId, Vec<NodeId>> {
    let mut neighbors = HashMap::<NodeId, BTreeSet<NodeId>>::new();
    for (from, to) in edges {
        let (key, value) = if reverse {
            (to.clone(), from.clone())
        } else {
            (from.clone(), to.clone())
        };
        neighbors.entry(key).or_default().insert(value);
    }

    neighbors
        .into_iter()
        .map(|(node_id, values)| (node_id, values.into_iter().collect()))
        .collect()
}

fn build_graph(reports: &[NodeReport]) -> GraphData {
    let mut nodes = BTreeMap::<NodeId, NodeMeta>::new();
    let mut leaf_nodes = BTreeMap::<NodeId, NodeMeta>::new();
    let mut edges = BTreeSet::<(NodeId, NodeId)>::new();

    nodes.insert(
        NodeId::Root,
        NodeMeta {
            kind: GraphNodeKind::Root,
            label: "本机".to_string(),
            region: "本机".to_string(),
            asn: None,
            org: None,
            related_leaf_ids: BTreeSet::new(),
            upstream_ids: BTreeSet::new(),
        },
    );

    for report in reports {
        let leaf_id = NodeId::Leaf(leaf_node_id(&report.probe.node));
        leaf_nodes
            .entry(leaf_id.clone())
            .or_insert_with(|| NodeMeta {
                kind: GraphNodeKind::Leaf,
                label: report.probe.node.name.clone(),
                region: detect_country_group(&report.probe.node.name).to_string(),
                asn: None,
                org: None,
                related_leaf_ids: BTreeSet::new(),
                upstream_ids: BTreeSet::new(),
            });

        let sequence = extract_as_sequence(report);
        let graph_sequence = collapse_graph_sequence(&sequence);
        let mut previous = NodeId::Root;
        for (asn, annotation) in &graph_sequence {
            let as_id = NodeId::As(asn.clone());
            nodes
                .entry(as_id.clone())
                .and_modify(|meta| merge_as_meta(meta, annotation, &leaf_id))
                .or_insert_with(|| NodeMeta {
                    kind: GraphNodeKind::As,
                    label: format!("AS{asn}"),
                    region: annotation_region(annotation),
                    asn: Some(asn.clone()),
                    org: annotation.org.clone(),
                    related_leaf_ids: BTreeSet::from([leaf_id.to_string()]),
                    upstream_ids: BTreeSet::new(),
                });

            if previous != as_id {
                edges.insert((previous.clone(), as_id.clone()));
            }
            previous = as_id;
        }

        let final_asn = sequence.last().cloned();

        let group_id = build_group_id(final_asn.as_ref().map(|(asn, _)| asn.as_str()));
        let (group_label, group_region, group_org, group_asn) = match final_asn.as_ref() {
            Some((asn, annotation)) => (
                format!("终点节点\nAS{asn}"),
                annotation_region(annotation),
                annotation.org.clone().unwrap_or_else(|| "未知".to_string()),
                Some(asn.clone()),
            ),
            None => (
                "未识别终点".to_string(),
                "未分类".to_string(),
                "未知".to_string(),
                None,
            ),
        };

        nodes
            .entry(group_id.clone())
            .and_modify(|meta| {
                meta.related_leaf_ids.insert(leaf_id.to_string());
                if meta.region != group_region {
                    meta.region = "多地区".to_string();
                }
                if meta.org.as_deref() != Some(&group_org) && group_org != "未知" {
                    meta.org = Some("多个组织".to_string());
                }
            })
            .or_insert_with(|| NodeMeta {
                kind: GraphNodeKind::Group,
                label: group_label,
                region: group_region,
                asn: group_asn,
                org: Some(group_org),
                related_leaf_ids: BTreeSet::from([leaf_id.to_string()]),
                upstream_ids: BTreeSet::new(),
            });

        edges.insert((previous.clone(), group_id.clone()));
        if let Some(meta) = leaf_nodes.get_mut(&leaf_id) {
            meta.upstream_ids.insert(group_id.to_string());
        }
    }

    GraphData {
        nodes,
        leaf_nodes,
        edges,
    }
}

fn extract_as_sequence(report: &NodeReport) -> Vec<(String, IpAnnotation)> {
    let Ok(result) = &report.probe.traceroute else {
        return Vec::new();
    };

    let mut sequence = Vec::new();

    for hop in &result.hops {
        let mut hop_seen = HashSet::new();
        for ip in hop.probes.iter().filter_map(|probe| probe.ip) {
            let Some(annotation) = report.annotations.get(&ip) else {
                continue;
            };
            let Some(asn) = annotation.asn.clone() else {
                continue;
            };
            if !hop_seen.insert(asn.clone()) {
                continue;
            }

            if sequence.last().map(|(last_asn, _)| last_asn) == Some(&asn) {
                continue;
            }

            sequence.push((asn, annotation.clone()));
        }
    }

    sequence
}

fn collapse_graph_sequence(sequence: &[(String, IpAnnotation)]) -> Vec<(String, IpAnnotation)> {
    let mut seen = HashSet::new();
    let mut collapsed = Vec::new();

    for (asn, annotation) in sequence {
        if seen.insert(asn.clone()) {
            collapsed.push((asn.clone(), annotation.clone()));
        }
    }

    collapsed
}

fn build_group_id(asn: Option<&str>) -> NodeId {
    match asn {
        Some(asn) => NodeId::Group(asn.to_string()),
        None => NodeId::Group("unknown".to_string()),
    }
}

fn merge_as_meta(target: &mut NodeMeta, annotation: &IpAnnotation, leaf_id: &NodeId) {
    let region = annotation_region(annotation);
    if target.region == "未知" {
        target.region = region.clone();
    } else if region != "未知" && target.region != region {
        target.region = "多地区".to_string();
    }

    let org = annotation.org.clone().unwrap_or_else(|| "未知".to_string());
    target.org = match &target.org {
        None => Some(org),
        Some(existing) if existing == &org || org == "未知" => Some(existing.clone()),
        Some(existing) if existing == "未知" => Some(org),
        Some(_) => Some("多个组织".to_string()),
    };

    target.related_leaf_ids.insert(leaf_id.to_string());
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

fn detect_country_group(name: &str) -> &'static str {
    if contains_any(
        name,
        &[
            "美国",
            "美國",
            "🇺🇸",
            "圣何塞",
            "聖荷西",
            "洛杉矶",
            "洛杉磯",
            "阿什本",
        ],
    ) {
        return "美国";
    }
    if contains_any(name, &["日本", "東京", "东京", "🇯🇵"]) {
        return "日本";
    }
    if contains_any(name, &["香港", "🇭🇰"]) {
        return "香港";
    }
    if contains_any(name, &["新加坡", "狮城", "獅城", "🇸🇬"]) {
        return "新加坡";
    }
    if contains_any(name, &["台湾", "台灣", "🇹🇼"]) {
        return "台湾";
    }
    "未分类"
}

fn contains_any(name: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| name.contains(pattern))
}

fn node_size(node_id: &NodeId, graph: &GraphData) -> NodeSize {
    let meta = graph.nodes.get(node_id).expect("node meta should exist");
    let width = match meta.kind {
        GraphNodeKind::Root => 110.0,
        GraphNodeKind::As => 230.0,
        GraphNodeKind::Group => 180.0,
        GraphNodeKind::Leaf => 200.0,
    };
    let lines = node_lines(meta);
    NodeSize {
        width,
        height: NODE_PADDING_Y * 2.0 + lines.len() as f32 * LINE_HEIGHT,
    }
}

fn node_sort_key(node_id: &NodeId, graph: &GraphData) -> (u8, String) {
    let meta = graph.nodes.get(node_id).expect("node meta should exist");
    let priority = match meta.kind {
        GraphNodeKind::Root => 0,
        GraphNodeKind::As => 1,
        GraphNodeKind::Group => 2,
        GraphNodeKind::Leaf => 3,
    };
    (priority, meta.label.clone())
}

fn node_lines(meta: &NodeMeta) -> Vec<String> {
    match meta.kind {
        GraphNodeKind::Root => vec![meta.label.clone()],
        GraphNodeKind::As => {
            let mut lines = vec![
                meta.asn
                    .as_ref()
                    .map(|asn| format!("AS{asn}"))
                    .unwrap_or_else(|| meta.label.clone()),
                meta.region.clone(),
            ];
            lines.extend(wrap_text(
                meta.org
                    .clone()
                    .unwrap_or_else(|| "未知".to_string())
                    .as_str(),
                22,
                2,
            ));
            lines
        }
        GraphNodeKind::Group => {
            let count = meta.related_leaf_ids.len();
            if let Some(asn) = &meta.asn {
                vec![format!("AS{asn} 终点组"), format!("{count} 个节点")]
            } else {
                vec!["未识别终点".to_string(), format!("{count} 个节点")]
            }
        }
        GraphNodeKind::Leaf => wrap_text(&meta.label, 18, 3),
    }
}

fn wrap_text(value: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    if value.is_empty() {
        return vec!["未知".to_string()];
    }
    let chars = value.chars().collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut start = 0usize;
    while start < chars.len() && lines.len() < max_lines {
        let end = (start + max_chars).min(chars.len());
        let mut line = chars[start..end].iter().collect::<String>();
        if end < chars.len() && lines.len() + 1 == max_lines && line.len() >= 2 {
            line.pop();
            line.push('…');
        }
        lines.push(line);
        start = end;
    }
    lines
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Debug, Clone, Copy)]
struct NodeSize {
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, Copy)]
struct NodePosition {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Debug)]
struct GraphLayout {
    width: f32,
    height: f32,
    positions: HashMap<NodeId, NodePosition>,
    render_order: Vec<NodeId>,
}

#[derive(Debug, Default)]
struct GraphData {
    nodes: BTreeMap<NodeId, NodeMeta>,
    leaf_nodes: BTreeMap<NodeId, NodeMeta>,
    edges: BTreeSet<(NodeId, NodeId)>,
}

#[derive(Debug, Clone)]
struct NodeMeta {
    kind: GraphNodeKind,
    label: String,
    region: String,
    asn: Option<String>,
    org: Option<String>,
    related_leaf_ids: BTreeSet<String>,
    upstream_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodeKind {
    Root,
    As,
    Group,
    Leaf,
}

impl GraphNodeKind {
    fn as_str(self) -> &'static str {
        match self {
            GraphNodeKind::Root => "root",
            GraphNodeKind::As => "as",
            GraphNodeKind::Group => "group",
            GraphNodeKind::Leaf => "leaf",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum NodeId {
    Root,
    As(String),
    Group(String),
    Leaf(String),
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeId::Root => write!(f, "root:local"),
            NodeId::As(asn) => write!(f, "as:{asn}"),
            NodeId::Group(asn) => write!(f, "group:{asn}"),
            NodeId::Leaf(name) => write!(f, "leaf:{name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};

    use crate::model::{
        IpAnnotation, NodeProbeResult, NodeReport, ParsedNode, PingResult, TraceHop, TraceProbe,
        TracerouteResult,
    };

    use super::{build_topology_view, render_topology_svg};

    fn build_report(name: &str, hops: &[(u8, u8, &str, &str, &str)]) -> NodeReport {
        let mut annotations = HashMap::new();
        let trace_hops = hops
            .iter()
            .map(|(ttl, last_octet, asn, region, org)| {
                let ip = IpAddr::V4(Ipv4Addr::new(1, 1, 1, *last_octet));
                annotations.insert(
                    ip,
                    IpAnnotation {
                        ip,
                        asn: Some((*asn).to_string()),
                        org: Some((*org).to_string()),
                        location: Some((*region).to_string()),
                        note: None,
                    },
                );
                TraceHop {
                    ttl: *ttl,
                    raw_line: "hop".to_string(),
                    probes: vec![TraceProbe {
                        ip: Some(ip),
                        latency_ms: Some(1.0),
                    }],
                }
            })
            .collect::<Vec<_>>();

        NodeReport {
            probe: NodeProbeResult {
                node: ParsedNode {
                    protocol: "vless".to_string(),
                    name: name.to_string(),
                    host: "example.com".to_string(),
                    port: 443,
                    source_line: "vless://...".to_string(),
                },
                ping: Ok(PingResult {
                    raw_output: String::new(),
                    times_ms: vec![1.0],
                    transmitted: Some(1),
                    received: Some(1),
                    loss_percent: Some(0.0),
                }),
                traceroute: Ok(TracerouteResult {
                    raw_output: String::new(),
                    hops: trace_hops,
                }),
            },
            annotations,
        }
    }

    #[test]
    fn keeps_single_as_node_when_same_as_appears_multiple_times() {
        let svg = render_topology_svg(&[
            build_report("Node A", &[(5, 1, "4134", "中国", "ChinaNet")]),
            build_report("Node B", &[(9, 2, "4134", "中国", "ChinaNet")]),
        ]);

        assert_eq!(svg.matches("data-node-id=\"as:4134\"").count(), 1);
        assert_eq!(svg.matches("data-node-id=\"group:4134\"").count(), 1);
    }

    #[test]
    fn renders_horizontal_graph_with_group_nodes() {
        let svg = render_topology_svg(&[build_report(
            "US Node",
            &[(5, 1, "13335", "美国", "Cloudflare")],
        )]);

        assert!(svg.contains("<svg"));
        assert!(svg.contains("data-node-id=\"as:13335\""));
        assert!(svg.contains("data-node-id=\"group:13335\""));
        assert!(!svg.contains("data-node-id=\"leaf:US Node\""));
        assert!(svg.contains("AS13335 终点组"));
    }

    #[test]
    fn does_not_create_backward_edge_when_path_returns_to_previous_as() {
        let svg = render_topology_svg(&[build_report(
            "Loop Node",
            &[
                (5, 1, "4134", "中国", "ChinaNet"),
                (6, 2, "13335", "美国", "Cloudflare"),
                (7, 3, "4134", "中国", "ChinaNet"),
            ],
        )]);

        assert!(svg.contains("data-node-id=\"group:4134\""));
        assert!(!svg.contains("data-source=\"as:13335\" data-target=\"as:4134\""));
    }

    #[test]
    fn keeps_duplicate_node_names_as_separate_leaf_entries() {
        let mut first = build_report("Same Name", &[(5, 1, "13335", "美国", "Cloudflare")]);
        first.probe.node.host = "one.example.com".to_string();
        let mut second = build_report("Same Name", &[(5, 2, "46997", "美国", "NATOLAB")]);
        second.probe.node.host = "two.example.com".to_string();

        let reports = vec![first, second];

        let topology = build_topology_view(&reports);
        let leaf_nodes = topology
            .nodes
            .iter()
            .filter(|node| node.kind == "leaf" && node.label == "Same Name")
            .collect::<Vec<_>>();

        assert_eq!(leaf_nodes.len(), 2);
        assert_ne!(leaf_nodes[0].id, leaf_nodes[1].id);
    }
}
