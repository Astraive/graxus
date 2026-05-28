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
2. Copy originals to `.graxus/snapshots/{id}/`
3. Apply changes
4. Snapshot metadata saved for rollback

```bash
# Rollback
graxus replace --rollback <snapshot-id>
```

## Related Notes

* [[ARCHITECTURE]]
* [[CLI]]
