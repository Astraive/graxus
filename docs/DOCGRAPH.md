# Docgraph

`graxus-docgraph` models markdown and notes as a graph of pages, tags, headings, and backlinks. It exists to complement the codemap instead of competing with it.

The intended flow is:

1. Parse repo documentation into stable nodes and edges.
2. Keep that graph queryable for agent context assembly.
3. Bridge important docs back to code symbols and files in `graxus-agent-api`.
