# Go Language Rules

## Overview

Graxus parses Go files using `tree-sitter-go`. The `GoIndexer` handles extraction of symbols, imports, and function/selector calls from `.go` source files.

**Supported extensions:** `go`

**Language ID:** `go`

---

## Symbol Extraction

### Functions

Extracts function declarations with their full parameter signature. Also detects Go test functions.

**Tree-sitter query:**
```
(function_declaration name: (identifier) @name parameters: (parameter_list) @params) @func
```

**Symbol kind:** `Function`

```go
func greet(name string) string {
    return "Hello, " + name
}
// Extracted: name="greet", signature="func greet(name string) string"
```

**Visibility:** Determined by the first character of the function name:
- Uppercase first letter (`Greet`) -> `exported: true`, `visibility: Public`
- Lowercase first letter (`greet`) -> `exported: false`, `visibility: Private`

**Test detection:** A function is flagged as `is_test: true` if:
1. Its name starts with `Test` and has more than 4 characters (e.g., `TestAdd`, not just `Test`)
2. Its parameter list contains `testing.T`

```go
func TestAdd(t *testing.T) {
    if add(1, 2) != 3 {
        t.Fatal("fail")
    }
}
// Extracted: name="TestAdd", is_test=true
```

### Types (Structs and Interfaces)

**Tree-sitter query:**
```
(type_declaration (type_spec name: (type_identifier) @name)) @type
```

**Symbol kind:** `Struct`

Note: Both `struct` and `interface` type declarations use the same query and are extracted as `SymbolKind::Struct`. The current implementation does not distinguish between struct and interface types.

```go
type User struct {
    ID   int
    Name string
}
// Extracted: name="User", kind=Struct, exported=true (uppercase)

type Reader interface {
    Read(p []byte) (n int, err error)
}
// Extracted: name="Reader", kind=Struct (not Interface)
```

### Not Currently Extracted

- `const` declarations
- `var` declarations
- Methods (functions with receivers like `func (u *User) GetName() string`)
- Type constraints (generics)

---

## Import Extraction

### Grouped Imports

**Tree-sitter query:**
```
(import_declaration (import_spec_list (import_spec path: (interpreted_string_literal) @source))) @import
```

**Import kind:** `GoImport`

```go
import (
    "fmt"
    "net/http"
    "github.com/org/pkg"
)
// Extracted: source="fmt", local_name="fmt"
// Extracted: source="net/http", local_name="http"
// Extracted: source="github.com/org/pkg", local_name="pkg"
```

### Single Imports

**Tree-sitter query:**
```
(import_declaration (import_spec path: (interpreted_string_literal) @source)) @import
```

**Import kind:** `GoImport`

```go
import "fmt"
// Extracted: source="fmt", local_name="fmt"
```

The `local_name` is derived from the last segment of the import path (split by `/`).

### Not Currently Extracted

- Aliased imports: `import f "fmt"` (the alias `f` is not captured)
- Dot imports: `import . "fmt"`
- Blank imports: `import _ "github.com/lib/pq"` (side-effect imports)

---

## Import Resolution

Graxus resolves Go import paths to actual files using these strategies.

### Relative Imports (`./pkg`, `../pkg`)

For imports starting with `.`:

1. Resolves relative to the current file's directory
2. Looks for any `.go` file in the target directory
3. **Confidence:** 95% (`NamedImportExactExport`)

```go
import "./utils"
// Resolves to any .go file in ./utils/ directory
```

### Module Imports (`github.com/org/pkg`)

For standard module-style imports:

1. Extracts the last path segment (e.g., `pkg` from `github.com/org/pkg`)
2. Searches for any `.go` file whose parent directory name matches that segment
3. **Confidence:** 40% (`NameMatchSameProject`)

```go
import "github.com/myorg/mypkg"
// Looks for .go files in directories named "mypkg"
```

### Standard Library Imports

Standard library imports like `"fmt"`, `"net/http"` are single or two-segment paths. They are not resolved to local files (external dependency).

### Confidence Score Summary

| Resolution Method | Score | Label |
|---|---|---|
| Relative import, `.go` file found | 95% | Exact |
| Module import, package name match | 40% | Low |
| Unresolved (external/std) | 0% | Unresolved |

---

## Call Extraction

Graxus extracts two kinds of calls from Go code.

### Function Calls

**Tree-sitter query:**
```
(call_expression function: (identifier) @callee) @call
```

**Call kind:** `FunctionCall`

```go
fmt.Println("goodbye")
// Extracted (as function call): callee_text="fmt" -- note: this is the selector operand
```

For direct function calls:
```go
doSomething()
// Extracted: callee_text="doSomething"
```

### Selector Calls (method calls)

**Tree-sitter query:**
```
(call_expression function: (selector_expression operand: (identifier) @object field: (field_identifier) @property)) @call
```

**Call kind:** `SelectorCall`

```go
fmt.Println("goodbye")
// Extracted: object="fmt", callee_text="Println"

user.GetName()
// Extracted: object="user", callee_text="GetName"
```

---

## Visibility

Go's visibility system is based on identifier capitalization. Graxus determines visibility at extraction time:

| First Character | Visibility | Exported |
|---|---|---|
| Uppercase (`User`, `GetID`) | `Public` | `true` |
| Lowercase (`user`, `getID`) | `Private` | `false` |

This applies to all symbol kinds (functions, types, etc.).

---

## Package System

Graxus does not extract `package` declarations. The package name is not stored as a symbol. Import resolution relies on directory structure rather than package names.

**Package-related patterns (for reference):**

- `package main` -- standard entry point
- `package mylib` -- library package
- Package name typically matches the directory name (convention, not enforced by the compiler)

---

## Naming Conventions

| Entity | Convention | Example |
|---|---|---|
| Exported functions | MixedCase (PascalCase) | `GetUserByID`, `NewServer` |
| Unexported functions | camelCase | `parseConfig`, `handleRequest` |
| Exported types | MixedCase (PascalCase) | `UserService`, `HTTPClient` |
| Unexported types | camelCase | `userService`, `config` |
| Exported constants | MixedCase (PascalCase) | `MaxRetries`, `DefaultPort` |
| Unexported constants | camelCase | `maxRetries`, `defaultPort` |
| Test functions | `Test` prefix + PascalCase | `TestAdd`, `TestUserCreation` |
| Test helpers | `test` prefix (unexported) | `testSetup`, `createTestUser` |

---

## Interface Satisfaction

Go interfaces are satisfied implicitly (no `implements` keyword). Graxus does not currently analyze interface satisfaction -- this would require type inference across files.

```go
type Reader interface {
    Read(p []byte) (n int, err error)
}

// FileRead satisfies Reader implicitly -- graxus does not detect this relationship
type FileRead struct{}
func (f FileRead) Read(p []byte) (n int, err error) { ... }
```

---

## Edge Cases and Gotchas

1. **Methods not extracted**: Functions with receivers (`func (u *User) GetName() string`) are not extracted as symbols. Only standalone functions are captured. The receiver function would appear as a regular `Function` symbol if the query matches, but the receiver parameter would be included in the signature.

2. **Struct vs Interface not distinguished**: Both `type X struct {}` and `type X interface {}` are extracted as `SymbolKind::Struct`. The current tree-sitter query does not differentiate between struct and interface type specs.

3. **Constants and variables not extracted**: `const` and `var` declarations are not captured by the current queries.

4. **Aliased imports**: `import f "fmt"` does not capture the alias `f`. The `local_name` would still be derived from the last path segment (`fmt`).

5. **Dot imports**: `import . "fmt"` is not specially handled. The `.` would be part of the path string.

6. **Blank imports**: `import _ "github.com/lib/pq"` may or may not be captured depending on whether the tree-sitter query matches the blank identifier node.

7. **Package name not stored**: The `package` declaration is not extracted. File identity is based on path, not package name.

8. **Test detection strictness**: Only functions matching `Test*` with `testing.T` in the parameter list are flagged as tests. Table-driven test subtests (`t.Run("name", ...)`) are not individually detected.

9. **Selector vs function call**: `fmt.Println("x")` is captured as a `SelectorCall` (object="fmt", property="Println"), not as a `FunctionCall`. Direct calls like `doSomething()` are `FunctionCall`.

10. **Generated files**: Go files with `// Code generated ... DO NOT EDIT.` comments are not specially handled. They are parsed the same as hand-written code.