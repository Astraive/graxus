//! HTML template for D3.js force-directed graph visualization.

use crate::D3Graph;

/// Generate a standalone HTML file with an interactive D3.js graph.
pub fn render_html(graph: &D3Graph) -> String {
    let data_json = serde_json::to_string(graph).unwrap_or_default();

    format!(r##"<!DOCTYPE html>
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
    <h2>${{d.label}}</h2>
    <span class="type" style="background:${{typeColor}}22;color:${{typeColor}}">${{d.node_type}}</span>
    ${{d.file ? `<div class="label">File</div><div class="value">${{d.file}}</div>` : ''}}
    ${{d.line ? `<div class="label">Line</div><div class="value">${{d.line}}</div>` : ''}}
    ${{d.details ? `<div class="label">Details</div><div class="value">${{d.details}}</div>` : ''}}
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_render_html_with_data() {
        let mut graph = D3Graph::new("Test", "desc");
        graph.nodes.push(crate::D3Node {
            id: "n1".into(), label: "Node 1".into(), node_type: "function".into(),
            file: None, line: None, details: None,
        });
        graph.links.push(crate::D3Link {
            source: "n1".into(), target: "n1".into(), edge_type: "test".into(), label: None,
        });
        let html = render_html(&graph);
        assert!(html.contains("Node 1"));
        assert!(html.contains("1 nodes, 1 links"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>alert('xss')</script>"), "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;");
    }
}
