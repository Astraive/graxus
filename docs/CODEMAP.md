# Codemap

`graxus-codemap` owns language-aware source analysis. Graxus consumes the published [Ripex v0.3.0 parser](https://github.com/astraive/ripex/releases/tag/v0.3.0) as its primary backend and fact extractor for JavaScript/TypeScript, Python, Go, Rust, C, C++, and C#. Tree-sitter remains the per-file fallback when Ripex is unavailable, does not support a language, or fails to extract a file.

For each file, parser provenance records the requested backend, actual backend, fallback reason, diagnostics, and lossless parser-native facts. Each retained parser-native symbol, import, call, and variable payload links to its normalized Graxus fact id.

## Language and framework coverage

The primary Ripex language set is:

- JavaScript / TypeScript
- Python
- Go
- Rust
- C / C++
- C#

Tree-sitter provides the explicit fallback path, including Java, Kotlin, Swift, and any Ripex-supported file that fails extraction. Select `--codemap-backend tree-sitter` to force tree-sitter; `ripex` is the default.

Framework-native extraction is implemented for:

- **Python:** FastAPI, Flask, Django
- **Rust:** Axum, Actix Web, Rocket
- **Go:** Gin, Fiber, Echo
- **JavaScript / TypeScript:** Express, NestJS, Next.js app-router
- **C#:** ASP.NET Core minimal APIs and controller attributes
- **C++:** Crow, Pistache, Drogon

Express and Next.js preserve JavaScript versus TypeScript route language from the source file. NestJS DI extraction preserves JavaScript or TypeScript for `@Injectable` and `useClass` providers. Type relationships include explicit Rust trait implementations, TypeScript/Java/Kotlin inheritance or implementation declarations, and C# class, record, struct, and interface base/interface lists.

## Semantic pipeline

Parser extraction is separate from cross-file resolution. Framework extraction runs after parsing and requires registration syntax, decorator/attribute, import/package evidence, qualified type, or file convention. This prevents shared method names or decorators from becoming routes for every framework.

The normalized graph contains symbols, imports, calls, variables, routes, type relationships, and DI bindings. Route handlers link to same-file or cross-file symbols only when the target is unambiguous. Type and DI facts retain their source file, line, language, relationship/registration kind, and normalized names.

## Incremental updates and persistence

`graxus update` removes all retained facts and derived edges for changed or deleted files, parses only added/modified files, merges the new graph, and refreshes imports, calls, route handlers, and relationship edges against the complete retained graph. Resolutions are cleared before recomputation so links to removed symbols or files cannot survive an update.

`graxus index` writes the complete graph to `.graxus/code/codemap.json` and rebuilds code data in `.graxus/index.db`. Incremental SQLite updates delete rows for each touched file from symbols, imports, calls, routes, type relationships, DI bindings, parser facts, and parser results before inserting fresh rows.

## Consumer interfaces

The CLI exposes routes through `graxus routes` (`--framework`, `--lang`, `--json`) and relationships plus DI bindings through `graxus types` (`--name`, `--json`). The agent API exposes normalized semantic facts in text, file, symbol, and bounded context; parser-native payloads remain in `parser_results` and are not duplicated.

Bounded context and agent exports honor token budgets, deterministic category allocations, and collection caps. Semantic facts are retained as whole records; when a budget is exhausted, the export reports truncation rather than emitting partial facts.
