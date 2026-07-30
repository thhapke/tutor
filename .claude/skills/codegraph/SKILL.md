---
name: codegraph
description: Query a pre-built Rust knowledge graph instead of reading/grepping source. Use FIRST when finding functions/types, tracing call paths, doing impact analysis for a refactor, auditing tests, or exploring an unfamiliar Rust crate/workspace. Saves 60-90% tokens vs. reading files.
---

# Codegraph — Query Rust Code Before Reading It

`codegraph` answers structural questions about a Rust crate/workspace from a
pre-built graph (`*.bin`) in **< 100ms**. Query it to get exact `file:line`
locations, then read ONLY those lines. This avoids grepping and reading whole
modules — typically **60-90% fewer tokens**.

## Golden Rule

**Query first, read second.** Never `Glob`/`Grep` for a function or type name,
and never read a module hoping to find something — run a codegraph query, then
`Read` only the returned `file:line`.

## Setup (do this once)

```bash
# The tool auto-detects a *.bin graph in the current directory — no --graph flag needed.
# If no .bin exists (or code changed a lot), build it:
codegraph build .          # single crate → <crate>.bin; workspace → <workspace>.bin
```

- Workspace detection is automatic (`[workspace]` in `Cargo.toml`); all member
  crates go into one unified graph, and queries cross crate boundaries.
- Rebuild after significant code changes (~2-5s for typical codebases).

## Commands

All query commands accept `--graph <FILE>` (optional; auto-detected).

### Find code

```bash
codegraph find <pattern>             # unified search: functions + types + usages. START HERE if unsure.
codegraph function <pattern>         # functions by name → qualified_name -> ReturnType at file:line
codegraph type <pattern>             # a type's declaration + ALL usages (params, returns, fields, refs, impls)
codegraph type <pattern> --declaration-only   # just the declaration site (terse)
codegraph impact <symbol>            # refactor blast radius: callers + usage sites + trait impls, in one report
codegraph public                     # the public API surface
```

### Understand structure

```bash
codegraph file                       # list every file in the graph
codegraph file parser.rs -d -f -t    # a file's description (-d), functions (-f), types (-t)
codegraph trait-impls                # all trait implementations
codegraph unsafe-functions           # unsafe / extern functions
codegraph entry-points               # main + tests
```

### Trace execution flow

```bash
codegraph code-path <function>          # UPSTREAM: who calls this (default direction)
codegraph code-path <function> --down   # DOWNSTREAM: what this calls
```

Terminal output is always a colored tree; leaf entry points (no caller) are
highlighted. Add `--json` to emit the paths as a plain JSON array of strings
(no color) to stdout.

### Machine-readable output (`--json`)

Every query command (and `code-path`) accepts `--json`, which prints plain JSON
to stdout with no color or decoration — redirect it to a file to store results:

```bash
codegraph find Config --json > config.json
codegraph function parse --json | jq '.[].qualified_name'
```

### Test analysis

```bash
codegraph find-tests                 # all tests
codegraph find-tests <pattern>       # tests matching a name, tested fn, or called fn
codegraph test-coverage              # functions with no test coverage
```

### Export (advanced)

```bash
codegraph csv .                      # build + export nodes.csv / edges.csv (--output-path <dir>)
codegraph ontology                   # dump ontology definitions
codegraph build . --turtle           # build as .ttl instead of .bin (NOTE: queries need .bin)
```

## Output format

```
# function / find
  module::path::name -> ReturnType at ./src/file.rs:188
  Description: <doc summary, when present>

# type
  Struct workspace::PackageConfig at ./src/workspace.rs:54
  Enum  / Trait ... likewise, followed by usage sites
```

Read the `file:line` and jump straight there.

## Task recipes

| Task | Steps |
| --- | --- |
| **Find anything** | `find <name>` → read the returned `file:line` |
| **Debug a function** | `function <name>` → `code-path <name>` (trace callers) → read only files on the path |
| **Refactor a type or fn** (impact) | `impact <name>` — one report: callers + usage sites (incl. struct fields) + trait impls → read only affected files |
| **Understand a call flow** | `entry-points` → `code-path main --down` → follow interesting branches |
| **Learn the API** | `public` → `trait-impls` → `entry-points` |
| **Test audit** | `test-coverage` → `find-tests <critical_fn>` |
| **Explore an unfamiliar file** | `file <name>.rs -d -f -t` before reading it |

## Do NOT

- ❌ `Grep`/`Glob` for a function or type name — use `function` / `type` / `find`.
- ❌ Read a whole module to find a definition or a usage — query for the `file:line`.
- ❌ Read files hoping to discover relationships — use `code-path` (calls) or `type` (usages).
- ❌ Query a `.ttl` graph — queries require `.bin`.

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| "No binary graph file found" | `codegraph build .` |
| "No function/type found matching" | check spelling; patterns match substrings, so try a shorter fragment |
| Results look stale | run `codegraph status` to see which files drifted; rebuild with `codegraph build .` if it reports any |
| "Turtle not supported for queries" | rebuild without `--turtle` to get `.bin` |
