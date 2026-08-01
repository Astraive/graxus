# Changelog

## Current workspace (2026-08-01; package version 0.4.0)

This section records the current Graxus workspace after the Ripex migration and semantic hardening; it is not a Graxus release tag.

### Ripex 0.3.0 migration

- Graxus now consumes the published [Ripex v0.3.0 release](https://github.com/astraive/ripex/releases/tag/v0.3.0) as its primary parser dependency.
- Ripex-backed extraction covers JavaScript/TypeScript, Python, Go, Rust, C, C++, and C#. Tree-sitter remains an explicit per-file fallback for unsupported languages and files where Ripex extraction fails.
- Every parser result records the requested and actual backend, fallback reason, diagnostics, and lossless parser-native facts linked to normalized Graxus facts.
- Both repository CI pipelines passed after the migration.

### Semantic and incremental hardening

- `graxus update` re-indexes added or modified files, removes deleted-file facts, and refreshes imports, calls, route handlers, and relationship edges against the complete retained graph.
- Incremental SQLite updates delete all code, semantic, and parser rows for each touched file before re-inserting fresh rows, preventing stale or duplicate records. A full `graxus index` clears code tables before rebuilding them.
- Implemented framework extraction covers FastAPI, Flask, and Django; Axum, Actix Web, and Rocket; Gin, Fiber, and Echo; Express, NestJS, and Next.js app-router; ASP.NET Core; and Crow, Pistache, and Drogon.
- Express and Next.js route facts preserve JavaScript versus TypeScript source language. NestJS DI facts preserve JavaScript or TypeScript and include injectable classes and `useClass` providers with normalized scopes.
- Type relationship extraction includes C# class, record, struct, and interface base/interface lists, alongside Rust, TypeScript, Java, and Kotlin relationships.

### CLI, context, and export

- Shared CLI argument handling standardizes `--depth`, `--max-results`, `--min-confidence`, `--budget`, and `--dry-run`; persistent config supports `graxus config update`, `graxus config show`, and provider key management.
- `graxus routes` and `graxus types` expose normalized semantic facts; `graxus context` and `graxus agent-export` expose them with token, file, symbol, note, depth, confidence, and edge limits.
- Bounded context and agent exports use deterministic ordering and category budgets for structural facts, routes, type relationships, DI bindings, parser provenance, and documentation without splitting individual facts.

See the [Graxus CLI guide](docs/CLI.md), [codemap pipeline](docs/CODEMAP.md), and [Ripex repository](https://github.com/astraive/ripex) for current interfaces and dependency history.

## v0.2.0 (2026-05-28)

### New Crates
- **graxus-embed** — Vector embedding engine with OpenAI, Cohere, and Ollama providers. JSON vector store with cosine similarity search. Batch embedding pipeline with deduplication.
- **graxus-llm** — LLM documentation generation with OpenAI, Anthropic, and Ollama providers. Prompt templates for 6 doc types. Cost tracking, rate limiting.
- **graxus-server** — JSON-RPC server over stdio with 6 RPC methods (ping, status, context, file_context, symbol_context, update).

### New Commands
- `graxus doctor` — Health diagnostics (index freshness, parse errors, resolution coverage, stale docs)
- `graxus impact FILE` — Blast radius analysis (transitive callers/importers)
- `graxus hotspots` — Most-called symbols ranked by usage count
- `graxus dead-code` — Uncalled symbols (potentially unused code)
- `graxus history` — Edit snapshot timeline
- `graxus watch` — Filesystem event auto-reindex (notify crate)
- `graxus workspaces` — Workspace boundary detection (Cargo, npm, Go)
- `graxus plugins` — Plugin system (list, install, uninstall)
- `graxus config set-key` — API key management via env vars
- `graxus config show` — Display config with keys redacted
- `graxus embed` — Generate vector embeddings for semantic search
- `graxus search QUERY` — Semantic search using embeddings
- `graxus generate docs` — LLM-generated documentation
- `graxus generate architecture` — Generate ARCHITECTURE.md
- `graxus serve` — JSON-RPC server on stdio
- `graxus deps` — List detected project dependencies

### Features
- SQLite storage backend (rusqlite) alongside JSON
- Symbol signature extraction via tree-sitter (params + return types)
- Test function detection (Rust `#[test]`, TS `it()`, Go `Test*`, Python `test_*`)
- Usage frequency counting on symbols
- Call graph traversal: callers, callees, transitive callers/callees, blast radius, cycle detection
- Import resolution improvements: Rust `crate::`/`super::`/`self::`, TS relative, Python relative, Go modules
- Context window budget management (token estimation, priority scoring, bounded queries)
- Differential context (git diff → affected symbols → context)
- Bounded agent export (truncate to fit token budget)
- Plugin system with `GraxusPlugin` trait
- Monorepo awareness (workspace detection)
- Cross-repo dependency detection (Cargo.toml, package.json, go.mod, requirements.txt)
- Snapshot diff history
- Bridge stale detection (DocMayBeStale edges)
- HashMap indexes on CodeGraph for O(1) lookups
- Config extensions: `EmbeddingsConfig`, `LlmConfig` with API key resolution

### Tests
- 87 tests across 5 crates (graxus-core: 14, graxus-codemap: 23, graxus-agent-api: 25, graxus-embed: 10, graxus-llm: 14)

### Architecture
- 10 crates, 86 source files, 10,581 lines of Rust
- 24 CLI commands

---

## v0.1.0 (2026-05-28)

### Initial Release
- **graxus-core** — Config loading, file type detection, workspace management, repo scanner
- **graxus-docgraph** — Obsidian-compatible markdown parsing (frontmatter, wiki links, tags, headings, backlinks)
- **graxus-codemap** — tree-sitter code parsing for TypeScript, Rust, Go, Python. Extracts imports, symbols, calls.
- **graxus-index** — JSON file storage with snapshot management
- **graxus-edit** — Find/replace engine with preview, apply, safety checks
- **graxus-agent-api** — Docs-code bridge, context queries, agent export
- **graxus-cli** — CLI with 10 commands (init, status, index, graph, codemap, find, replace, context, agent-export)

### Architecture
- 7 crates, 43 source files, 4,446 lines of Rust
- 10 CLI commands
