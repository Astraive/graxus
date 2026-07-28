# Agent Context

`graxus-agent-api` is the boundary that turns indexed documentation and code
into agent-friendly context. It remains orchestration-focused:

- gathers relevant symbols, imports, calls, routes, type relationships, and DI bindings from the codemap
- gathers relevant documents from the docgraph
- merges diff-aware and query-aware context
- exports compact, deterministic payloads for downstream agents

`AgentContext` carries normalized `routes`, `type_impls`, and `di_bindings`
alongside symbols, imports, and calls. Text, file, symbol, and topic queries
match these facts directly; parser-native Ripex payloads remain in the
codemap’s `parser_results` and are not duplicated into agent context.

Bounded exports reserve deterministic space for semantic facts, sort them by
stable keys, and report their counts. This keeps the indexing crates reusable
while giving CLI and server consumers a consistent semantic context surface.
