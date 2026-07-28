# Graxus v0.2 Roadmap

## Priority 1: `graxus update` — Incremental Indexing

**Goal:** Only re-index changed files instead of full re-scan.

### Current Problem
`graxus index` re-scans ALL files every time — slow on large repos.

### Solution
```
graxus update              # Incremental (changed files only)
graxus update --full       # Force full re-scan
graxus update --dry-run    # Show what would change
```

### How It Works
1. Load previous file hashes from `.graxus/files.json`
2. Re-scan project, compare hashes
3. Compute diff: added / modified / deleted files
4. Only re-parse changed files with tree-sitter (codemap) and markdown (docgraph)
5. Remove graph/codemap entries for deleted files
6. Add entries for new files
7. Update entries for modified files
8. Save updated indexes

### Implementation Steps
- [x] Add `FileDiff` struct to `graxus-core::scanner` with `added`, `modified`, `removed` fields
- [x] Add `diff_scan(root, config, previous_files) -> FileDiff` function
- [x] Add `Update` CLI command to `graxus-cli`
- [x] Modify `graxus-codemap` to support incremental updates (remove/add symbols per file)
- [x] Modify `graxus-docgraph` to support incremental updates (remove/add nodes per file)
- [x] Add `--dry-run` flag that prints diff without applying
- [x] Add `--full` flag that delegates to existing `graxus index`
- [x] Snapshot before mutation (existing `graxus-index` snapshot system)

### Key Files
- `crates/graxus-core/src/scanner.rs` — add diff logic
- `crates/graxus-codemap/src/lib.rs` — add `update()` method on `CodemapBuilder`
- `crates/graxus-docgraph/src/lib.rs` — add `update()` to graph builder
- `crates/graxus-cli/src/main.rs` — add `Update` command
- `crates/graxus-cli/src/commands/update.rs` — new file

---

## Priority 2: Better Import Resolution

**Goal:** Resolve import paths to actual files with confidence levels.

### Current State
Import resolution exists in `graxus-codemap/src/resolver/import_resolver.rs` but is basic.

### Improvements
- [ ] TypeScript: resolve `./foo` → `./foo.ts`, `./foo.tsx`, `./foo/index.ts`
- [ ] Rust: resolve `crate::auth::session` → `src/auth/session.rs` or `src/auth/session/mod.rs`
- [ ] Go: resolve package paths to directories
- [ ] Python: resolve `app.auth.session` → `app/auth/session.py`
- [ ] Store `resolved_file` and `confidence` on each import

---

## Priority 3: Call Resolution

**Goal:** Resolve call sites to their definitions across files.

### Improvements
- [ ] Match callee names against known symbols
- [ ] Track which file defines each symbol
- [ ] Resolve `obj.method()` to the file that defines the method
- [ ] Assign confidence: high (exact match), medium (name match), low (global match)

---

## Priority 4: Unit Tests

**Goal:** Comprehensive test coverage for all crates.

- [ ] graxus-core: config parsing, file scanning, glob matching
- [ ] graxus-docgraph: frontmatter, wiki links, tags, graph building
- [ ] graxus-codemap: tree-sitter parsing for each language, import resolution
- [ ] graxus-index: JSON storage, snapshot creation/rollback
- [ ] graxus-edit: find, replace, safety checks
- [ ] graxus-agent-api: bridge, context queries
- [ ] Integration tests: full index/update/find/replace pipeline

---

## Priority 5: CLI Improvements

- [ ] Progress bars during long operations (use `indicatif`)
- [ ] Better colored output for errors/warnings
- [ ] `graxus watch` — auto-update on file changes (use `notify` crate)
- [ ] `graxus diff` — show what changed since last index
- [ ] Shell completions (bash, zsh, fish, powershell)
