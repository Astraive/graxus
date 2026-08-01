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

Graxus builds a documentation graph for Markdown files and a codemap for source code, then exposes search, safe editing, semantic relationships, and bounded agent context through a CLI/API.

## Features

- **Docs Graph** — Obsidian-compatible Markdown knowledge graph
- **Code Codemap** — Ripex v0.3.0-first source analysis with a per-file tree-sitter fallback
- **Semantic facts** — HTTP routes, type relationships, and dependency-injection bindings
- **Search and edit** — Literal, regex, symbol search, previewed replacements, and snapshot rollback
- **Agent API** — Query-aware context and deterministic, token-bounded exports

## Quick Start

```bash
graxus init
graxus index --codemap-backend ripex  # ripex is the default
graxus status
graxus find "auth"
graxus update --dry-run               # inspect an incremental update
```

Use `graxus update` to re-index added or modified files, remove deleted-file data, refresh cross-file relationships, and replace the touched rows in SQLite. Use `graxus index` for a full rebuild.

## Language Support

Ripex is the primary parser dependency (`ripex = 0.3.0`) for:

- JavaScript and TypeScript
- Python
- Go
- Rust
- C and C++
- C#

Graxus records the requested and actual backend per file. If Ripex does not support a language or cannot extract a file, Graxus falls back to tree-sitter for that file; Java, Kotlin, and Swift therefore remain available through the fallback path.

## Framework and semantic coverage

Implemented route extraction covers FastAPI, Flask, and Django (Python); Axum, Actix Web, and Rocket (Rust); Gin, Fiber, and Echo (Go); Express, NestJS, and Next.js app-router (JavaScript/TypeScript); ASP.NET Core minimal APIs and controller attributes (C#); and Crow, Pistache, and Drogon (C++).

Express and Next.js route facts preserve whether the source file is JavaScript or TypeScript. NestJS dependency-injection facts preserve JavaScript or TypeScript language and include `@Injectable` and `useClass` providers. Type relationships include C# class, record, struct, and interface base/interface lists.

## Agent limits

Context queries honor a token budget and CLI caps for files, symbols, notes, depth, confidence, and edges. `graxus agent-export --budget N` uses deterministic category budgets (including routes, type relationships, DI bindings, parser provenance, and documentation) and never splits an individual fact.

## CLI

See [the CLI guide](docs/CLI.md) for commands and options.

Useful semantic commands:

```bash
graxus routes --json
graxus routes --framework express --lang javascript
graxus types --json
graxus context --query "auth" --budget 12000
graxus agent-export --budget 12000
```

## Architecture and safety

- [Architecture](docs/ARCHITECTURE.md)
- [Codemap and parser pipeline](docs/CODEMAP.md)
- [Route facts](docs/ROUTE_FACTS.md)
- [Type and DI resolution](docs/TYPE_AND_DI_RESOLUTION.md)
- [Agent context](docs/AGENT_CONTEXT.md)
- [Safety model](docs/SAFETY.md)

## Ripex dependency

Graxus consumes the published [Ripex v0.3.0 release](https://github.com/astraive/ripex/releases/tag/v0.3.0) as its primary parser. See the [Ripex repository](https://github.com/astraive/ripex) for the parser's source and release history.
