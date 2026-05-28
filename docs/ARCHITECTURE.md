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
2. **Code Codemap** — Source-code structure, symbols, imports, and relationships powered by tree-sitter

## Crate Structure

```
graxus/
  crates/
    graxus-core/       — Config, scanner, workspace, file types
    graxus-docgraph/   — Markdown parsing, wiki links, backlinks
    graxus-codemap/    — tree-sitter parsing, symbols, imports, calls
    graxus-index/      — JSON storage, snapshots
    graxus-edit/       — Find/replace engine with safety
    graxus-agent-api/  — Bridge layer, context queries
    graxus-cli/        — CLI interface
```

## Data Flow

1. `graxus index` scans the repository
2. [[Docs Graph]] parses Markdown files
3. [[Code Codemap]] parses source files with tree-sitter
4. [[Bridge Layer]] connects docs to code
5. Agent API exposes context for AI agents

## Knowledge Layers

### Docs Graph

See [[DOCS_GRAPH]] for details.

- Obsidian-compatible wiki links `[[Note]]`
- YAML frontmatter parsing
- Tag indexing `#tag`
- Backlink generation

### Code Codemap

See [[CODEMAP]] for details.

- tree-sitter-based parsing
- Language support: TypeScript, Rust, Go, Python
- Import resolution with confidence levels
- Call graph extraction

## Related Notes

* [[DOCS_GRAPH]]
* [[CODEMAP]]
* [[SAFETY]]
* [[TESTING]]
