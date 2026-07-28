# Type And DI Resolution

Type and DI resolution are separate from raw symbol extraction because they need semantic linking:

- type implementation facts map traits, interfaces, inheritance, and extension edges
- DI facts map abstract contracts to concrete registrations and service lifetimes

This is especially important for:

- Rust trait implementations
- TypeScript class/interface relationships
- C# inheritance and service registration
- framework-managed dependency injection in web stacks

The repository now includes dedicated fact and resolver modules plus SQLite schema placeholders for both categories.
