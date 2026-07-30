---

name: codegraph
description: >
Analyze code structures, call graphs, and dependencies.
MUST be used foremost before any general file reading or grep tools.
---

# Termprint API Reference

Rust library for colorized terminal output with tables, data structures, and automatic formatting.

## Feature Flags

- `to-log`: Routes output through `tracing` instead of stdout

## Constants

```rust
LONG=120, MEDIUM=80, SHORT=30, SPACE=3, STD_WIDTH=20, MAX_WIDTH=120, MAX_COL_WIDTH=50
```

## Color Semantics

- Blue: info keys, general text
- Bright Cyan: values, variables
- Bold Blue: titles, lines
- Bright Red Bold: errors
- Bright Yellow: warnings
- Bright Green Bold: success
- Cycling (tables): BrightBlue, BrightCyan, BrightMagenta, BrightGreen, BrightYellow

## API Functions

### Errors & Warnings

```rust
error(message: &str, info: Option<I>, err_msg: Option<E>)  // Full error with context
error_msg(message: K)                                        // Simple error
warning(msg: &str)                                          // Warning message
warn(msg: K, info: V)                                       // Warning with info
```

### Information Display

```rust
info(key: K, value: V)                    // Key: value pairs
info_max(key: K, value: &str, max: usize) // Truncated value
info_and(key: K, value: V)                // No newline, expects info_end()
info_end(value: V)                        // Completes info_and()
message(txt: K)                           // General message
message_and(txt: K) / message_end(txt: K) // Multi-part message
success(txt: K)                           // Success message
item(key: K, value: V)                    // Key-value without colon
```

### Headers & Lines

```rust
title(header: K)                          // Bold title
section(header: K)                        // Title with ═══ borders
line(length: usize)                       // Horizontal line ─
double_line(length: usize)                // Double line ═
```

### Collections

```rust
list(list: &[T], txt: Option<&str>)      // Bulleted list with optional title
vec(vec: &Vec<&str>, title: Option<&str>) // Bullet list from vec
hashmap(data: &HashMap<K,V>, txt: Option<&str>) // Key-value with wrapping
```

### Structured Data

```rust
data_struct<T: Serialize>(obj: &T, title: Option<&str>)
  // Displays struct fields as key-value pairs with auto-wrapping

vec_struct<T: Serialize>(title: &str, vec: &Vec<T>, max_col_width: Option<usize>)
  // Displays vector of structs as table

table(table: &Vec<Vec<S>>, has_header: bool, header_row: Option<Vec<S>>,
      title: Option<&str>, column_width: Option<usize>)
  // Full table with auto column sizing, supports wide tables (splits if needed)

table_basic(tablevec: &Vec<Vec<S>>)
  // Simplified table with defaults

header_row(headers: &Vec<T>, widths: &Vec<usize>)  // Custom table header
row(values: &Vec<T>, widths: &Vec<usize>)         // Custom table row
```

### Special Output

```rust
json(json_str: &str)                     // Pretty-printed colorized JSON
progress_bar(index: u64, total: u64, terminal_width: usize)  // Progress bar
cmd()                                     // Prints command line args
terminal_type()                          // Displays $TERM value
```

### Program Lifecycle

```rust
start_program(name: &str) -> SystemTime  // Banner with timestamp, returns start time
end_program(name: &str, start: SystemTime) -> SystemTime  // Banner with duration
```

## Usage Examples

```rust
// Basic info
termprint::info("Host", "localhost");
termprint::success("Connected");

// Errors
termprint::error("Failed", Some("db"), Some("timeout"));
termprint::warning("Deprecated API");

// Structured data
#[derive(Serialize)]
struct User { name: String, age: u32 }
let user = User { name: "John".into(), age: 30 };
termprint::data_struct(&user, Some("User Info"));

// Tables
let data = vec![
    vec!["Name", "Age"],
    vec!["Alice", "30"],
    vec!["Bob", "25"],
];
termprint::table(&data, true, None, Some("Users"), None);

// Vector of structs
let users = vec![user1, user2, user3];
termprint::vec_struct("All Users", &users, Some(40));

// Progress
for i in 0..=100 {
    termprint::progress_bar(i, 100, 50);
}

// Program timing
let start = termprint::start_program("MyApp");
// ... work ...
termprint::end_program("MyApp", start);
```

## Key Features

- Auto-detects terminal width (default 100, or 80 with `to-log`)
- Auto-wraps long text to fit terminal
- Tables split into segments if too wide
- Truncation marked with `*`
- All functions have `#[cfg]` variants for `to-log` feature
