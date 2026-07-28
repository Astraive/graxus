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
