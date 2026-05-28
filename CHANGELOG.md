# Changelog

## v0.3.0 (planned)

### Theme: CLI Standardization & Config UX

- Numeric confidence system (0-100) replacing string labels
- Confidence label enum: Exact/High/Medium/Low/Weak/Unresolved
- Resolution method tracking on all facts (local_definition, named_import_exact_export, etc.)
- Central shared CLI argument structs (GlobalArgs, FileFilterArgs, TraversalArgs, ContextArgs, etc.)
- Persistent config UX: `graxus config update`, `graxus config set`, `graxus config show --source`
- Init-time config: `graxus init --max-depth 3 --k 25 --max-notes 50`
- Config-backed defaults for all commands (CLI flag > env var > graxus.yaml > built-in default)
- New commands: `graxus rollback`, `graxus regex`, `graxus replace-regex`
- JSON schema update with confidence objects on all fact types
- Environment variable overrides (GRAXUS_CONTEXT_BUDGET, GRAXUS_SEARCH_K, etc.)
- Full command arg standardization (--depth, --max-results, --min-confidence, --budget, --dry-run)

See `.nstack/plans/v0.3-master-plan.md` for full plan.

---

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
