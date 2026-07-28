---
title: Graxus
type: project
status: active
tags:
  - graxus
  - codebase-engine
  - ai-agent
---

# Graxus

AI-native codebase knowledge engine for AI agents.

## What is Graxus?

Graxus builds a documentation graph for Markdown files and a codemap for source code, then exposes fast search, safe editing, and repo intelligence through a CLI/API.

## Features

- **Docs Graph** — Obsidian-compatible Markdown knowledge graph
- **Code Codemap** — Ripex-first source analysis with per-file tree-sitter fallback
- **Search** — Literal, regex, and symbol search
- **Safe Edit** — Preview/replace with snapshot rollback
- **Agent API** — Structured context for AI agents

## Quick Start

```bash
graxus init       # Initialize project
graxus index      # Scan and build indexes
graxus status     # Show project info
graxus find "auth" # Search codebase
```

## Language Support

- TypeScript / JavaScript
- Rust
- Go
- Python

## Architecture

See [[ARCHITECTURE]] for detailed design.

## CLI

See [[CLI]] for all commands.

## Safety

See [[SAFETY]] for the safety model.
