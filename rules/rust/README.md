# Rust Language Rules

## Overview

Graxus parses Rust files using `tree-sitter-rust`. The `RustIndexer` handles extraction of symbols, imports (`use` statements), and function/method calls from `.rs` source files.

**Supported extensions:** `rs`

**Language ID:** `rust`

---

## Symbol Extraction

### Functions

Extracts function declarations with their full parameter signature. Also detects `#[test]` functions.

**Tree-sitter query:**
```
(function_item name: (identifier) @name parameters: (parameters) @params) @def
```

**Symbol kind:** `Function`

```rust
fn greet(name: &str) -> String {
    format!("Hello, {}", name)
}
// Extracted: name="greet", signature="fn greet(name: &str) -> String"
```

**Test detection:** A function is flagged as `is_test: true` if the line(s) immediately above it contain `#[test`. This is a heuristic check of the 1-2 lines preceding the function definition.

```rust
#[test]
fn test_parse_input() {
    assert_eq!(parse("42"), 42);
}
// Extracted: name="test_parse_input", is_test=true
```

### Structs

**Tree-sitter query:**
```
(struct_item name: (type_identifier) @name) @def
```

**Symbol kind:** `Struct`

```rust
pub struct User {
    id: u64,
    name: String,
}
// Extracted: name="User", kind=Struct
```

### Enums

**Tree-sitter query:**
```
(enum_item name: (type_identifier) @name) @def
```

**Symbol kind:** `Enum`

```rust
pub enum Color {
    Red,
    Green,
    Blue,
}
// Extracted: name="Color", kind=Enum
```

### Traits

**Tree-sitter query:**
```
(trait_item name: (type_identifier) @name) @def
```

**Symbol kind:** `Trait`

```rust
pub trait Drawable {
    fn draw(&self);
}
// Extracted: name="Drawable", kind=Trait
```

### Type Aliases

**Tree-sitter query:**
```
(type_item name: (type_identifier) @name) @def
```

**Symbol kind:** `Type`

```rust
type Result<T> = std::result::Result<T, Error>;
// Extracted: name="Result", kind=Type
```

### Constants

**Tree-sitter query:**
```
(const_item name: (identifier) @name) @def
```

**Symbol kind:** `Constant`

```rust
const MAX_RETRIES: u32 = 3;
// Extracted: name="MAX_RETRIES", kind=Constant
```

### Impl Blocks

**Tree-sitter query:**
```
(impl_item type: (type_identifier) @name) @def
```

**Symbol kind:** `Module`

Note: Impl blocks are extracted with `SymbolKind::Module`, not as a separate "impl" kind. The name captured is the type being implemented for.

```rust
impl UserService {
    pub fn new() -> Self { ... }
}
// Extracted: name="UserService", kind=Module
```

### Not Currently Extracted

- `static` items
- `mod` declarations (inline modules)
- Methods inside `impl` blocks (only the `impl` block itself is extracted)
- `macro_rules!` definitions

---

## Import Extraction

### `use` Declarations

**Tree-sitter query:**
```
(use_declaration argument: (scoped_identifier) @path) @use
```

**Import kind:** `RustUse`

Graxus extracts the full path from `use` statements. The `local_name` is set to the last segment of the path.

```rust
use std::collections::HashMap;
// Extracted: source="std::collections::HashMap", local_name="HashMap"

use crate::models::User;
// Extracted: source="crate::models::User", local_name="User"

use super::utils::parse_config;
// Extracted: source="super::utils::parse_config", local_name="parse_config"
```

### Not Currently Extracted

- Glob imports: `use std::collections::*` (the `*` is captured but the query may not match cleanly)
- Grouped imports: `use std::{collections::HashMap, io::Read}` (only top-level path captured)
- `extern crate` declarations

---

## Import Resolution

Graxus resolves Rust `use` paths to actual file paths using these strategies.

### `crate::` Prefix

Strips `crate::` and resolves relative to the `src/` directory.

- `crate::models::user` resolves to `src/models/user.rs` or `src/models/user/mod.rs`
- **Confidence:** 95% (`NamedImportExactExport`)

### `super::` Prefix

Strips `super::` and resolves relative to the parent directory of the current file.

- `super::utils` from `src/models/user.rs` resolves to `src/models/utils.rs` or `src/models/utils/mod.rs`
- **Confidence:** 95% (`NamedImportExactExport`)

### `self::` Prefix

Strips `self::` and resolves relative to the current file's directory.

- `self::helper` from `src/models/mod.rs` resolves to `src/models/helper.rs`
- **Confidence:** 95% (`NamedImportExactExport`)

### Bare Paths (no prefix)

For paths like `foo::bar` that have multiple segments but no prefix:

- Tries `src/foo/bar.rs` and `src/foo/bar/mod.rs`
- Also tries the last segment alone: `src/bar.rs`
- **Confidence:** 60% (`PathMatchOnly`)

### Single-Segment Paths (external crates)

Paths with a single segment (e.g., `use serde`) are assumed to be external crate references and are **not resolved**. They remain with `resolved_file: None`.

### Module File Resolution

For any path, graxus tries two file patterns:

1. `base/path/segments.rs` (direct file)
2. `base/path/segments/mod.rs` (module directory)

### Confidence Score Summary

| Resolution Method | Score | Label |
|---|---|---|
| `crate::` exact path match | 95% | Exact |
| `super::` exact path match | 95% | Exact |
| `self::` exact path match | 95% | Exact |
| Bare path match (multiple segments) | 60% | Medium |
| Single segment (external crate) | 0% | Unresolved |

---

## Call Extraction

Graxus extracts two kinds of calls from Rust code.

### Function Calls

**Tree-sitter query:**
```
(call_expression function: (identifier) @callee) @call
```

**Call kind:** `FunctionCall`

```rust
println!("goodbye");
// Extracted: callee_text="println"
```

### Path Calls (qualified function calls)

**Tree-sitter query:**
```
(call_expression function: (scoped_identifier) @path) @call
```

**Call kind:** `PathCall`

```rust
std::fs::read_to_string("file.txt")?;
// Extracted: callee_text="std::fs::read_to_string"
```

### Not Currently Extracted

- Method calls via `.` syntax (`self.method()`, `vec.len()`)
- Trait method calls
- Macro invocations (only `println!` as a function call, not as a macro)

---

## Visibility

Graxus currently marks all Rust symbols as `visibility: Public` by default. The actual `pub`, `pub(crate)`, `pub(super)`, and `pub(in path)` qualifiers are not parsed from the AST in the current implementation. This is a known simplification.

**Rust visibility levels (for reference, not currently enforced by graxus):**

| Modifier | Scope |
|---|---|
| `pub` | Public to all |
| `pub(crate)` | Public within the crate |
| `pub(super)` | Public to parent module |
| `pub(in path)` | Public to specified path |
| (no modifier) | Private to current module |

---

## Naming Conventions

| Entity | Convention | Example |
|---|---|---|
| Functions / Methods | snake_case | `get_user_by_id`, `parse_config` |
| Variables / Constants | snake_case / SCREAMING_SNAKE | `user_name`, `MAX_RETRIES` |
| Structs | PascalCase | `UserService`, `HttpClient` |
| Enums | PascalCase | `Color`, `HttpStatus` |
| Traits | PascalCase | `Drawable`, `Serialize` |
| Type aliases | PascalCase | `Result`, `BoxFuture` |
| Modules | snake_case | `models`, `user_service` |
| Test functions | snake_case with `test_` prefix | `test_parse_input`, `test_empty_string` |

---

## Attribute Patterns

Graxus uses attribute detection for specific patterns:

### `#[test]`

Detected by scanning the 1-2 lines above a function definition for `#[test`. Functions with this attribute are marked `is_test: true`.

### Not Currently Parsed

- `#[derive(...)]` -- not extracted or stored
- `#[cfg(test)]` -- not used for module-level test detection
- `#[tokio::main]` / `#[async_std::main]` -- not specially handled
- `#[allow(...)]`, `#[warn(...)]` -- not extracted
- Custom procedural macros

---

## Edge Cases and Gotchas

1. **Methods inside `impl` blocks**: Only the `impl` block itself is extracted (as `SymbolKind::Module`). Individual methods within the impl are not separately extracted as symbols.

2. **Trait methods**: Methods defined in a `trait` are not individually extracted. Only the trait declaration itself is captured.

3. **Macro-generated code**: Code generated by macros (`vec![]`, `derive()`, custom macros) is not parsed since tree-sitter operates on the source text, not the expanded form.

4. **`pub(crate)` and other restricted visibility**: All symbols are marked `Public` regardless of actual visibility modifier. The current implementation does not parse visibility qualifiers from the AST.

5. **Wildcard imports**: `use std::collections::*` may not be cleanly captured. The query targets `scoped_identifier` which requires at least one `::` segment.

6. **Multi-segment grouped imports**: `use std::{collections::HashMap, io::Read}` -- only the outer path may be captured, not individual items within the group.

7. **Extern crate**: `extern crate serde;` is not extracted by the current queries.

8. **Inline modules**: `mod utils { ... }` declarations are not extracted as symbols. Only file-level module references would appear if `mod` declarations were parsed.

9. **Signature capture**: Function signatures are captured as `fn {name}{params}` (e.g., `fn add(a: i32, b: i32)`). Return types are included only if they appear in the parameter list node's sibling text, which depends on tree-sitter's AST structure.

10. **Test detection heuristic**: The `#[test]` check is a simple line-text scan, not an AST-based attribute query. This means it checks the raw text of the 1-2 lines above the function, which could false-positive on comments containing `#[test`.