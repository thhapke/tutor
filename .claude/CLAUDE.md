# Rust Project - Code Analysis Guidelines

## General Guidline

1. Don't assume. Don't hide confusion. Surface tradeoffs.
2. Minimum code that solves the problem. Nothing speculative.
3. Touch only what you must. Clean up only your own mess.
4. Define success criteria. Loop until verified.
5. No backward compatibility without an explicit statement
6. For projects with modules use the `snafu` crate for error handling otherwise anyhow

## Error Handling with snafu

Use the `snafu` crate for error handling. Define errors using `#[derive(Debug, Snafu)]` on an enum and use `ResultExt` (`.context(...)`) to attach context at call sites.

**Naming convention:**

- Name the error enum after the file it lives in, converting to PascalCase and appending `Error`.
- Apply a sensible adjustment when the raw file name would produce an awkward name (e.g., `main.rs` → `AppError`, `utils.rs` → `UtilError`).
- Examples: `parser.rs` → `ParseError`, `server.rs` → `ServerError`, `config.rs` → `ConfigError`.

**Placement rules:**

- **By default**, define the error enum at the top of the same file where it is used.
- **Use a separate `errors.rs` file** only when the error enum is shared across multiple modules or becomes large enough to clutter the file.

## Query the Knowledge Graph First

This project ships a pre-built codegraph knowledge graph (`codegraph.bin` in the
project root). **Before reading or grepping source, query the graph** to get
exact `file:line` locations, then read only those lines. This saves 60-90% of
the tokens a read-everything approach would burn.

**Query first, read second:**

1. Query the graph to get exact locations.
2. Read only the specific files/lines returned.
3. Never `Glob`/`Grep` for a name, and never scan a module hoping to find something.

## How to use it

The **`codegraph` skill** is the single source of truth for commands, task
recipes, output format, and troubleshooting — invoke it (or follow it) whenever
you need to find functions/types, trace call paths, do refactor impact analysis,
audit tests, or explore unfamiliar code. It auto-triggers on those tasks.

Quick reminders:

- The tool auto-detects `*.bin` in the current directory — no `--graph` flag needed.
- If the graph is missing or the code has changed since it was built, rebuild:
  `codegraph build .` (workspace-aware; ~2-5s). **Queries reflect the graph as
  last built — stale results look valid, so rebuild after significant changes.**

## Priority

Querying codegraph is **MANDATORY** before reading source when searching for
functions/types, understanding call graphs, finding type usage, tracing
execution, or analyzing the public API. Read files directly only once you have
exact locations from a query.
