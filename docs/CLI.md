---
title: Graxus CLI
type: docs
status: active
tags:
  - graxus
  - cli
---

# Graxus CLI

## Commands

### Project Management

```bash
graxus init          # Create graxus.yaml and .graxus/
graxus index         # Scan files, build graph and codemap
graxus status        # Show project info
```

### Docs Graph

```bash
graxus graph docs           # Print docs graph summary
graxus graph docs --json    # Output full graph as JSON
graxus graph docs --file F  # Graph for specific file
graxus graph backlinks F    # Show backlinks to a file
graxus graph tags           # List all tags
```

### Code Codemap

```bash
graxus codemap show           # Print codemap summary
graxus codemap show --json    # Output full codemap as JSON
graxus symbols                # List all symbols
graxus symbols --file F       # Symbols in specific file
graxus codemap imports F      # Imports of a file
graxus codemap impacted F     # Files impacted by changes to F
```

### Search

```bash
graxus find "pattern"         # Search everywhere
graxus find "pattern" --code  # Code only
graxus find "pattern" --docs  # Docs only
graxus find "symbol" --symbol # Symbol search
```

### Replace

```bash
graxus replace "old" "new" --preview  # Preview changes
graxus replace "old" "new" --apply    # Apply changes (creates snapshot)
```

### Agent Context

```bash
graxus context --query "auth"      # Context for a query
graxus context --file F            # Context for a file
graxus context --symbol NAME       # Context for a symbol
graxus agent-export                # Export full context as JSON
```

## Related Notes

* [[ARCHITECTURE]]
* [[SAFETY]]
