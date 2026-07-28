# Framework-Aware Indexing

Framework-aware indexing is the next layer above plain AST extraction. The repository now has a dedicated `frameworks/` module in `graxus-codemap` so route and DI logic can be implemented per ecosystem instead of being buried in language parsers.

The near-term target frameworks are:

- Python: FastAPI, Flask, Django
- Rust: Axum, Actix, Rocket
- Go: Gin, Fiber, Echo
- TypeScript / JavaScript: Express, NestJS, Next.js
- C#: ASP.NET
- C++: Crow, Pistache, Drogon

The current implementation is scaffold-first: descriptors and module boundaries exist now, with deeper extraction logic to be filled in incrementally framework by framework.
