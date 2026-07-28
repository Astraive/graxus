# Type And DI Resolution

Type and DI resolution are separate from raw symbol extraction because they
represent semantic relationships rather than declarations alone:

- type implementation facts map an implementing type to a trait, interface, or
  parent type
- DI facts map an abstract contract to a concrete registration and lifetime

## Type relationships

`TypeImplFact` records the implementing type, trait/interface, source file and
line, language, and a relationship kind. Graxus currently extracts:

- explicit Rust `impl Trait for Type` relationships
- TypeScript `extends` and `implements`
- C# base/interface lists
- Java `extends` and `implements`
- Kotlin colon-separated supertypes

Inherent Rust `impl Type` blocks are intentionally omitted: they have no
second relationship participant and therefore cannot form a valid normalized
type edge. Deduplication is deterministic and prefers explicit implementation
relationships when a source construct could otherwise produce a weaker
inheritance fact.

## Dependency injection

`DIFact` records the abstract type, concrete type, optional normalized
lifetime, source file and line, language, and framework. Graxus recognizes:

- ASP.NET Core `AddSingleton`, `AddScoped`, and `AddTransient` generic service
  registrations, including self-registration
- NestJS `@Injectable` classes and explicit `@Module` `useClass` providers

NestJS default scope is normalized to `singleton`; `Scope.REQUEST` becomes
`scoped`, `Scope.TRANSIENT` becomes `transient`, and unknown dynamic scopes
remain unset rather than guessed.

## Storage and queries

Both fact types are written during `graxus index` to the codemap JSON and
SQLite (`type_impls` and `di_bindings`). Use:

```sh
graxus types
graxus types --name IUserService
graxus types --json
```

The same normalized facts are included in agent context and bounded exports.
