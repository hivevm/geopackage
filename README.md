[![Build Geopackage](https://github.com/hivevm/geopackage/actions/workflows/build.yaml/badge.svg)](https://github.com/hivevm/geopackage/actions/workflows/build.yaml)


# Description
A SQLite 3 extension that provides a minimal [OGC GeoPackage](http://www.ogcnetwork.net/geopackage) implementation.

## Features

### Core Functionality
- ✅ Interactive REPL with multi-line SQL support
- ✅ One-shot SQL execution from command line
- ✅ Piped input support for batch processing
- ✅ Full compatibility with standard sqlite3 commands

### Output Formats
- **table** - Beautiful box-drawing table (default, with headers)
- **list** - Pipe-separated values
- **csv** - Comma-separated values with proper quoting
- **column** - Aligned columns
- **json** - JSON array output
- **jsonl** - JSON Lines (one object per line)
- **line** - One value per line
- **markdown** - Markdown table format

### Dot Commands (Tier 1 & 2)

**Essential Commands:**
- `.quit` / `.exit` - Exit the program
- `.help` - Show help message
- `.tables [PATTERN]` - List tables (optionally filtered)
- `.schema [TABLE]` - Show CREATE statements
- `.mode [MODE]` - Set output mode
- `.headers on|off` - Toggle column headers
- `.show` - Show current settings

**File Operations:**
- `.dump [TABLE]` - Generate SQL dump
- `.import FILE TABLE` - Import CSV into table
- `.output [FILE]` - Redirect output to file
- `.read FILE` - Execute SQL from file
- `.open FILE` - Close existing database and reopen FILE

**Database Operations:**
- `.databases` - List attached databases
- `.separator SEP` - Set column separator
- `.nullvalue STRING` - Set NULL display value
- `.width NUM1 NUM2 ...` - Set column widths for "column" mode

**Other:**
- `.timer on|off` - Show query execution time
- `.echo on|off` - Echo commands before executing
- `.bail on|off` - Stop after hitting an error

## Installation

### From Source

```bash
git clone <repository-url>
cd rsqlite3
cargo build --release
```

The binary will be at `target/release/rsqlite3`.

### Usage

Install to your PATH:
```bash
cargo install --path .
```

## Usage

### Interactive Mode

Start the REPL:
```bash
rsqlite3 database.db
```

```
rsqlite3 version 1.0.0
Enter ".help" for usage hints.
Connected to database.db
rsqlite3> CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);
rsqlite3> INSERT INTO users VALUES (1, 'Alice');
rsqlite3> SELECT * FROM users;
1|Alice
rsqlite3> .mode column
rsqlite3> .headers on
rsqlite3> SELECT * FROM users;
id  name
--  -----
1   Alice
rsqlite3> .quit
```

### One-Shot SQL

Execute SQL directly from command line:
```bash
rsqlite3 database.db "SELECT * FROM users"
```

### Piped Input

Process SQL from files or pipes:
```bash
echo "SELECT * FROM users;" | rsqlite3 database.db

# Or from a file
rsqlite3 database.db < script.sql
```

### Command-Line Options

```
Usage: rsqlite3 [OPTIONS] [DATABASE] [SQL]

Arguments:
  [DATABASE]  Path to the SQLite database file
  [SQL]       SQL command to execute

Options:
  -H, --header              Show column headers
      --noheader            Do not show column headers
  -m, --mode <MODE>         Set output mode (list, csv, column, json, jsonl, line, table, markdown)
  -s, --separator <SEP>     Set column separator for list mode
  -n, --nullvalue <TEXT>    Set NULL value display string
  -r, --readonly            Open database in read-only mode
      --init <FILE>         Execute SQL from init file before processing
      --cmd <COMMAND>       Run command before reading stdin
  -e, --echo                Echo commands before executing
  -b, --bail                Stop after hitting an error
      --color               Enable color output
      --no-color            Disable color output
  -h, --help                Print help
  -V, --version             Print version
```

### Examples

**Column mode with headers:**
```bash
rsqlite3 data.db "SELECT * FROM users" --mode column --header
```

**JSON output:**
```bash
rsqlite3 data.db "SELECT * FROM products" --mode json
```

**JSON Lines output:**
```bash
rsqlite3 data.db "SELECT * FROM products" --mode jsonl
```

**Table mode (pretty printing with colors):**
```bash
rsqlite3 data.db "SELECT * FROM orders" --mode table --header --color
```

Output:
```
┌────┬───────────┬────────┐
│ id │ product   │ amount │
├────┼───────────┼────────┤
│ 1  │ Laptop    │ 999.99 │
│ 2  │ Mouse     │ 29.99  │
└────┴───────────┴────────┘
(2 rows)
```

**CSV export to file:**
```bash
rsqlite3 data.db "SELECT * FROM users" --mode csv > users.csv
```

**Import CSV:**
```bash
rsqlite3 data.db ".import users.csv users"
```

**Dump database:**
```bash
rsqlite3 data.db ".dump" > backup.sql
```

## Enhanced Features ✨

### Smart Tab Completion
- **Context-aware completion** - SQL keywords, table names, and column names
- **Dot command completion** - All dot commands are auto-completable
- **Smart dot command arguments** - Intelligent suggestions for command parameters
  - `.schema <Tab>` → suggests table names
  - `.mode <Tab>` → suggests output modes
  - `.headers <Tab>` → suggests on/off
- **Automatic cache refresh** - Database schema is cached and auto-updated
- **Multi-context support** - Different suggestions based on SQL context

```sql
SELECT * FROM <Tab>     -- Shows all tables
SELECT n<Tab>           -- Completes column names
.<Tab>                  -- Shows all dot commands
.schema <Tab>           -- Shows table names
.mode <Tab>             -- Shows output modes (csv, json, table, etc.)
```

### Enhanced Table UI
- **Beautiful box-drawing tables** - Clean, professional output
- **Color support** - Cyan headers, alternating rows, NULL highlighting
- **Smart column widths** - Terminal-aware with automatic truncation
- **Row count footer** - Always shows total rows returned

```bash
# Beautiful colored tables
rsqlite3 data.db "SELECT * FROM users" --mode table --header --color
```

For detailed information about these features, see [FEATURES.md](FEATURES.md).

## Compatibility

rsqlite3 aims to be a drop-in replacement for the standard sqlite3 CLI. Most common workflows should work identically.

### Differences from sqlite3

- **Default output mode**: Table mode with headers (sqlite3 uses list mode without headers)
- Enhanced output formatting (beautiful tables, markdown mode)
- Optional color output (with `--color` flag)
- Auto-detected color support
- Written in Rust for improved safety and maintainability

## Development

### Project Structure

```
src/
├── main.rs           - CLI entry point and argument parsing
├── repl.rs           - REPL loop and interactive shell
├── cli_state.rs      - Session state and configuration
├── db.rs             - Database operations
├── sql_executor.rs   - SQL execution with formatting
├── output.rs         - Output formatting system
├── dot_commands.rs   - Dot command handlers
├── import_export.rs  - Import/export functionality
├── completion.rs     - Tab completion logic
├── lsp.rs            - Language server protocol features (internal)
└── sql_highlight.rs  - Syntax highlighting
```

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.


# Dependencies

- libgpkg requires SQLite 3.51.0 or higher.
