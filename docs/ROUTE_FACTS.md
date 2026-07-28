# Route Facts

Route facts capture HTTP endpoints as normalized records so Graxus can answer questions like "where is `/api/users/:id` handled?" across languages and frameworks.

Each route fact should eventually include:

- method
- path
- framework
- registration file
- handler symbol
- resolved handler file
- middleware chain when available

The current codebase now reserves `routes` in the codemap schema and CLI surface so extraction work can land without another structural migration.
