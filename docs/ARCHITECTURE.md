---
title: Graxus Architecture
type: docs
status: active
tags:
  - graxus
  - architecture
---

# Graxus Architecture

## Overview

Graxus is an AI-native codebase knowledge engine built in Rust. It maintains two separate knowledge layers:

1. **Docs Graph** — Obsidian-compatible Markdown knowledge graph for intent, decisions, plans, and architecture
2. **Code Codemap** — Source-code structure, parser-native facts, and relationships powered primarily by the published [Ripex v0.3.0 parser](https://github.com/astraive/ripex/releases/tag/v0.3.0), with tree-sitter fallback

## Crate Structure

```
graxus/
  crates/
    graxus-core/       — Config, scanner, workspace, file types
    graxus-docgraph/   — Markdown parsing, wiki links, backlinks
    graxus-codemap/    — Ripex/tree-sitter parsing, normalized code and framework facts
    graxus-index/      — JSON snapshots and SQLite-backed fact storage
    graxus-edit/       — Find/replace engine with safety
    graxus-agent-api/  — Query-aware, token-bounded agent context and export
    cli/               — Indexing, semantic queries, diagnostics, and workflows
```

## Data Flow

1. `graxus index` scans the repository.
2. [Docs Graph](DOCGRAPH.md) parses Markdown files into the documentation graph.
3. [Code Codemap](CODEMAP.md) parses each supported source file with Ripex first, then uses tree-sitter only as a per-file fallback.
4. The codemap normalizes symbols, imports, calls, variables, HTTP routes, type relationships, and DI bindings; it resolves cross-file links and persists JSON plus SQLite records.
5. The [bridge and agent context layer](AGENT_CONTEXT.md) connects docs to code and exposes query-specific semantic context.

`graxus update` removes changed or deleted file data, parses only added/modified files, merges the result, and refreshes cross-file imports, calls, route handlers, and relationship edges against all retained files. Its SQLite path deletes rows for each touched file before re-inserting fresh rows, so removed facts do not remain stale.

## Knowledge Layers

### Docs Graph

See [DOCGRAPH.md](DOCGRAPH.md) for details.

- Obsidian-compatible wiki links `[[Note]]`
- YAML frontmatter parsing
- Tag indexing `#tag`
- Backlink generation

### Code Codemap

See [CODEMAP.md](CODEMAP.md) for details.

- Ripex-first parsing with explicit per-file tree-sitter fallback and retained backend diagnostics
- Lossless parser-native facts linked to normalized symbols, imports, calls, and variables
- Framework-native HTTP routes with registration, handler, framework, and middleware metadata
- Type implementation/inheritance facts and framework DI contract-to-concrete bindings
- Cross-file import, call, and route-handler resolution with explicit confidence

## Storage and limits

`graxus index` writes `.graxus/code/codemap.json` and rebuilds code facts in `.graxus/index.db`. `graxus update` preserves unchanged rows while replacing rows for touched files. Context queries and agent exports apply deterministic token and collection limits; see [AGENT_CONTEXT.md](AGENT_CONTEXT.md).

## Related Notes

* [CLI](CLI.md)
* [Safety](SAFETY.md)
* [Route facts](ROUTE_FACTS.md)
* [Type and DI resolution](TYPE_AND_DI_RESOLUTION.md)
