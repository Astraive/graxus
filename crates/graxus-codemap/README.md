# graxus-codemap

This crate extracts and resolves source-code facts across the primary target languages: Rust, Python, Go, TypeScript / JavaScript, and the C family.

The current structure separates:

- language parsers
- framework-aware extraction scaffolding
- normalized facts
- semantic resolvers
