# Enhanced Features Guide

## Smart Tab Completion

rsqlite3 includes intelligent context-aware tab completion that helps you write SQL queries faster.

### Features

**SQL Keywords**
- Press `Tab` to complete SQL keywords like SELECT, FROM, WHERE, etc.
- Keywords are suggested based on context

**Table Names**
- Tab completion automatically suggests table names from your database
- Completions appear after keywords like `FROM`, `JOIN`, `UPDATE`, `INTO`
- Cache is automatically refreshed when schema changes

**Column Names**
- Column names are suggested after `SELECT`, `WHERE`, and `SET`
- Context-aware completions based on available tables

**Dot Commands**
- All dot commands (`.help`, `.tables`, `.schema`, etc.) are auto-completable
- Just type `.` and press `Tab`

**Dot Command Arguments** (Smart Context-Aware)
- `.schema <Tab>` - Suggests table names from your database
- `.dump <Tab>` - Suggests table names for selective dumping
- `.mode <Tab>` - Suggests output modes (list, csv, column, json, line, table, markdown)
- `.headers <Tab>` - Suggests "on" or "off"
- `.timer <Tab>` - Suggests "on" or "off"
- `.echo <Tab>` - Suggests "on" or "off"
- `.import FILE <Tab>` - Suggests table names for the import target

### Usage Examples

```sql
-- Type "SEL" and press Tab → "SELECT"
SELECT

-- After FROM, press Tab to see all tables
SELECT * FROM <Tab>
users    products    orders

-- After SELECT, press Tab to see columns
SELECT n<Tab>
name    number

-- Dot commands
.<Tab>
.help    .quit    .tables    .schema    .mode

-- Dot command arguments (NEW!)
.schema <Tab>
users    products    orders

.mode <Tab>
list    csv    column    json    line    table    markdown

.headers <Tab>
on    off
```

### How It Works

The completion system:
1. Analyzes the SQL you're typing
2. Detects the context (what keyword came before)
3. Queries the database for relevant tables/columns
4. Filters suggestions based on what you've typed
5. Presents matching completions

## Enhanced Table UI

The table output mode has been significantly enhanced with better formatting and visual appeal.

### Features

#### Beautiful Box Drawing
```
┌────┬───────────────┬───────────────────┬─────┐
│ id │ name          │ email             │ age │
├────┼───────────────┼───────────────────┼─────┤
│ 1  │ Alice Johnson │ alice@example.com │ 28  │
│ 2  │ Bob Smith     │ bob@example.com   │ 35  │
└────┴───────────────┴───────────────────┴─────┘
(2 rows)
```

#### Color Support (with --color flag)
- **Cyan bold headers** - Column names stand out
- **Alternating row colors** - Dimmed gray for even rows improves readability
- **Null value highlighting** - NULL values shown in dark gray italic
- **Green footer** - Row count displayed in muted green

Example with colors:
```bash
rsqlite3 data.db "SELECT * FROM users" --mode table --header --color
```

#### Smart Column Widths
- Automatically calculates optimal column widths
- Maximum column width of 50 characters prevents overflow
- Long values are truncated with ellipsis (...)
- Terminal-aware sizing

#### Row Count Footer
Every table output includes a row count:
```
(42 rows)
```

### Usage

Enable table mode:
```bash
# Command line
rsqlite3 data.db "SELECT * FROM users" --mode table --header

# In REPL
sqlite> .mode table
sqlite> .headers on
sqlite> SELECT * FROM users;
```

Enable colors:
```bash
rsqlite3 data.db "SELECT * FROM users" --mode table --header --color
```

### Comparison with Other Modes

**List Mode** (default):
```
1|Alice|28
2|Bob|35
```

**Column Mode**:
```
id  name   age
--  -----  ---
1   Alice  28
2   Bob    35
```

**Table Mode** (enhanced):
```
┌────┬───────┬─────┐
│ id │ name  │ age │
├────┼───────┼─────┤
│ 1  │ Alice │ 28  │
│ 2  │ Bob   │ 35  │
└────┴───────┴─────┘
(2 rows)
```

**Markdown Mode**:
```
| id | name | age |
| --- | --- | --- |
| 1 | Alice | 28 |
| 2 | Bob | 35 |
```

## REPL Improvements

### History
- Full command history with up/down arrow navigation
- History persists across sessions
- Search history with Ctrl+R

### Multi-line SQL
- Automatic detection of incomplete statements
- Smart prompt changes:
  - `sqlite>` for new statements
  - `   ...>` for continuations

### Keyboard Shortcuts
- `Tab` - Auto-complete
- `Up/Down` - Navigate history
- `Ctrl+R` - Search history
- `Ctrl+C` - Cancel current input
- `Ctrl+D` - Exit (same as `.quit`)
- `Ctrl+L` - Clear screen

## Performance Features

### Caching
- Table and column names are cached for fast completion
- Cache automatically refreshes when:
  - `.schema` command is run
  - `.tables` command is run
  - Database structure changes are detected

### Efficient Rendering
- Terminal width detection prevents line wrapping
- Streaming output for large result sets
- Smart truncation for wide columns

## Color Customization

Colors are automatically detected based on terminal capabilities. You can:

**Enable colors explicitly:**
```bash
rsqlite3 data.db --color
```

**Disable colors explicitly:**
```bash
rsqlite3 data.db --no-color
```

**Auto-detection:**
Colors are enabled by default if:
- Output is to a terminal (not a file or pipe)
- Terminal supports ANSI colors

## Tips and Tricks

### Fast Table Inspection
```bash
# Quick table view with colors
rsqlite3 data.db ".schema users" --color

# Pretty table output
rsqlite3 data.db "SELECT * FROM users LIMIT 10" --mode table --header --color
```

### Export with Style
```bash
# Export to markdown for documentation
rsqlite3 data.db "SELECT * FROM metrics" --mode markdown --header > metrics.md

# Create pretty table for reports
rsqlite3 data.db "SELECT * FROM summary" --mode table --header --color | tee report.txt
```

### Interactive Exploration
```sql
-- In REPL, explore with completion
sqlite> .mode table
sqlite> .headers on
sqlite> SELECT * FROM <Tab>  -- See all tables
sqlite> SELECT n<Tab>        -- Complete column names
```

### NULL Value Handling
```bash
# Show NULL values distinctly
rsqlite3 data.db "SELECT * FROM users" --nullvalue "(null)" --mode table --header

# Or with colors for automatic NULL highlighting
rsqlite3 data.db "SELECT * FROM users" --mode table --header --color
```

## Compatibility Note

All enhanced features are **fully backward compatible** with standard sqlite3:
- Default output modes work identically
- Standard CLI options are preserved
- Dot commands behave the same way
- SQL syntax is unchanged

Enhanced features are **additive only** - they don't break existing workflows.
