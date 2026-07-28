# Agent Context

`graxus-agent-api` is the boundary that turns raw indexing data into agent-friendly context. It should stay orchestration-focused:

- gather relevant code facts from the codemap
- gather relevant docs from the docgraph
- merge diff-aware and query-aware context
- export compact payloads for downstream agents

This keeps indexing crates reusable while giving the CLI and servers a consistent context surface.
