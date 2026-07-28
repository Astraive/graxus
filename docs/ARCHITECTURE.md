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

[[Graxus]] is an AI-native codebase knowledge engine built in Rust. It maintains two separate knowledge layers:

1. **Docs Graph** — Obsidian-compatible Markdown knowledge graph for intent, decisions, plans, and architecture
2. **Code Codemap** — Source-code structure, parser-native facts, and relationships powered primarily by Ripex with tree-sitter fallback

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
    graxus-cli/        — Indexing, semantic queries, diagnostics, and workflows
```

## Data Flow

1. `graxus index` scans the repository.
2. [[Docs Graph]] parses Markdown files into the documentation graph.
3. [[Code Codemap]] parses each source file with Ripex first, then uses
   tree-sitter only as a per-file fallback.
4. The codemap normalizes symbols, imports, calls, variables, HTTP routes, type
   relationships, and DI bindings; it resolves cross-file links and persists
   JSON plus SQLite records.
5. [[Bridge Layer]] connects docs to code. The Agent API and CLI expose
   query-specific semantic context.

## Knowledge Layers

### Docs Graph

See [[DOCS_GRAPH]] for details.

- Obsidian-compatible wiki links `[[Note]]`
- YAML frontmatter parsing
- Tag indexing `#tag`
- Backlink generation

### Code Codemap

See [[CODEMAP]] for details.

- Ripex-first parsing with explicit per-file tree-sitter fallback and retained backend diagnostics
- Lossless parser-native facts linked to normalized symbols, imports, calls, and variables
- Framework-native HTTP routes with registration, handler, framework, and middleware metadata
- Type implementation/inheritance facts and framework DI contract-to-concrete bindings
- Cross-file import, call, and route-handler resolution with explicit confidence

## Related Notes

* [[DOCS_GRAPH]]
* [[CODEMAP]]
* [[SAFETY]]
* [[TESTING]]
