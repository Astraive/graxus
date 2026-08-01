---
title: Graxus CLI
type: docs
status: active
tags:
  - graxus
  - cli
---

# Graxus CLI

All commands accept the global project/config options (`--root`, `--config`, `--json`, `--quiet`, `--verbose`, `--no-color`, and `--timeout`).

## Initialize and index

```bash
graxus init
graxus index                                  # Ripex-first; default backend
graxus index --codemap-backend tree-sitter    # Force fallback parser
graxus status
graxus update --dry-run
graxus update
graxus update --full
graxus diff --json
```

`graxus index` scans docs and code, builds `.graxus/docs/` and `.graxus/code/codemap.json`, and rebuilds code facts in `.graxus/index.db`. `graxus update` parses only added/modified files, removes deleted-file data, refreshes cross-file relationships, and replaces SQLite rows for touched files. Use `--codemap-backend ripex|tree-sitter|auto` on `index` or `update`.

## Docs graph

```bash
graxus graph docs
graxus graph docs --json
graxus graph docs --file F
graxus graph backlinks F
graxus graph tags
graxus graph export --format json --save
```

## Code codemap

```bash
graxus codemap show
graxus codemap show --json
graxus codemap symbols --file F --lang rust --limit 100
graxus codemap imports F --resolved
graxus codemap calls SYMBOL --depth 2
graxus codemap impacted F --depth 3
graxus codemap export --format json --save
graxus symbols --file F
```

The codemap reports Ripex/tree-sitter backend provenance, parser diagnostics, normalized facts, and framework/relationship counts.

## Semantic facts

```bash
graxus routes
graxus routes --framework express --lang javascript
graxus routes --json
graxus types
graxus types --name IUserService
graxus types --json
```

Routes include registration file, source language, method, path, handler, framework, optional resolved handler file, and middleware. Type queries include trait/interface/inheritance relationships and DI bindings.

## Search

```bash
graxus find "pattern"
graxus find "pattern" --code
graxus find "pattern" --docs
graxus find "symbol" --symbol
graxus regex "pattern" --code
graxus search "query" --mode hybrid
```

## Agent context and export

```bash
graxus context --query "auth" --budget 12000
graxus context --file src/auth/session.ts
graxus context --symbol validateSession
graxus agent-export --budget 12000 --json
```

Context queries honor `--budget`, `--max-files`, `--max-symbols`, `--max-notes`, `--depth`, and `--min-confidence`; semantic facts share the edge cap configured for context. Bounded exports use deterministic category budgets and retain whole facts.

## Safe edits and diagnostics

```bash
graxus replace "old" "new" --preview
graxus replace "old" "new" --apply
graxus replace "old" "new" --regex --apply
graxus history
graxus rollback SNAPSHOT_ID --preview
graxus rollback SNAPSHOT_ID --apply
graxus doctor
graxus impact FILE
graxus hotspots
graxus dead-code
graxus watch
graxus workspaces
graxus deps
```

Mutating operations require `--apply`; replacement snapshots can be inspected with `history` and restored with `rollback`.

## Configuration and integrations

```bash
graxus config update context.budget 12000
graxus config show
graxus config set-key openai "$OPENAI_API_KEY"
graxus plugins list
graxus embed
graxus serve
graxus lsp
graxus visualize all
graxus clean --force
```

## Related Notes

* [Architecture](ARCHITECTURE.md)
* [Codemap](CODEMAP.md)
* [Agent context](AGENT_CONTEXT.md)
* [Safety](SAFETY.md)
