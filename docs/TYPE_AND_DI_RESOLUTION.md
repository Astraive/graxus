# Type And DI Resolution

Type and DI resolution are semantic passes after parser extraction. They operate on normalized source facts and retain explicit source evidence rather than guessing relationships from names.

## Type relationships

`TypeImplFact` records the implementing type, trait/interface or parent type, source file and line, language, and a relationship kind. Graxus currently extracts:

- explicit Rust `impl Trait for Type` relationships
- TypeScript `extends` and `implements`
- C# class, `record`, and `struct` declarations with colon-separated base/interface lists, including interface declarations
- Java `extends` and `implements`
- Kotlin colon-separated supertypes

C# records and structs are normalized as implementing types just like classes, while interface declarations can participate in explicit base/interface lists. Inherent Rust `impl Type` blocks are intentionally omitted: they have no second relationship participant and therefore cannot form a valid normalized type edge. Deduplication is deterministic and prefers explicit implementation relationships when a source construct could otherwise produce a weaker inheritance fact.

Type extraction is syntax-based and does not require the target trait, interface, or base class to be indexed in the same project. Relationship IDs are deterministic and preserve source line and relationship kind.

## Dependency injection

`DIFact` records the abstract type, concrete type, optional normalized lifetime, source file and line, language, and framework. Graxus recognizes:

- ASP.NET Core `AddSingleton`, `AddScoped`, and `AddTransient` generic service registrations, including self-registration
- NestJS `@Injectable` classes and explicit `@Module` `useClass` providers in JavaScript and TypeScript files

NestJS default scope is normalized to `singleton`; `Scope.REQUEST` becomes `scoped`, `Scope.TRANSIENT` becomes `transient`, and unknown dynamic scopes remain unset rather than guessed. NestJS DI facts preserve `javascript` or `typescript` from the file extension.

## Storage, incremental replacement, and queries

Both fact types are written during `graxus index` to codemap JSON and SQLite (`type_impls` and `di_bindings`). During `graxus update`, rows for each changed or deleted file are removed before fresh rows are inserted, so deleted or replaced relationships and bindings do not remain stale. Use:

```sh
graxus types
graxus types --name IUserService
graxus types --json
```

The same normalized facts are included in query-aware agent context and bounded exports, subject to their token, collection, and edge limits.
