---
title: Graxus Safety Model
type: docs
status: active
tags:
  - graxus
  - safety
---

# Graxus Safety Model

## Principles

Graxus must be safe because AI agents may use it to modify code.

## Rules

1. **Preview by default** — Never modify files without explicit `--apply`
2. **Snapshots** — Create backup before any mutation
3. **Respect .gitignore** — Never touch ignored files
4. **No binary files** — Detect and skip binary content
5. **No .git/** — Never modify git internals
6. **Max file limits** — Limit bulk operations (default: 100 files)
7. **No secrets** — Never edit .env, credentials, or secret files

## Snapshot System

Before any replace operation:

1. Collect affected files
2. Copy originals to `.graxus/snapshots/{id}/` with a `meta.json` manifest
3. Apply changes
4. Snapshot metadata saved for rollback

```bash
# List snapshots created by replace/update
graxus history

# Rollback a snapshot
graxus rollback <snapshot-id> --apply
```

## Related Notes

* [[ARCHITECTURE]]
* [[CLI]]
