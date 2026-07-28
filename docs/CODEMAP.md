# Codemap

`graxus-codemap` owns language-aware source analysis. Ripex is the primary parser and fact extractor for JavaScript/TypeScript, Python, Go, Rust, C, C++, and C#. Tree-sitter remains the per-file fallback when Ripex is unavailable, does not support a language, or fails to extract a file.

The normalized graph contains symbols, imports, calls, variables, and the newer framework-native facts that the CLI can surface as routes, type relationships, and DI bindings. For Ripex files, `parser_results` also preserves:

- the requested and actual backend
- parse diagnostics
- the complete parser-native symbol, import, call, and variable payloads
- a stable link from each parser-native payload to its normalized Graxus fact id

This lets CLI, server/LSP, and agent-export consumers use normalized cross-file resolution while retaining Ripex-specific details such as async/constructor flags, base classes, storage and type metadata, import specifiers, and awaited/optional call data.

Current priority languages are the "big 5" set for repository intelligence:

- Rust
- Python
- Go
- TypeScript / JavaScript
- C / C++ / C#

Parser extraction is separated from cross-file resolution. Select `--codemap-backend tree-sitter` to force the fallback implementation; `ripex` is the default.

## Semantic pipeline

For each scanned source file, Graxus records the requested and actual parser
backend, parser diagnostics, and lossless Ripex facts. It then performs
cross-file resolution over the normalized graph. Framework extraction is a
separate post-parser stage: a file is only attributed to a framework when its
registration syntax, decorator, import/package evidence, or file convention
identifies that framework. This avoids treating shared method names or
decorators as routes for every framework.

The complete `CodeGraph` additionally contains:

| Fact | Key fields | Resolution/persistence |
|------|------------|------------------------|
| Route | method, path, handler, handler file, framework, middleware | Handler is linked to a same-file or cross-file symbol when resolvable; stored in JSON and SQLite |
| Type implementation | implementing type, trait/interface, relationship kind | Explicit trait, implementation, and inheritance relationships; stored in JSON and SQLite |
| DI binding | abstract type, concrete type, lifetime, framework | Semantic contract-to-implementation registration; stored in JSON and SQLite |

IDs for these normalized facts are assigned deterministically by the
`CodeMapBuilder`, after resolution and deduplication. Parser-native Ripex fact
identifiers remain linked through `parser_results` rather than being replaced.

## Framework coverage

Route extraction currently recognizes:

- **Python:** FastAPI, Flask, Django
- **Rust:** Axum, Actix Web, Rocket
- **Go:** Gin, Fiber, Echo
- **JavaScript / TypeScript:** Express, NestJS, Next.js app-router
- **C#:** ASP.NET Core minimal APIs and controller attributes
- **C++:** Crow, Pistache, Drogon

DI extraction recognizes ASP.NET `AddSingleton` / `AddScoped` /
`AddTransient` registrations and NestJS injectable/provider registrations.
Type relationship extraction covers explicit Rust trait implementations plus
TypeScript, C#, Java, and Kotlin inheritance or implementation declarations.

## Consumer interfaces

`graxus index` writes the complete graph to `.graxus/code/codemap.json` and
persists route, type, and DI records to `.graxus/index.db`. The CLI exposes
routes through `graxus routes` (with `--framework`, `--lang`, and `--json`) and
relationships plus DI bindings through `graxus types` (with `--name` and
`--json`). The agent API selects these normalized semantic facts directly for
file, symbol, topic, text, and bounded export context.
