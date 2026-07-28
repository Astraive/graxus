# Python Language Rules

## Overview

Graxus parses Python files using `tree-sitter-python`. The `PythonIndexer` handles extraction of symbols, imports, and function/method calls from `.py` and `.pyi` source files.

**Supported extensions:** `py`, `pyi`

**Language ID:** `python`

---

## Symbol Extraction

### Functions

Extracts function definitions (`def`) with their name.

**Tree-sitter query:**
```
(function_definition name: (identifier) @name) @def
```

**Symbol kind:** `Function`

```python
def greet(name: str) -> str:
    return f"Hello, {name}"
# Extracted: name="greet", kind=Function
```

**Visibility:** Determined by the first character of the function name:
- Starts with `_` -> `exported: false`, `visibility: Private`
- Does not start with `_` -> `exported: true`, `visibility: Public`

**Test detection:** A function is flagged as `is_test: true` if its name:
- Starts with `test_` (standard unittest/pytest convention)
- Starts with `Test` (test class methods)

```python
def test_parse_input():
    assert parse("42") == 42
# Extracted: name="test_parse_input", is_test=true
```

### Classes

**Tree-sitter query:**
```
(class_definition name: (identifier) @name) @def
```

**Symbol kind:** `Class`

```python
class UserService:
    def __init__(self):
        pass
# Extracted: name="UserService", kind=Class
```

### Not Currently Extracted

- Variable assignments (`x = 42`, `CONFIG = {...}`)
- Decorated functions (the decorator is not captured, but the underlying `def` is)
- Methods inside classes (only the class itself is extracted)
- `async def` functions (may or may not match depending on tree-sitter node structure)
- Module-level constants

---

## Import Extraction

### From Imports

**Tree-sitter query:**
```
(import_from_statement module_name: (dotted_name) @module name: (dotted_name) @name) @import
```

**Import kind:** `FromImport`

```python
from os.path import join
# Extracted: source="os.path", local_name="join", imported_name="join"

from .models import User
# Extracted: source=".models", local_name="User", imported_name="User"
```

### Direct Imports

**Tree-sitter query:**
```
(import_statement name: (dotted_name) @module) @import
```

**Import kind:** `PythonImport`

```python
import os
# Extracted: source="os", local_name="os"

import os.path
# Extracted: source="os.path", local_name="path"
```

The `local_name` is derived from the last segment of the dotted name (split by `.`).

### Not Currently Extracted

- Star imports: `from module import *`
- Aliased imports: `import numpy as np` (the alias `np` is not captured)
- Multi-name from imports: `from os import path, getcwd` (each name may or may not generate a separate fact depending on tree-sitter node structure)

---

## Import Resolution

Graxus resolves Python import paths to actual files using these strategies.

### Relative Imports (`.foo`, `..foo`)

For imports starting with `.`:

1. Counts the number of leading dots to determine how many directories to walk up
2. Strips the dots and converts the remaining dotted path to a file path
3. Tries `module.py` and `module/__init__.py`

```python
from .models import User
# From src/api/views.py:
#   1 dot -> stay in src/api/
#   "models" -> try src/api/models.py, then src/api/models/__init__.py
# Confidence: 95% (NamedImportExactExport)

from ..utils import helper
# From src/api/views.py:
#   2 dots -> walk up to src/
#   "utils" -> try src/utils.py, then src/utils/__init__.py
# Confidence: 95% (NamedImportExactExport)
```

**Confidence:** 95% (`NamedImportExactExport`)

### Absolute Imports

For dotted module paths without leading dots:

1. Converts dots to path separators
2. Tries these candidates:
   - `module/path.py`
   - `module/path/__init__.py`
   - `src/module/path.py`
   - `src/module/path/__init__.py`

```python
from mypackage.models import User
# Tries: mypackage/models.py, mypackage/models/__init__.py,
#        src/mypackage/models.py, src/mypackage/models/__init__.py
```

**Confidence:** 60% (`PathMatchOnly`)

### Confidence Score Summary

| Resolution Method | Score | Label |
|---|---|---|
| Relative import (`.foo`, `..foo`) | 95% | Exact |
| Absolute import, path match | 60% | Medium |
| Unresolved (stdlib/external) | 0% | Unresolved |

---

## Call Extraction

Graxus extracts two kinds of calls from Python code.

### Function Calls

**Tree-sitter query:**
```
(call function: (identifier) @callee) @call
```

**Call kind:** `FunctionCall`

```python
print("goodbye")
# Extracted: callee_text="print"

len([1, 2, 3])
# Extracted: callee_text="len"
```

### Attribute/Method Calls

**Tree-sitter query:**
```
(call function: (attribute object: (identifier) @object attribute: (identifier) @property)) @call
```

**Call kind:** `MethodCall`

```python
user.get_name()
# Extracted: object="user", callee_text="get_name"

os.path.join("a", "b")
# Extracted: object="os", callee_text="path" (only first level captured)
```

### Not Currently Extracted

- Chained method calls: `obj.method1().method2()` (only the outermost call is captured)
- Calls on non-identifier expressions: `[1,2,3].append(4)`
- `super()` calls
- Lambda calls

---

## Visibility

Python does not have access modifiers like `pub` or `private`. Graxus uses naming conventions:

| Pattern | Visibility | Exported |
|---|---|---|
| `def function_name(...)` | `Public` | `true` |
| `def _private_func(...)` | `Private` | `false` |
| `def __dunder__(...)` | `Public` | `true` |
| `class ClassName` | `Public` | `true` |
| `class _PrivateClass` | `Private` | `false` |

The `__all__` list is not currently parsed to determine true public API.

---

## Naming Conventions

| Entity | Convention | Example |
|---|---|---|
| Functions | snake_case | `get_user_by_id`, `parse_config` |
| Variables | snake_case | `user_name`, `retry_count` |
| Constants | UPPER_SNAKE_CASE | `MAX_RETRIES`, `API_URL` |
| Classes | PascalCase | `UserService`, `HttpClient` |
| Private functions | `_` prefix | `_validate_input`, `_helper` |
| Dunder methods | `__double_underscore__` | `__init__`, `__str__` |
| Test functions | `test_` prefix | `test_parse_input`, `test_empty_string` |
| Test classes | `Test` prefix | `TestUserService`, `TestParser` |

---

## Decorator Patterns

Graxus does not specially parse decorators. However, decorated functions and classes are still extracted because the underlying `def` / `class` node is captured.

Common decorator patterns (for reference):

### `@property`
```python
class User:
    @property
    def name(self) -> str:
        return self._name
# The `def name` is extracted as a Function symbol
```

### `@staticmethod` / `@classmethod`
```python
class MathUtils:
    @staticmethod
    def add(a: int, b: int) -> int:
        return a + b
# The `def add` is extracted as a Function symbol
```

### `@pytest.fixture` / `@pytest.mark.parametrize`
```python
@pytest.fixture
def db_session():
    ...
# The `def db_session` is extracted as a Function symbol, is_test=false
```

---

## Type Hints

Graxus does not parse type hints from function signatures. Type annotations in the source are part of the tree-sitter AST but are not extracted into the `SymbolFact` or `ImportFact` structures.

Common type hint patterns (for reference):

```python
from typing import Optional, List, Dict

def get_users(active: bool = True) -> List[User]:
    ...

def find_user(user_id: int) -> Optional[User]:
    ...
```

The `from typing import Optional` would be extracted as a `FromImport` fact, but the type annotations themselves are not stored.

---

## Framework-Specific Patterns

### Django

Django models, views, and URL patterns are extracted as regular classes and functions:

```python
from django.db import models

class User(models.Model):
    name = models.CharField(max_length=100)
# Extracted: name="User", kind=Class

def user_list(request):
    return render(request, "users/list.html")
# Extracted: name="user_list", kind=Function
```

Django's `models.Model` inheritance and field definitions are not specially handled.

### Flask

```python
from flask import Flask, route

app = Flask(__name__)

@app.route("/users")
def get_users():
    return jsonify([])
# Extracted: name="get_users", kind=Function
```

The `@app.route` decorator is not parsed for route metadata.

### FastAPI

```python
from fastapi import FastAPI

app = FastAPI()

@app.get("/items/{item_id}")
async def read_item(item_id: int):
    return {"item_id": item_id}
# Extracted: name="read_item", kind=Function (if async def is matched)
```

---

## Edge Cases and Gotchas

1. **Methods not separately extracted**: Functions defined inside a class body are not individually extracted as symbols. Only the class itself is captured. Top-level functions are extracted.

2. **Decorators not stored**: Decorator metadata (`@property`, `@staticmethod`, route definitions) is not extracted. Only the underlying `def` or `class` is captured.

3. **`__all__` not parsed**: The module's public API as defined by `__all__` is not analyzed. Visibility is determined solely by the `_` prefix convention.

4. **Aliased imports**: `import numpy as np` -- the alias `np` is not captured. The `local_name` would be derived from the module name.

5. **Star imports**: `from module import *` is not extracted by the current tree-sitter queries.

6. **`async def`**: Async function definitions may or may not be captured depending on whether `tree-sitter-python` represents them as `function_definition` nodes or a separate node type.

7. **Nested functions**: Inner functions defined within other functions are not extracted.

8. **Dynamic imports**: `importlib.import_module("name")` calls are not extracted.

9. **Test detection specificity**: Only functions with `test_` prefix or `Test` prefix are flagged. Pytest fixtures, parametrize markers, and test class methods are not specially handled for `is_test` detection.

10. **`.pyi` stub files**: Type stub files are parsed with the same rules as `.py` files. They use the same Python tree-sitter grammar.

11. **Multi-line imports**: `from os import (\n    path,\n    getcwd\n)` -- tree-sitter handles the multi-line structure, but each named import may generate a separate `ImportFact` depending on how the AST nodes are structured.