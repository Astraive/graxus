# JavaScript / TypeScript Language Rules

## Overview

Graxus parses JavaScript and TypeScript files using `tree-sitter-typescript`. Both JS and TS share the same indexer (`TypeScriptIndexer`) since the grammar covers both languages. This document describes how graxus extracts symbols, imports, and calls from JS/TS source files.

**Supported extensions:** `ts`, `tsx`, `mts`, `cts`, `js`, `jsx`, `mjs`, `cjs`

**Language ID:** `typescript` (JS files also map to the `typescript` indexer in the registry)

---

## Symbol Extraction

Graxus extracts the following symbol kinds from JS/TS files.

### Functions

Extracts named function declarations with their full parameter signature.

**Tree-sitter query:**
```
(function_declaration name: (identifier) @name parameters: (formal_parameters) @params) @def
```

**Symbol kind:** `Function`

**Example:**
```typescript
function greet(name: string, age: number): string {
  return "";
}
// Extracted: name="greet", signature="function greet(name: string, age: number): string"
```

**Test detection:** A function is flagged as `is_test: true` if its name starts with `test_` or `test ` (case-insensitive).

### Classes

**Tree-sitter query:**
```
(class_declaration name: (type_identifier) @name) @def
```

**Symbol kind:** `Class`

```typescript
class UserService { }
// Extracted: name="UserService", kind=Class
```

### Interfaces

**Tree-sitter query:**
```
(interface_declaration name: (type_identifier) @name) @def
```

**Symbol kind:** `Interface`

```typescript
interface Config { port: number; }
// Extracted: name="Config", kind=Interface
```

### Type Aliases

**Tree-sitter query:**
```
(type_alias_declaration name: (type_identifier) @name) @def
```

**Symbol kind:** `Type`

```typescript
type UserID = string | number;
// Extracted: name="UserID", kind=Type
```

### Enums

**Tree-sitter query:**
```
(enum_declaration name: (identifier) @name) @def
```

**Symbol kind:** `Enum`

```typescript
enum Direction { Up, Down, Left, Right }
// Extracted: name="Direction", kind=Enum
```

### Constants and Variables (lexical declarations)

**Tree-sitter query:**
```
(lexical_declaration (variable_declarator name: (identifier) @name)) @def
```

**Symbol kind:** `Constant`

Covers `const` and `let` declarations at module level.

```typescript
const API_URL = "https://api.example.com";
let retryCount = 3;
// Extracted: name="API_URL", kind=Constant
// Extracted: name="retryCount", kind=Constant
```

### Test Blocks (it/test calls)

Graxus detects test framework calls (`it(...)` and `test(...)`) that contain arrow functions.

**Tree-sitter query:**
```
(call_expression function: (identifier) @test_name arguments: (arguments (string) @desc (arrow_function) @fn)) @test
```

**Symbol kind:** `Function` with `is_test: true`

```typescript
it("should return true", () => {
  expect(true).toBe(true);
});
// Extracted: name="should return true", is_test=true
```

---

## Import Extraction

### Named Imports

**Tree-sitter query:**
```
(import_statement (import_clause (named_imports (import_specifier name: (identifier) @name))) source: (string) @source) @import
```

**Import kind:** `NamedImport`

```typescript
import { useState, useEffect } from "react";
// Extracted: source="react", local_name="useState" (one ImportFact per named import)
```

### Default Imports

**Tree-sitter query:**
```
(import_statement (import_clause (identifier) @name) source: (string) @source) @import
```

**Import kind:** `DefaultImport`

```typescript
import React from "react";
// Extracted: source="react", local_name="React"
```

### Namespace Imports

**Tree-sitter query:**
```
(import_statement (import_clause (namespace_import (identifier) @name)) source: (string) @source) @import
```

**Import kind:** `NamespaceImport`

```typescript
import * as utils from "./utils";
// Extracted: source="./utils", local_name="utils"
```

### Not Currently Extracted

- `require('module')` (CommonJS) -- not parsed by the current tree-sitter queries
- Dynamic `import()` expressions
- Side-effect-only imports (`import "./polyfill"`)

---

## Import Resolution

When graxus resolves an import source to an actual file, it assigns a confidence score.

### Relative Imports (`./foo`, `../bar`)

The resolver tries these strategies in order:

1. **Exact file match** with extension variants: `.ts`, `.tsx`, `.js`, `.jsx`, `.mts`, `.cts`
   - `import { x } from "./utils"` resolves to `./utils.ts` if it exists
   - **Confidence:** 95% (`NamedImportExactExport`)

2. **Index file match**: looks for `index.ts`, `index.tsx`, `index.js`, etc. in the target directory
   - `import { x } from "./components"` resolves to `./components/index.ts`
   - **Confidence:** 95% (`NamedImportExactExport`)

### Bare Module Imports (non-relative)

For imports like `import { x } from "my-module"`:

1. **Stem match** in `src/`, `lib/`, or project root
   - Looks for `my-module.ts`, `my-module.tsx`, etc.
   - **Confidence:** 60% (`PathMatchOnly`)

2. **Direct path match** with extension appended
   - **Confidence:** 60% (`PathMatchOnly`)

### Alias Paths (`@/...`)

Paths starting with `@` are skipped entirely (unresolved). These require project-specific path mapping that graxus does not currently handle.

### Confidence Score Summary

| Resolution Method | Score | Label |
|---|---|---|
| Named import, exact file match | 95% | Exact |
| Default import, exact file match | 88% | High |
| Namespace import, exact file match | 93% | High |
| Path match only (stem search) | 60% | Medium |
| Unresolved | 0% | Unresolved |

---

## Call Extraction

Graxus extracts three kinds of calls from JS/TS code.

### Function Calls

**Tree-sitter query:**
```
(call_expression function: (identifier) @callee) @call
```

**Call kind:** `FunctionCall`

```typescript
fetch("/api/data");
// Extracted: callee_text="fetch"
```

### Method Calls

**Tree-sitter query:**
```
(call_expression function: (member_expression object: (identifier) @object property: (property_identifier) @property)) @call
```

**Call kind:** `MethodCall`

```typescript
console.log("goodbye");
// Extracted: object="console", callee_text="log"
```

### Constructor Calls

**Tree-sitter query:**
```
(new_expression constructor: (identifier) @callee) @call
```

**Call kind:** `ConstructorCall`

```typescript
const router = new Router();
// Extracted: callee_text="Router"
```

---

## Naming Conventions

Graxus does not enforce naming conventions, but the following are the standard patterns it expects:

| Entity | Convention | Example |
|---|---|---|
| Functions | camelCase | `getUserById`, `handleSubmit` |
| Variables / Constants | camelCase or UPPER_SNAKE | `userName`, `API_URL` |
| Classes | PascalCase | `UserService`, `HttpClient` |
| Interfaces | PascalCase (often `I`-prefixed in some codebases) | `Config`, `IConfig` |
| Types | PascalCase | `UserID`, `ApiResponse` |
| Enums | PascalCase | `Direction`, `HttpStatus` |
| Test functions | `test_` prefix or descriptive `it(...)` | `test_parseInput`, `"should return 200"` |

---

## Framework-Specific Patterns

### React Components

React functional components are extracted as regular functions. JSX/TSX files use the same parser. Class components are extracted as classes.

```tsx
// Functional component -- extracted as Function
export function App() {
  return <div>Hello</div>;
}

// Class component -- extracted as Class
export class App extends React.Component { }
```

React hooks (`useState`, `useEffect`, etc.) appear as function calls within components.

### Express Routes

Route handler functions are extracted as regular functions. The Express router pattern is not specially treated:

```typescript
router.get("/users", async (req, res) => { ... });
// The arrow function is not extracted as a symbol (anonymous)
// router.get appears as a method call
```

### Next.js Pages

Next.js page components in `pages/` or `app/` directories are extracted as regular functions or default exports. No special routing metadata is extracted.

---

## Visibility and Export Status

All extracted symbols are marked `exported: true` and `visibility: Public` by default. Graxus does not currently analyze `export` statements to determine export status -- this is a simplification in the current implementation.

Test functions (detected via `it()`/`test()` calls) are marked `exported: false` and `visibility: Private`.

---

## Edge Cases and Gotchas

1. **Arrow functions as module-level `const`**: `const foo = () => {}` is extracted as `Constant`, not `Function`. The arrow function itself is not separately extracted.

2. **Re-exports**: `export { foo } from "./bar"` is not currently parsed. Only the import side may be partially captured.

3. **CommonJS**: `require()` calls are not extracted by the current tree-sitter queries. Only ESM `import` statements are parsed.

4. **Dynamic imports**: `import("./module")` is not extracted.

5. **Destructured imports**: Each named import in `import { a, b, c } from "x"` generates a separate `ImportFact`.

6. **Alias paths**: Imports using path aliases (e.g., `@/components/Foo`) are not resolved. They remain with `resolved_file: None`.

7. **TypeScript type-only imports**: `import type { Foo } from "bar"` is parsed the same as a regular named import. The `type` keyword does not affect extraction.

8. **String quote normalization**: Import sources are normalized by stripping surrounding `"` and `'` characters.