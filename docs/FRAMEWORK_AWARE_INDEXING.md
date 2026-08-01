# Framework-Aware Indexing

Framework-aware indexing is a post-parser stage in `graxus-codemap`. It adds route and dependency-injection facts only when source evidence identifies a framework, rather than treating shared method names or decorators as framework semantics.

## Implemented route coverage

| Language | Frameworks and implemented evidence |
|----------|--------------------------------------|
| Python | FastAPI and Flask decorators; Django `path`/`re_path` registrations |
| Rust | Axum router calls; Actix Web and Rocket route attributes |
| Go | Gin, Fiber, and Echo receiver registrations |
| JavaScript / TypeScript | Express registrations, NestJS controller decorators, and Next.js app-router file conventions |
| C# | ASP.NET Core minimal API calls and controller attributes |
| C++ | Crow macros, Pistache route registrations, and Drogon `ADD_METHOD_TO` registrations |

Framework extractors parse each file after language extraction and require import/package, qualified receiver, decorator/attribute, registration API, or file-convention evidence. Routes are normalized with method, path, handler, source language, registration file/line, framework, middleware, and an optional resolved handler file.

## JavaScript and TypeScript details

- Express route facts retain the caller-provided `javascript` or `typescript` language.
- Next.js app-router facts derive the route language from the `.js`/`.jsx`/`.mjs`/`.cjs` or `.ts`/`.tsx` route file and map `app/**/route.*` segments to normalized paths.
- NestJS controller route extraction is implemented for its TypeScript syntax. NestJS DI extraction separately preserves JavaScript and TypeScript language for `@Injectable` classes and `useClass` providers.

## Resolution and persistence

Route handlers are resolved to same-file or cross-file normalized symbols when the target is unambiguous; unresolved handlers remain explicitly unset. `graxus index` stores routes in `.graxus/code/codemap.json` and SQLite. `graxus update` removes changed/deleted file facts, merges changed files, refreshes relationship resolution against the retained graph, and replaces touched SQLite rows so removed routes or bindings do not remain stale.

Use `graxus routes [--framework NAME] [--lang LANGUAGE] [--json]` to inspect route facts and `graxus types [--name NAME] [--json]` for type and DI facts.
