# Route Facts

Route facts capture HTTP endpoints as normalized records so Graxus can answer
questions such as “where is `/api/users/:id` handled?” across languages and
frameworks.

Each `RouteFact` contains:

- a deterministic ID assigned by `CodeMapBuilder`
- registration `file`, `language`, and 1-based `line`
- HTTP `method` and source-derived `path`
- source `handler` and an optional resolved `handler_file`
- exact `framework`
- a middleware chain when the framework syntax exposes one

## Extraction

Framework extraction runs after the language parser and before cross-file
resolution. Extractors require framework-specific source evidence—such as a
registration API, decorator, import/package, qualified type, or Next.js
app-router file convention—so overlapping syntax is not misattributed.

Supported frameworks:

| Language | Frameworks |
|----------|------------|
| Python | FastAPI, Flask, Django |
| Rust | Axum, Actix Web, Rocket |
| Go | Gin, Fiber, Echo |
| JavaScript / TypeScript | Express, NestJS, Next.js app-router |
| C# | ASP.NET Core minimal APIs and controller attributes |
| C++ | Crow, Pistache, Drogon |

The route resolver links handlers to same-file or cross-file symbols whenever
the normalized symbol graph provides an unambiguous target. Absence of a
resolution leaves `handler_file` empty; it does not fabricate a link.

## Storage and queries

Routes are written to both `.graxus/code/codemap.json` and the `routes` table
in `.graxus/index.db` during `graxus index`. Query them with:

```sh
graxus routes
graxus routes --framework fastapi --lang python
graxus routes --json
```
