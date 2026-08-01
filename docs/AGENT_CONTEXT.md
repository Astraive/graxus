# Agent Context

`graxus-agent-api` is the boundary that turns indexed documentation and code into agent-friendly context. It remains orchestration-focused:

- gathers relevant symbols, imports, calls, routes, type relationships, and DI bindings from the codemap
- gathers relevant documents from the docgraph
- merges diff-aware and query-aware context
- exports compact, deterministic payloads for downstream agents

`AgentContext` carries normalized `routes`, `type_impls`, and `di_bindings` alongside symbols, imports, and calls. Text, file, symbol, and topic queries match these facts directly; parser-native Ripex payloads remain in the codemap's `parser_results` and are not duplicated into agent context.

## Bounded context queries

`ContextBudget` estimates tokens from Unicode character count and refuses additions that would exceed `max_tokens`. `query_bounded` scores matching docs, symbols, files, bridge edges, routes, type relationships, and DI bindings deterministically, then retains only whole items that fit. A zero remaining budget emits a truncation warning instead of partial data.

The CLI applies additional caps for `--max-files`, `--max-symbols`, `--max-notes`, `--depth`, `--min-confidence`, and a shared semantic/graph edge budget. Direct query matches remain while depth zero removes traversed imports, calls, and bridge edges.

## Bounded agent exports

`AgentExport::export_bounded(max_tokens)` reserves independent portions of the content budget for structural and semantic collections. The semantic allocations are routes (15%), type relationships (10%), and DI bindings (10%); the remaining allocations cover symbols, imports, calls, files, code/bridge/doc edges, documents, and parser provenance. Items are sorted by stable keys and are never split across the budget boundary.

Export stats report emitted counts for routes, type relationships, and DI bindings, while parser backend metadata and diagnostics remain available for retained parser results. This gives CLI, server, and LSP consumers a bounded and reproducible semantic context surface.
