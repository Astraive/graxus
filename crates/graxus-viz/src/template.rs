//! HTML template for D3.js force-directed graph visualization.
//!
//! Renders a [`D3Graph`] as a standalone HTML file with an interactive
//! force-directed layout powered by D3.js v7. The template includes
//! search, zoom, drag, and a detail panel.

use crate::D3Graph;

/// Escape the five HTML-sensitive characters for safe interpolation into HTML.
///
/// Converts `&` to `&amp;`, `<` to `&lt;`, `>` to `&gt;`, `"` to `&quot;`,
/// and `'` to `&#x27;`.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Generate a standalone HTML file with an interactive D3.js graph.
///
/// The graph is sanitized before rendering to enforce size limits and strip
/// HTML from node IDs and edge types. All user-provided content (title,
/// description, node labels, file paths, details) is escaped for safe HTML
/// rendering — both server-side (Rust) for the header and client-side
/// (JavaScript) for the detail panel.
pub fn render_html(graph: &D3Graph) -> String {
    // Sanitize a clone so the caller's graph is not mutated
    let mut graph = graph.clone();
    graph.sanitize();

    let data_json = serde_json::to_string(&graph).unwrap_or_default();

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Graxus</title>
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0d1117; color: #c9d1d9; }}
  #header {{ background: #161b22; padding: 12px 20px; border-bottom: 1px solid #30363d; display: flex; align-items: center; gap: 16px; }}
  #header h1 {{ font-size: 18px; color: #58a6ff; }}
  #header span {{ color: #8b949e; font-size: 14px; }}
  #search {{ background: #0d1117; border: 1px solid #30363d; color: #c9d1d9; padding: 6px 12px; border-radius: 6px; width: 240px; font-size: 13px; }}
  #search:focus {{ outline: none; border-color: #58a6ff; }}
  #stats {{ margin-left: auto; font-size: 12px; color: #8b949e; }}
  #graph {{ width: 100%; height: calc(100vh - 52px); }}
  #detail {{ position: fixed; right: 0; top: 52px; width: 320px; height: calc(100vh - 52px); background: #161b22; border-left: 1px solid #30363d; padding: 16px; overflow-y: auto; display: none; }}
  #detail h2 {{ font-size: 16px; color: #58a6ff; margin-bottom: 8px; }}
  #detail .type {{ display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 11px; margin-bottom: 8px; }}
  #detail .label {{ font-size: 12px; color: #8b949e; margin-bottom: 4px; }}
  #detail .value {{ font-size: 14px; color: #c9d1d9; margin-bottom: 12px; }}
  #legend {{ position: fixed; left: 16px; bottom: 16px; background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 12px; font-size: 12px; }}
  #legend div {{ display: flex; align-items: center; gap: 8px; margin-bottom: 4px; }}
  #legend .dot {{ width: 12px; height: 12px; border-radius: 50%; }}
</style>
</head>
<body>
<div id="header">
  <h1>{title}</h1>
  <span>{description}</span>
  <input id="search" type="text" placeholder="Search nodes...">
  <span id="stats"></span>
</div>
<div id="graph"></div>
<div id="detail"></div>
<div id="legend">
  <div><span class="dot" style="background:#58a6ff"></span> Function / Method</div>
  <div><span class="dot" style="background:#3fb950"></span> Class / Struct</div>
  <div><span class="dot" style="background:#d2a8ff"></span> Interface / Trait</div>
  <div><span class="dot" style="background:#f0883e"></span> Module / File</div>
  <div><span class="dot" style="background:#f97583"></span> Doc / Note</div>
  <div><span class="dot" style="background:#79c0ff"></span> Constant / Type</div>
  <div><span class="dot" style="background:#ffa657"></span> Enum</div>
  <div><span class="dot" style="background:#e6edf3"></span> Other</div>
</div>

<script src="https://d3js.org/d3.v7.min.js"></script>
<script>
const data = {data_json};

function escapeHtml(s) {{
  if (!s) return '';
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#x27;');
}}

function nodeColor(type) {{
  const colors = {{
    'function': '#58a6ff', 'method': '#58a6ff',
    'class': '#3fb950', 'struct': '#3fb950',
    'interface': '#d2a8ff', 'trait': '#d2a8ff',
    'module': '#f0883e', 'file': '#f0883e',
    'doc': '#f97583', 'tag': '#f97583',
    'constant': '#79c0ff', 'type': '#79c0ff',
    'enum': '#ffa657',
    'caller': '#58a6ff', 'target': '#f97583',
    'symbol': '#58a6ff', 'import': '#79c0ff',
  }};
  return colors[type] || '#e6edf3';
}}

const width = window.innerWidth;
const height = window.innerHeight - 52;

const svg = d3.select("#graph").append("svg")
  .attr("width", width)
  .attr("height", height);

const g = svg.append("g");

// Zoom
svg.call(d3.zoom().scaleExtent([0.1, 10]).on("zoom", (e) => g.attr("transform", e.transform)));

document.getElementById("stats").textContent = `${{data.nodes.length}} nodes, ${{data.links.length}} links`;

const simulation = d3.forceSimulation(data.nodes)
  .force("link", d3.forceLink(data.links).id(d => d.id).distance(80))
  .force("charge", d3.forceManyBody().strength(-200))
  .force("center", d3.forceCenter(width / 2, height / 2))
  .force("collision", d3.forceCollide(30));

const link = g.append("g")
  .selectAll("line")
  .data(data.links)
  .join("line")
  .attr("stroke", "#30363d")
  .attr("stroke-width", 1.5)
  .attr("stroke-opacity", 0.6);

const node = g.append("g")
  .selectAll("circle")
  .data(data.nodes)
  .join("circle")
  .attr("r", d => d.node_type === 'file' ? 10 : 7)
  .attr("fill", d => nodeColor(d.node_type))
  .attr("stroke", "#0d1117")
  .attr("stroke-width", 1.5)
  .style("cursor", "pointer")
  .call(d3.drag()
    .on("start", (e, d) => {{ if (!e.active) simulation.alphaTarget(0.3).restart(); d.fx = d.x; d.fy = d.y; }})
    .on("drag", (e, d) => {{ d.fx = e.x; d.fy = e.y; }})
    .on("end", (e, d) => {{ if (!e.active) simulation.alphaTarget(0); d.fx = null; d.fy = null; }})
  )
  .on("click", (e, d) => showDetail(d));

const label = g.append("g")
  .selectAll("text")
  .data(data.nodes)
  .join("text")
  .text(d => d.label)
  .attr("font-size", d => d.node_type === 'file' ? 11 : 10)
  .attr("fill", "#c9d1d9")
  .attr("dx", 12)
  .attr("dy", 4)
  .style("pointer-events", "none");

simulation.on("tick", () => {{
  link.attr("x1", d => d.source.x).attr("y1", d => d.source.y)
      .attr("x2", d => d.target.x).attr("y2", d => d.target.y);
  node.attr("cx", d => d.x).attr("cy", d => d.y);
  label.attr("x", d => d.x).attr("y", d => d.y);
}});

function showDetail(d) {{
  const detail = document.getElementById("detail");
  detail.style.display = "block";
  const typeColor = nodeColor(d.node_type);
  detail.innerHTML = `
    <h2>${{escapeHtml(d.label)}}</h2>
    <span class="type" style="background:${{typeColor}}22;color:${{typeColor}}">${{escapeHtml(d.node_type)}}</span>
    ${{d.file ? `<div class="label">File</div><div class="value">${{escapeHtml(d.file)}}</div>` : ''}}
    ${{d.line ? `<div class="label">Line</div><div class="value">${{escapeHtml(String(d.line))}}</div>` : ''}}
    ${{d.details ? `<div class="label">Details</div><div class="value">${{escapeHtml(d.details)}}</div>` : ''}}
    <div class="label">Connections</div>
    <div class="value">${{data.links.filter(l => l.source.id === d.id || l.target.id === d.id).length}} edges</div>
  `;
}}

// Search
document.getElementById("search").addEventListener("input", (e) => {{
  const q = e.target.value.toLowerCase();
  node.attr("opacity", d => q === '' || d.label.toLowerCase().includes(q) || (d.file && d.file.toLowerCase().includes(q)) ? 1 : 0.1);
  label.attr("opacity", d => q === '' || d.label.toLowerCase().includes(q) || (d.file && d.file.toLowerCase().includes(q)) ? 1 : 0.1);
  link.attr("stroke-opacity", q === '' ? 0.6 : 0.1);
}});
</script>
</body>
</html>"##,
        title = html_escape(&graph.title),
        description = html_escape(&graph.description),
        data_json = data_json,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{D3Graph, D3Link, D3Node};

    fn sample_node(id: &str, label: &str, node_type: &str) -> D3Node {
        D3Node {
            id: id.into(),
            label: label.into(),
            node_type: node_type.into(),
            file: None,
            line: None,
            details: None,
        }
    }

    fn sample_link(source: &str, target: &str, edge_type: &str) -> D3Link {
        D3Link {
            source: source.into(),
            target: target.into(),
            edge_type: edge_type.into(),
            label: None,
        }
    }

    // ── html_escape ─────────────────────────────────────────────────

    #[test]
    fn test_html_escape_script_tag() {
        assert_eq!(
            html_escape("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
        );
    }

    #[test]
    fn test_html_escape_all_special_chars() {
        assert_eq!(
            html_escape(r#"a & b <c> d "e" f'g"#),
            "a &amp; b &lt;c&gt; d &quot;e&quot; f&#x27;g"
        );
    }

    #[test]
    fn test_html_escape_empty() {
        assert_eq!(html_escape(""), "");
    }

    #[test]
    fn test_html_escape_no_special() {
        assert_eq!(html_escape("goodbye world"), "goodbye world");
    }

    // ── render_html basic ───────────────────────────────────────────

    #[test]
    fn test_render_html_basic() {
        let graph = D3Graph::new("Test", "A test graph");
        let html = render_html(&graph);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Test"));
        assert!(html.contains("d3.v7.min.js"));
        assert!(html.contains("Graxus"));
    }

    #[test]
    fn test_render_html_empty_graph() {
        let graph = D3Graph::new("Empty", "No nodes or links");
        let html = render_html(&graph);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Empty"));
        assert!(html.contains(r#""nodes":[]"#));
        assert!(html.contains(r#""links":[]"#));
    }

    #[test]
    fn test_render_html_with_data() {
        let mut graph = D3Graph::new("Test", "desc");
        graph.nodes.push(D3Node {
            id: "n1".into(),
            label: "Node 1".into(),
            node_type: "function".into(),
            file: None,
            line: None,
            details: None,
        });
        graph.links.push(D3Link {
            source: "n1".into(),
            target: "n1".into(),
            edge_type: "test".into(),
            label: None,
        });
        let html = render_html(&graph);
        assert!(html.contains("Node 1"));
        assert!(html.contains("data.nodes.length"));
        assert!(html.contains("data.links.length"));
    }

    // ── Unicode content ─────────────────────────────────────────────

    #[test]
    fn test_render_html_unicode_title() {
        let graph = D3Graph::new("Code Analysis — 日本語分析", "Ünïcödé dëscrîptîön 🚀");
        let html = render_html(&graph);
        assert!(html.contains("Code Analysis"));
        assert!(html.contains("日本語分析"));
        assert!(html.contains("Ünïcödé"));
        assert!(html.contains("🚀"));
    }

    #[test]
    fn test_render_html_unicode_node_labels() {
        let mut graph = D3Graph::new("T", "D");
        graph
            .nodes
            .push(sample_node("n1", "données_françaises", "function"));
        graph.nodes.push(sample_node("n2", "中文符号", "struct"));
        graph.nodes.push(sample_node("n3", "العربية", "doc"));
        let html = render_html(&graph);
        assert!(html.contains("données_françaises"));
        assert!(html.contains("中文符号"));
        assert!(html.contains("العربية"));
    }

    // ── XSS prevention in template ──────────────────────────────────

    #[test]
    fn test_xss_prevention_title() {
        let graph = D3Graph::new("<script>alert('xss')</script>", "safe");
        let html = render_html(&graph);
        // The title in the header should be escaped
        assert!(html.contains("&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"));
        // Should NOT contain raw script tag in the header HTML (outside the JSON data)
        // Split at the script tag where JSON data begins
        let header_section = html.split("const data").next().unwrap();
        assert!(!header_section.contains("<script>alert"));
    }

    #[test]
    fn test_xss_prevention_description() {
        let graph = D3Graph::new("T", r#"<img src=x onerror="alert(1)">"#);
        let html = render_html(&graph);
        let header_section = html.split("const data").next().unwrap();
        // The < and > must be escaped so the browser won't parse it as an HTML tag
        assert!(header_section.contains("&lt;img"));
        assert!(!header_section.contains("<img "));
    }

    #[test]
    fn test_xss_prevention_node_label_in_detail_panel() {
        let mut graph = D3Graph::new("T", "D");
        graph.nodes.push(D3Node {
            id: "n1".into(),
            label: r#"<img src=x onerror="alert(1)">"#.into(),
            node_type: "function".into(),
            file: None,
            line: None,
            details: None,
        });
        let html = render_html(&graph);
        // The showDetail function should use escapeHtml for labels
        assert!(html.contains("escapeHtml(d.label)"));
        assert!(html.contains("escapeHtml(d.node_type)"));
    }

    #[test]
    fn test_xss_prevention_file_path_in_detail_panel() {
        let mut graph = D3Graph::new("T", "D");
        graph.nodes.push(D3Node {
            id: "n1".into(),
            label: "Node".into(),
            node_type: "function".into(),
            file: Some(r#"C:\Users\<script>alert(1)</script>\file.rs"#.into()),
            line: None,
            details: None,
        });
        let html = render_html(&graph);
        // showDetail should escape file paths
        assert!(html.contains("escapeHtml(d.file)"));
    }

    #[test]
    fn test_xss_prevention_details_in_detail_panel() {
        let mut graph = D3Graph::new("T", "D");
        graph.nodes.push(D3Node {
            id: "n1".into(),
            label: "Node".into(),
            node_type: "function".into(),
            file: None,
            line: None,
            details: Some(r#"Details with <b>HTML</b> and 'quotes'"#.into()),
        });
        let html = render_html(&graph);
        // showDetail should escape details
        assert!(html.contains("escapeHtml(d.details)"));
    }

    #[test]
    fn test_xss_prevention_escapehtml_function_present() {
        let graph = D3Graph::new("T", "D");
        let html = render_html(&graph);
        // Verify the JavaScript escapeHtml function exists
        assert!(html.contains("function escapeHtml(s)"));
        assert!(html.contains(".replace(/&/g, '&amp;')"));
        assert!(html.contains(".replace(/</g, '&lt;')"));
    }

    #[test]
    fn test_xss_prevention_node_type_escaped() {
        let graph = D3Graph::new("T", "D");
        let html = render_html(&graph);
        // node_type in the detail panel should be escaped
        assert!(html.contains("escapeHtml(d.node_type)"));
    }

    // ── Large graph ─────────────────────────────────────────────────

    #[test]
    fn test_render_html_large_graph() {
        let mut graph = D3Graph::new("Large", "100+ nodes");
        for i in 0..150 {
            graph.nodes.push(sample_node(
                &format!("n{}", i),
                &format!("Node {}", i),
                "function",
            ));
        }
        for i in 0..149 {
            graph.links.push(sample_link(
                &format!("n{}", i),
                &format!("n{}", i + 1),
                "calls",
            ));
        }
        let html = render_html(&graph);
        // The node/link counts are rendered client-side via JS; verify the JSON data has them
        assert!(html.contains(r#""nodes":["#));
        assert!(html.contains(r#""links":["#));
        // Verify all 150 node IDs are present in the JSON
        assert!(html.contains(r#""id":"n0""#));
        assert!(html.contains(r#""id":"n149""#));
    }

    #[test]
    fn test_render_html_sanitizes_before_render() {
        let mut graph = D3Graph::new("T", "D");
        graph.nodes.push(D3Node {
            id: "<bad>".into(),
            label: "x".into(),
            node_type: "function".into(),
            file: None,
            line: None,
            details: None,
        });
        let html = render_html(&graph);
        // The sanitized ID should appear in the JSON data (no angle brackets)
        assert!(!html.contains(r#""id":"<bad>""#));
        assert!(html.contains(r#""id":"bad""#));
    }

    // ── Graph with all node types ───────────────────────────────────

    #[test]
    fn test_render_html_all_node_types() {
        // These types are explicitly mapped in the nodeColor JS function
        let mapped_types = [
            "function",
            "method",
            "class",
            "struct",
            "interface",
            "trait",
            "module",
            "file",
            "doc",
            "tag",
            "constant",
            "type",
            "enum",
            "caller",
            "target",
            "symbol",
            "import",
        ];
        let mut graph = D3Graph::new("Types", "All node types");
        for (i, t) in mapped_types.iter().enumerate() {
            graph.nodes.push(sample_node(&format!("n{}", i), t, t));
        }
        // "other" type falls through to the default color
        graph.nodes.push(sample_node("n_other", "misc", "other"));

        let html = render_html(&graph);
        // Every explicitly mapped type should appear as a key in the nodeColor map
        for t in &mapped_types {
            assert!(
                html.contains(&format!("'{}'", t)),
                "nodeColor missing type '{}'",
                t
            );
        }
        // "other" is handled by the default fallback in nodeColor
        assert!(html.contains("'other'") || html.contains("|| '#e6edf3'"));
    }
}