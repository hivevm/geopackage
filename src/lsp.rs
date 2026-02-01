//! In-process LSP server for SQLite SQL queries.
//!
//! This module provides language server features (completion, hover, diagnostics,
//! goto definition) for SQLite SQL queries. Since it runs in-process, it uses
//! direct function calls instead of JSON-RPC/IPC communication.
//!
//! # Example
//!
//! ```no_run
//! use rsqlite3::lsp::{SqlLspService, Position};
//! use rusqlite::Connection;
//!
//! let conn = Connection::open("test.db").unwrap();
//! let mut lsp = SqlLspService::new();
//! lsp.refresh_schema(&conn).unwrap();
//!
//! // Get completions
//! let completions = lsp.completion("SELECT * FROM u", Position { line: 0, character: 15 });
//!
//! // Get hover info
//! let hover = lsp.hover("SELECT id FROM users", Position { line: 0, character: 16 });
//! ```

use std::collections::HashMap;

use rusqlite::Connection;
use sqlparser::dialect::SQLiteDialect;
use sqlparser::tokenizer::{Token, Tokenizer};

// ============================================================================
// LSP Types
// ============================================================================

/// Position in a text document (0-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Line number (0-indexed).
    pub line: u32,
    /// Character offset within the line (0-indexed).
    pub character: u32,
}

impl Position {
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

/// A range in a text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Start position (inclusive).
    pub start: Position,
    /// End position (exclusive).
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Create a range covering a single line from start to end character.
    pub fn on_line(line: u32, start_char: u32, end_char: u32) -> Self {
        Self {
            start: Position::new(line, start_char),
            end: Position::new(line, end_char),
        }
    }
}

/// The kind of a completion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    Table,
    Column,
    Function,
    Type,
    Keyword,
}

/// A completion item represents a text suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// The label of this completion item (displayed to the user).
    pub label: String,
    /// The kind of this completion item.
    pub kind: CompletionItemKind,
    /// A human-readable string with additional information (e.g., type info).
    pub detail: Option<String>,
    /// Extended documentation for this item.
    pub documentation: Option<String>,
    /// Text to insert when this completion is selected (if different from label).
    pub insert_text: Option<String>,
}

impl CompletionItem {
    pub fn type_(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: CompletionItemKind::Type,
            detail: Some("type".to_string()),
            documentation: None,
            insert_text: None,
        }
    }
    pub fn keyword(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: CompletionItemKind::Keyword,
            detail: None,
            documentation: None,
            insert_text: None,
        }
    }

    pub fn table(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: CompletionItemKind::Table,
            detail: Some("table".to_string()),
            documentation: None,
            insert_text: None,
        }
    }

    pub fn column(label: impl Into<String>, table: &str, type_: &str) -> Self {
        Self {
            label: label.into(),
            kind: CompletionItemKind::Column,
            detail: Some(format!("{} ({})", type_, table)),
            documentation: None,
            insert_text: None,
        }
    }

    pub fn function(label: impl Into<String>, signature: Option<&str>) -> Self {
        Self {
            label: label.into(),
            kind: CompletionItemKind::Function,
            detail: signature.map(|s| s.to_string()),
            documentation: None,
            insert_text: None,
        }
    }
}

/// Result of a hover request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverResult {
    /// The hover contents (markdown or plain text).
    pub contents: String,
    /// An optional range to highlight.
    pub range: Option<Range>,
}

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// A diagnostic message (error, warning, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The range where the diagnostic applies.
    pub range: Range,
    /// The severity of the diagnostic.
    pub severity: DiagnosticSeverity,
    /// The diagnostic message.
    pub message: String,
}

/// A symbol location for goto definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolLocation {
    /// The table name where the symbol is defined.
    pub table_name: String,
    /// The column name (if the symbol is a column).
    pub column_name: Option<String>,
    /// The CREATE statement for the table.
    pub create_sql: Option<String>,
}

// ============================================================================
// Internal Types
// ============================================================================

/// Cached column information.
#[derive(Debug, Clone)]
struct ColumnInfo {
    table: String,
    name: String,
    type_: String,
    is_pk: bool,
    is_nullable: bool,
    default_value: Option<String>,
}

/// SQL context for completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlContext {
    /// After FROM, JOIN, UPDATE, INTO, TABLE - suggest tables
    TableContext,
    /// After SELECT, WHERE, SET - suggest columns
    ColumnContext,
    /// After INSERT - suggest INTO
    InsertContext,
    /// Expecting a type (e.g. after column name in CREATE TABLE)
    TypeContext,
    /// After DROP INDEX - suggest index names
    IndexContext,
    /// Default context - suggest keywords and tables
    Default,
}

// ============================================================================
// SqlLspService
// ============================================================================

/// In-process LSP service for SQLite SQL queries.
///
/// This service provides language server features without requiring
/// inter-process communication. Call methods directly to get results.
pub struct SqlLspService {
    cached_tables: Vec<String>,
    cached_columns: Vec<ColumnInfo>,
    cached_create_sqls: Vec<(String, String)>, // (table, create_sql)
    cached_indexes: Vec<String>,
}

impl SqlLspService {
    /// Create a new LSP service with empty caches.
    pub fn new() -> Self {
        Self {
            cached_tables: Vec::new(),
            cached_columns: Vec::new(),
            cached_create_sqls: Vec::new(),
            cached_indexes: Vec::new(),
        }
    }

    /// Create an LSP service with pre-populated caches (useful for testing).
    #[cfg(test)]
    pub fn with_cache(
        tables: Vec<String>,
        columns: Vec<(String, String, String, bool, bool)>, // (table, name, type, pk, nullable)
    ) -> Self {
        Self::with_cache_and_indexes(tables, columns, Vec::new())
    }

    /// Create an LSP service with pre-populated caches including indexes (useful for testing).
    #[cfg(test)]
    pub fn with_cache_and_indexes(
        tables: Vec<String>,
        columns: Vec<(String, String, String, bool, bool)>, // (table, name, type, pk, nullable)
        indexes: Vec<String>,
    ) -> Self {
        let cached_columns = columns
            .into_iter()
            .map(|(table, name, type_, is_pk, is_nullable)| ColumnInfo {
                table,
                name,
                type_,
                is_pk,
                is_nullable,
                default_value: None,
            })
            .collect();

        Self {
            cached_tables: tables,
            cached_columns,
            cached_create_sqls: Vec::new(),
            cached_indexes: indexes,
        }
    }

    /// Refresh table and column caches from the database.
    pub fn refresh_schema(&mut self, conn: &Connection) -> rusqlite::Result<()> {
        // Get all tables
        self.cached_tables = crate::db::get_tables(conn)?;

        // Get columns for each table
        self.cached_columns.clear();
        self.cached_create_sqls.clear();

        for table in &self.cached_tables {
            if let Ok(schema) = crate::db::get_schema(conn, table) {
                for col in schema.columns {
                    self.cached_columns.push(ColumnInfo {
                        table: table.clone(),
                        name: col.name,
                        type_: col.type_,
                        is_pk: col.pk,
                        is_nullable: !col.notnull,
                        default_value: col.dflt_value,
                    });
                }
                if !schema.create_sql.is_empty() {
                    self.cached_create_sqls
                        .push((table.clone(), schema.create_sql));
                }
            }
        }

        // Get all indexes
        self.cached_indexes.clear();
        let mut stmt =
            conn.prepare("SELECT name FROM sqlite_master WHERE type='index' ORDER BY name")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for name in rows.flatten() {
            // Skip auto-generated indexes (sqlite_autoindex_*)
            if !name.starts_with("sqlite_autoindex_") {
                self.cached_indexes.push(name);
            }
        }

        Ok(())
    }

    /// Get SQL keywords.
    fn get_sql_keywords() -> Vec<&'static str> {
        vec![
            "SELECT",
            "FROM",
            "WHERE",
            "INSERT",
            "INTO",
            "VALUES",
            "UPDATE",
            "SET",
            "DELETE",
            "CREATE",
            "TABLE",
            "DROP",
            "ALTER",
            "INDEX",
            "ON",
            "PRIMARY",
            "KEY",
            "FOREIGN",
            "REFERENCES",
            "UNIQUE",
            "NOT",
            "NULL",
            "DEFAULT",
            "CHECK",
            "AS",
            "JOIN",
            "INNER",
            "LEFT",
            "RIGHT",
            "OUTER",
            "CROSS",
            "GROUP",
            "BY",
            "HAVING",
            "ORDER",
            "LIMIT",
            "OFFSET",
            "UNION",
            "ALL",
            "DISTINCT",
            "AND",
            "OR",
            "IN",
            "BETWEEN",
            "LIKE",
            "GLOB",
            "CASE",
            "WHEN",
            "THEN",
            "ELSE",
            "END",
            "EXISTS",
            "PRAGMA",
            "BEGIN",
            "COMMIT",
            "ROLLBACK",
            "TRANSACTION",
            "SAVEPOINT",
            "RELEASE",
            "ATTACH",
            "DETACH",
            "DATABASE",
            "TEMPORARY",
            "TEMP",
            "VIEW",
            "TRIGGER",
            "IF",
            "AUTOINCREMENT",
            "EXPLAIN",
            "ASC",
            "DESC",
            "COLLATE",
            "NOCASE",
            "BINARY",
            "RTRIM",
            "ESCAPE",
            "ISNULL",
            "NOTNULL",
            "TRUE",
            "FALSE",
        ]
    }

    /// Get SQL functions.
    fn get_sql_functions() -> Vec<(&'static str, &'static str)> {
        vec![
            ("COUNT", "COUNT(expr) - Count rows"),
            ("SUM", "SUM(expr) - Sum of values"),
            ("AVG", "AVG(expr) - Average of values"),
            ("MIN", "MIN(expr) - Minimum value"),
            ("MAX", "MAX(expr) - Maximum value"),
            ("ABS", "ABS(x) - Absolute value"),
            ("COALESCE", "COALESCE(x, y, ...) - First non-null value"),
            ("IFNULL", "IFNULL(x, y) - y if x is null"),
            ("NULLIF", "NULLIF(x, y) - null if x equals y"),
            ("LENGTH", "LENGTH(str) - String length"),
            ("SUBSTR", "SUBSTR(str, start, len) - Substring"),
            ("UPPER", "UPPER(str) - Uppercase"),
            ("LOWER", "LOWER(str) - Lowercase"),
            ("TRIM", "TRIM(str) - Remove whitespace"),
            ("LTRIM", "LTRIM(str) - Remove leading whitespace"),
            ("RTRIM", "RTRIM(str) - Remove trailing whitespace"),
            ("REPLACE", "REPLACE(str, from, to) - Replace substring"),
            ("INSTR", "INSTR(str, substr) - Find substring position"),
            ("PRINTF", "PRINTF(format, ...) - Formatted string"),
            ("TYPEOF", "TYPEOF(expr) - Type of value"),
            ("ROUND", "ROUND(x, digits) - Round number"),
            ("RANDOM", "RANDOM() - Random integer"),
            ("DATETIME", "DATETIME(time, ...) - Date/time"),
            ("DATE", "DATE(time, ...) - Date"),
            ("TIME", "TIME(time, ...) - Time"),
            ("STRFTIME", "STRFTIME(format, time) - Format date/time"),
            ("JULIANDAY", "JULIANDAY(time) - Julian day number"),
            ("HEX", "HEX(blob) - Hexadecimal encoding"),
            ("QUOTE", "QUOTE(value) - SQL literal"),
            ("CAST", "CAST(expr AS type) - Type conversion"),
            ("GLOB", "GLOB(pattern, str) - Unix glob matching"),
            ("LIKE", "LIKE(pattern, str) - SQL pattern matching"),
            (
                "GROUP_CONCAT",
                "GROUP_CONCAT(expr, sep) - Concatenate group",
            ),
            ("TOTAL", "TOTAL(expr) - Sum as float"),
            ("JSON", "JSON(value) - Parse JSON"),
            (
                "JSON_EXTRACT",
                "JSON_EXTRACT(json, path) - Extract from JSON",
            ),
            ("JSON_ARRAY", "JSON_ARRAY(...) - Create JSON array"),
            ("JSON_OBJECT", "JSON_OBJECT(...) - Create JSON object"),
        ]
    }

    /// Get SQL types.
    fn get_sql_types() -> Vec<&'static str> {
        vec![
            "TEXT", "INTEGER", "REAL", "BLOB", "NUMERIC", "VARCHAR", "CHAR", "BOOLEAN", "DATETIME",
            "DATE", "TIME", "FLOAT", "DOUBLE", "INT", "BIGINT", "SMALLINT", "TINYINT",
        ]
    }

    /// Detect SQL context based on text before cursor.
    fn detect_context(&self, text: &str, offset: usize) -> SqlContext {
        let before_cursor = &text[..offset.min(text.len())];

        // Tokenize the input
        let dialect = SQLiteDialect {};
        let mut tokenizer = Tokenizer::new(&dialect, before_cursor);
        // We want to process incomplete SQL, so we just try to get whatever tokens we can
        let tokens = tokenizer.tokenize().unwrap_or_default();

        if tokens.is_empty() {
            return SqlContext::Default;
        }

        // Helper to get previous non-whitespace token
        let get_prev_token = |idx: usize| -> Option<&Token> {
            let mut i = idx;
            while i > 0 {
                i -= 1;
                let token = &tokens[i];
                if !matches!(token, Token::Whitespace(_)) {
                    return Some(token);
                }
            }
            None
        };

        let last_idx = tokens.len();
        let last_token = get_prev_token(last_idx);

        // Helper to check if we're in CREATE INDEX context
        let is_create_index_context = || -> bool {
            let mut found_index = false;
            for token in tokens.iter().rev() {
                if let Token::Word(w) = token {
                    let kw = w.value.to_uppercase();
                    if kw == "INDEX" {
                        found_index = true;
                    } else if kw == "CREATE" && found_index {
                        return true;
                    } else if kw == "JOIN" || kw == "FROM" || kw == "SELECT" {
                        // We hit a different clause, not CREATE INDEX
                        return false;
                    }
                }
            }
            false
        };

        // Helper to check if we're in INSERT INTO column list context
        // Pattern: INSERT INTO tablename (column_list)
        let is_insert_into_column_list_context = || -> bool {
            let mut paren_depth = 0;
            let mut found_lparen = false;
            let mut found_into = false;

            for token in tokens.iter().rev() {
                match token {
                    Token::RParen => {
                        paren_depth += 1;
                    }
                    Token::LParen => {
                        if paren_depth > 0 {
                            paren_depth -= 1;
                        } else {
                            found_lparen = true;
                        }
                    }
                    Token::Word(w) if found_lparen => {
                        let kw = w.value.to_uppercase();
                        if kw == "INTO" {
                            found_into = true;
                        } else if kw == "INSERT" && found_into {
                            return true;
                        } else if kw == "VALUES" || kw == "SELECT" || kw == "FROM" {
                            // VALUES ( ... ) is not column list context
                            return false;
                        }
                    }
                    Token::Whitespace(_) => continue,
                    _ => {}
                }
            }
            false
        };

        // Helper to check if we're in DROP INDEX context
        // Pattern: DROP INDEX [IF EXISTS] index_name
        let is_drop_index_context = || -> bool {
            let mut found_index = false;
            for token in tokens.iter().rev() {
                if let Token::Word(w) = token {
                    let kw = w.value.to_uppercase();
                    if kw == "INDEX" {
                        found_index = true;
                    } else if kw == "DROP" && found_index {
                        return true;
                    } else if kw == "IF" || kw == "EXISTS" {
                        // These are allowed between DROP INDEX and index name
                        continue;
                    } else if kw == "CREATE" || kw == "FROM" || kw == "SELECT" {
                        return false;
                    }
                }
            }
            false
        };

        if let Some(token) = last_token {
            match token {
                Token::Word(w) => {
                    let kw = w.value.to_uppercase();
                    match kw.as_str() {
                        "FROM" | "JOIN" | "UPDATE" | "INTO" | "TABLE" => {
                            return SqlContext::TableContext
                        }
                        "ON" => {
                            // "ON" can mean different things:
                            // - CREATE INDEX ... ON table -> TableContext
                            // - JOIN ... ON condition -> ColumnContext
                            if is_create_index_context() {
                                return SqlContext::TableContext;
                            }
                            return SqlContext::ColumnContext;
                        }
                        "INDEX" => {
                            // "INDEX" after DROP -> suggest index names
                            if is_drop_index_context() {
                                return SqlContext::IndexContext;
                            }
                        }
                        "EXISTS" => {
                            // "EXISTS" after DROP INDEX IF -> suggest index names
                            if is_drop_index_context() {
                                return SqlContext::IndexContext;
                            }
                        }
                        "SELECT" | "WHERE" | "SET" | "BY" | "HAVING" => {
                            return SqlContext::ColumnContext
                        }
                        "INSERT" => return SqlContext::InsertContext,
                        "AND" | "OR" => return SqlContext::ColumnContext,
                        _ => {
                            // Check if we're typing an index name after DROP INDEX
                            if is_drop_index_context() {
                                return SqlContext::IndexContext;
                            }
                        }
                    }
                }
                Token::Comma => {
                    // Check if we're inside CREATE INDEX column list
                    if is_create_index_context() {
                        return SqlContext::ColumnContext;
                    }

                    // Check if we're inside INSERT INTO column list
                    if is_insert_into_column_list_context() {
                        return SqlContext::ColumnContext;
                    }

                    // Check what we are in a list of
                    // Scan backwards to find the start of the clause
                    let mut i = last_idx - 1; // Start before the comma (which is at last_idx-1 effectively if ignored whitespace, wait logic)
                                              // Actually get_prev_token(last_idx) returns the last meaningful token.
                                              // If that token is a comma, we want to see what came before.

                    // Simple heuristic: search backwards for keywords
                    while i > 0 {
                        i -= 1;
                        match &tokens[i] {
                            Token::Word(w) => {
                                let kw = w.value.to_uppercase();
                                match kw.as_str() {
                                    "SELECT" | "WHERE" | "GROUP" | "ORDER" => {
                                        return SqlContext::ColumnContext
                                    }
                                    "FROM" | "UPDATE" => return SqlContext::TableContext,
                                    "VALUES" => return SqlContext::Default, // Values list
                                    _ => {}
                                }
                            }
                            Token::LParen => {
                                // Might be CREATE TABLE (..
                                // Complex parenthesis context is handled in the full reverse scan below
                            }
                            _ => {}
                        }
                    }
                }
                Token::LParen => {
                    // Check if we are in CREATE INDEX context
                    // Pattern: CREATE INDEX ... ON tablename (
                    if is_create_index_context() {
                        return SqlContext::ColumnContext;
                    }
                    // Check if we are in INSERT INTO column list context
                    // Pattern: INSERT INTO tablename (
                    if is_insert_into_column_list_context() {
                        return SqlContext::ColumnContext;
                    }
                    // Check if we are in CREATE TABLE
                    // Backwards: LParen -> TableName -> TABLE -> CREATE
                    // Or: LParen -> VALUES -> ...
                    // Or: FunctionName -> LParen
                    // Note: Complex parenthesis context detection is handled
                    // in the full reverse scan below
                }
                _ => {}
            }
        }

        // Full reverse scan state machine
        // We only scan a limited distance back to avoid matching keywords from much earlier clauses
        let mut tokens_rev = tokens
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, t)| !matches!(t, Token::Whitespace(_)));

        // Track how many tokens we've scanned to limit context search depth
        let mut scan_count = 0;
        const MAX_SCAN_DEPTH: usize = 10; // Don't look back more than ~10 meaningful tokens

        // Track parenthesis depth to detect when we're inside parens
        let mut paren_depth = 0;

        while let Some((idx, last)) = tokens_rev.next() {
            scan_count += 1;
            if scan_count > MAX_SCAN_DEPTH {
                break;
            }

            match last {
                Token::RParen => {
                    paren_depth += 1;
                }
                Token::LParen => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    } else {
                        // We're at an unmatched opening paren
                        // Check if this is CREATE INDEX context
                        if is_create_index_context() {
                            return SqlContext::ColumnContext;
                        }
                        // Check if this is INSERT INTO column list context
                        if is_insert_into_column_list_context() {
                            return SqlContext::ColumnContext;
                        }
                    }
                }
                Token::Word(w) => {
                    let kw = w.value.to_uppercase();
                    match kw.as_str() {
                        "FROM" | "JOIN" | "UPDATE" | "INTO" | "TABLE" => {
                            return SqlContext::TableContext
                        }
                        "ON" => {
                            // "ON" can mean different things:
                            // - CREATE INDEX ... ON table -> TableContext (only when not inside parens)
                            // - CREATE INDEX ... ON table (col...) -> ColumnContext (inside parens)
                            // - JOIN ... ON condition -> ColumnContext
                            if is_create_index_context() {
                                // If we're inside parentheses, we want column context
                                if paren_depth > 0 {
                                    return SqlContext::ColumnContext;
                                }
                                return SqlContext::TableContext;
                            }
                            return SqlContext::ColumnContext;
                        }
                        "SELECT" | "WHERE" | "SET" | "BY" | "HAVING" => {
                            return SqlContext::ColumnContext
                        }
                        "AND" | "OR" => return SqlContext::ColumnContext,
                        "CREATE" => return SqlContext::Default,
                        // After LIMIT (and its argument), we should be in Default context
                        // to allow suggesting OFFSET, UNION, etc.
                        "LIMIT" | "OFFSET" | "UNION" | "EXCEPT" | "INTERSECT" => {
                            return SqlContext::Default
                        }
                        _ => {
                            // Check if we are inside CREATE TABLE for column type suggestions
                            if self.is_inside_create_table(&tokens, idx) {
                                return SqlContext::TypeContext;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        SqlContext::Default
    }

    // Helper to check if we are inside a CREATE TABLE statement
    fn is_inside_create_table(&self, tokens: &[Token], current_idx: usize) -> bool {
        let mut last_create_table_idx = None;
        for i in 0..current_idx {
            if let Token::Word(w) = &tokens[i] {
                if w.value.eq_ignore_ascii_case("CREATE") {
                    // Check if next non-whitespace is TABLE
                    let mut j = i + 1;
                    while j < current_idx {
                        match &tokens[j] {
                            Token::Whitespace(_) => {
                                j += 1;
                                continue;
                            }
                            Token::Word(w2) => {
                                if w2.value.eq_ignore_ascii_case("TABLE") {
                                    last_create_table_idx = Some(j); // index of TABLE
                                }
                                break;
                            }
                            _ => break,
                        }
                    }
                }
            }
        }

        if let Some(start_idx) = last_create_table_idx {
            let mut depth = 0;
            let mut open_found = false;
            let end_check = std::cmp::min(current_idx + 1, tokens.len());
            for t in tokens.iter().take(end_check).skip(start_idx + 1) {
                match t {
                    Token::LParen => {
                        depth += 1;
                        open_found = true;
                    }
                    Token::RParen => {
                        if depth > 0 {
                            depth -= 1;
                        }
                    }
                    _ => {}
                }
            }
            return open_found && depth == 1;
        }

        false
    }

    /// Get the word at the given offset.
    fn get_word_at_offset<'a>(&self, text: &'a str, offset: usize) -> (usize, &'a str) {
        let bytes = text.as_bytes();
        let offset = offset.min(text.len());

        // Find word start
        let mut start = offset;
        while start > 0 {
            let ch = bytes[start - 1] as char;
            if ch.is_alphanumeric() || ch == '_' {
                start -= 1;
            } else {
                break;
            }
        }

        // Find word end
        let mut end = offset;
        while end < bytes.len() {
            let ch = bytes[end] as char;
            if ch.is_alphanumeric() || ch == '_' {
                end += 1;
            } else {
                break;
            }
        }

        (start, &text[start..end])
    }

    /// Convert a position to a byte offset in the text.
    fn position_to_offset(&self, text: &str, pos: Position) -> usize {
        let mut offset = 0;
        for (line_num, line) in text.lines().enumerate() {
            if line_num == pos.line as usize {
                return offset + (pos.character as usize).min(line.len());
            }
            offset += line.len() + 1; // +1 for newline
        }
        offset.min(text.len())
    }

    /// Extract tables and their aliases from the SQL text.
    /// Returns a map of alias -> table_name.
    /// Also includes table_name -> table_name.
    fn extract_tables_aliases(&self, text: &str) -> HashMap<String, String> {
        let mut aliases = HashMap::new();
        let dialect = SQLiteDialect {};
        let mut tokenizer = Tokenizer::new(&dialect, text);
        let tokens = tokenizer.tokenize().unwrap_or_default();

        // Helper to check if we're in CREATE INDEX context at position i
        let is_create_index_at = |pos: usize| -> bool {
            // Look backwards from pos to find CREATE INDEX pattern
            let mut found_index = false;
            for j in (0..pos).rev() {
                if let Token::Word(w) = &tokens[j] {
                    let kw = w.value.to_uppercase();
                    if kw == "INDEX" {
                        found_index = true;
                    } else if kw == "CREATE" && found_index {
                        return true;
                    } else if matches!(kw.as_str(), "FROM" | "JOIN" | "SELECT" | "WHERE") {
                        return false;
                    }
                }
            }
            false
        };

        let mut i = 0;
        while i < tokens.len() {
            if let Token::Word(w) = &tokens[i] {
                let kw = w.value.to_uppercase();
                // Handle CREATE INDEX ... ON tablename
                if kw == "ON" && is_create_index_at(i) {
                    // Skip whitespace to find table name
                    let mut j = i + 1;
                    while j < tokens.len() && matches!(tokens[j], Token::Whitespace(_)) {
                        j += 1;
                    }
                    if j < tokens.len() {
                        if let Token::Word(table_w) = &tokens[j] {
                            let table_name = table_w.value.clone();
                            aliases.insert(table_name.clone(), table_name);
                        }
                    }
                }
                // Handle INSERT INTO tablename
                else if kw == "INTO" {
                    // Check if previous non-whitespace token is INSERT
                    let mut is_insert = false;
                    for j in (0..i).rev() {
                        match &tokens[j] {
                            Token::Whitespace(_) => continue,
                            Token::Word(w2) if w2.value.eq_ignore_ascii_case("INSERT") => {
                                is_insert = true;
                                break;
                            }
                            _ => break,
                        }
                    }
                    if is_insert {
                        // Skip whitespace to find table name
                        let mut j = i + 1;
                        while j < tokens.len() && matches!(tokens[j], Token::Whitespace(_)) {
                            j += 1;
                        }
                        if j < tokens.len() {
                            if let Token::Word(table_w) = &tokens[j] {
                                let table_name = table_w.value.clone();
                                aliases.insert(table_name.clone(), table_name);
                            }
                        }
                    }
                }
                // Handle FROM and JOIN clauses
                else if kw == "FROM" || kw == "JOIN" {
                    // Skip whitespace to find table name
                    let mut j = i + 1;
                    while j < tokens.len() && matches!(tokens[j], Token::Whitespace(_)) {
                        j += 1;
                    }

                    if j < tokens.len() {
                        if let Token::Word(table_w) = &tokens[j] {
                            let table_name = table_w.value.clone();

                            aliases.insert(table_name.clone(), table_name.clone());

                            // Check for alias
                            // Skip whitespace
                            let mut k = j + 1;
                            while k < tokens.len() && matches!(tokens[k], Token::Whitespace(_)) {
                                k += 1;
                            }

                            let mut alias = None;
                            if k < tokens.len() {
                                if let Token::Word(w2) = &tokens[k] {
                                    if w2.value.to_uppercase() == "AS" {
                                        // Skip whitespace
                                        let mut l = k + 1;
                                        while l < tokens.len()
                                            && matches!(tokens[l], Token::Whitespace(_))
                                        {
                                            l += 1;
                                        }
                                        if l < tokens.len() {
                                            if let Token::Word(w3) = &tokens[l] {
                                                alias = Some(w3.value.clone());
                                            }
                                        }
                                    } else {
                                        // Implicit alias?
                                        // Exclude keywords like WHERE, JOIN, ON, ORDER, GROUP, LIMIT
                                        let next_kw = w2.value.to_uppercase();
                                        if ![
                                            "WHERE", "JOIN", "INNER", "LEFT", "RIGHT", "FULL",
                                            "CROSS", "ON", "ORDER", "GROUP", "LIMIT", "HAVING",
                                            "SET", "ASC", "DESC", "AND", "OR",
                                        ]
                                        .contains(&next_kw.as_str())
                                        {
                                            alias = Some(w2.value.clone());
                                        }
                                    }
                                }
                            }

                            if let Some(a) = alias {
                                aliases.insert(a, table_name);
                            }
                        }
                    }
                }
            }
            i += 1;
        }

        aliases
    }

    /// Get completions at the given position.
    pub fn completion(&self, text: &str, pos: Position) -> Vec<CompletionItem> {
        let offset = self.position_to_offset(text, pos);
        let (word_start, prefix) = self.get_word_at_offset(text, offset);
        let prefix_lower = prefix.to_lowercase();

        // Check for dot completion
        let mut qualifier: Option<String> = None;
        if word_start > 0 {
            let bytes = text.as_bytes();
            // Check if character before word is a dot
            let mut check_idx = word_start - 1;
            // Skip potential whitespace between dot and word (e.g. "table . column") - though unusual for SQL completion usually
            while check_idx > 0 && bytes[check_idx].is_ascii_whitespace() {
                check_idx -= 1;
            }

            if bytes[check_idx] == b'.' {
                // Get the word before the dot
                let (_, q_word) = self.get_word_at_offset(text, check_idx);
                if !q_word.is_empty() {
                    qualifier = Some(q_word.to_string());
                }
            }
        }

        // If we have a qualifier, we prioritize looking up that table/alias
        if let Some(qual_name) = qualifier {
            let aliases = self.extract_tables_aliases(text);

            // Resolve alias to table name
            // If strictly resolving, we only look for the table.
            // If the qualifier matches a table name directly, use it.
            // If it matches an alias, use the mapped table.
            let table_name = aliases
                .get(&qual_name)
                .cloned()
                .or_else(|| {
                    // Also check case-insensitively against aliases
                    aliases
                        .iter()
                        .find(|(k, _)| k.to_lowercase() == qual_name.to_lowercase())
                        .map(|(_, v)| v.clone())
                })
                .or_else(|| {
                    // If not found in aliases scan, maybe it IS a table name that is cached but not in FROM clause (less likely valid SQL but good for completion)
                    self.cached_tables
                        .iter()
                        .find(|t| t.to_lowercase() == qual_name.to_lowercase())
                        .cloned()
                });

            if let Some(table) = table_name {
                let mut items = Vec::new();
                // Suggest columns for this table
                for col in &self.cached_columns {
                    if col.table == table && col.name.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::column(&col.name, &col.table, &col.type_));
                    }
                }
                return items;
            } else {
                // Unknown qualifier - return empty list rather than falling through
                // This is a dot completion with an unresolved table/alias
                return Vec::new();
            }
        }

        let context = self.detect_context(text, offset);

        let mut items = Vec::new();

        match context {
            SqlContext::TableContext => {
                // Suggest tables
                for table in &self.cached_tables {
                    if table.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::table(table.clone()));
                    }
                }

                // Suggest keywords that follow a table name
                // Common after FROM/JOIN: WHERE, JOIN, ON, GROUP, ORDER, LIMIT, HAVING
                // Common after UPDATE: SET
                // Common after INTO: VALUES, SELECT
                for kw in [
                    "WHERE", "JOIN", "ON", "GROUP", "ORDER", "LIMIT", "HAVING", "INNER", "LEFT",
                    "RIGHT", "OUTER", "CROSS", "AS", "SET", "VALUES", "SELECT",
                ] {
                    if kw.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::keyword(kw));
                    }
                }
            }
            SqlContext::InsertContext => {
                // Suggest INTO
                if "into".starts_with(&prefix_lower) {
                    items.push(CompletionItem::keyword("INTO"));
                }
            }
            SqlContext::ColumnContext => {
                // Extract aliases to prioritize columns from tables in context
                let aliases = self.extract_tables_aliases(text);
                let relevant_tables: Vec<String> = aliases.values().cloned().collect();

                // Suggest columns (deduplicated by name)
                let mut seen_columns = std::collections::HashSet::new();

                // First pass: columns from relevant tables (in FROM/JOIN)
                for col in &self.cached_columns {
                    if !relevant_tables.is_empty() && !relevant_tables.contains(&col.table) {
                        continue;
                    }

                    if col.name.to_lowercase().starts_with(&prefix_lower)
                        && !seen_columns.contains(&col.name)
                    {
                        seen_columns.insert(col.name.clone());
                        items.push(CompletionItem::column(&col.name, &col.table, &col.type_));
                    }
                }

                // Second pass: if we have few results or no relevant tables found, suggest all
                if items.is_empty() || relevant_tables.is_empty() {
                    for col in &self.cached_columns {
                        // Skip if already added
                        if seen_columns.contains(&col.name) {
                            continue;
                        }

                        if col.name.to_lowercase().starts_with(&prefix_lower) {
                            seen_columns.insert(col.name.clone());
                            items.push(CompletionItem::column(&col.name, &col.table, &col.type_));
                        }
                    }
                }

                // Also suggest aggregate functions
                for (func, desc) in Self::get_sql_functions() {
                    if func.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::function(func, Some(desc)));
                    }
                }

                // Also suggest aliases themselves if they match
                for (alias, _) in &aliases {
                    if alias.to_lowercase().starts_with(&prefix_lower) {
                        // Suggest alias as a "Table" kind or maybe new kind? Table is fine.
                        items.push(CompletionItem::table(alias.clone()));
                    }
                }

                // Suggest keywords that can follow a column expression
                // e.g. FROM, AS, WHERE, GROUP, ORDER, LIMIT
                for kw in ["FROM", "AS", "WHERE", "GROUP", "ORDER", "LIMIT"] {
                    if kw.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::keyword(kw));
                    }
                }
            }
            SqlContext::TypeContext => {
                // Suggest types
                for type_ in Self::get_sql_types() {
                    if type_.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::type_(type_));
                    }
                }

                // Also suggest keywords like PRIMARY, NULL, etc.
                for kw in [
                    "PRIMARY",
                    "KEY",
                    "NOT",
                    "NULL",
                    "DEFAULT",
                    "REFERENCES",
                    "UNIQUE",
                    "CHECK",
                    "AUTOINCREMENT",
                ] {
                    if kw.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::keyword(kw));
                    }
                }
            }
            SqlContext::IndexContext => {
                // Suggest index names
                for index in &self.cached_indexes {
                    if index.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem {
                            label: index.clone(),
                            kind: CompletionItemKind::Table, // Use Table kind for indexes
                            detail: Some("index".to_string()),
                            documentation: None,
                            insert_text: None,
                        });
                    }
                }

                // Also suggest IF EXISTS for DROP INDEX IF EXISTS
                for kw in ["IF", "EXISTS"] {
                    if kw.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::keyword(kw));
                    }
                }
            }
            SqlContext::Default => {
                // Suggest keywords
                for kw in Self::get_sql_keywords() {
                    if kw.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::keyword(kw));
                    }
                }

                // Suggest functions
                for (func, desc) in Self::get_sql_functions() {
                    if func.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::function(func, Some(desc)));
                    }
                }

                // Suggest tables
                for table in &self.cached_tables {
                    if table.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem::table(table.clone()));
                    }
                }
            }
        }

        items
    }

    /// Get hover information at the given position.
    pub fn hover(&self, text: &str, pos: Position) -> Option<HoverResult> {
        let offset = self.position_to_offset(text, pos);
        let (word_start, word) = self.get_word_at_offset(text, offset);

        if word.is_empty() {
            return None;
        }

        let word_lower = word.to_lowercase();

        // Check if it's a table name
        if let Some(table) = self
            .cached_tables
            .iter()
            .find(|t| t.to_lowercase() == word_lower)
        {
            let columns: Vec<&ColumnInfo> = self
                .cached_columns
                .iter()
                .filter(|c| c.table == *table)
                .collect();

            let mut contents = format!("**Table: {}**\n\n", table);
            contents.push_str("| Column | Type | Constraints |\n");
            contents.push_str("|--------|------|-------------|\n");

            for col in columns {
                let mut constraints: Vec<String> = Vec::new();
                if col.is_pk {
                    constraints.push("PRIMARY KEY".to_string());
                }
                if !col.is_nullable {
                    constraints.push("NOT NULL".to_string());
                }
                if let Some(ref def) = col.default_value {
                    constraints.push(format!("DEFAULT {}", def));
                }

                contents.push_str(&format!(
                    "| {} | {} | {} |\n",
                    col.name,
                    col.type_,
                    constraints.join(", ")
                ));
            }

            // Add CREATE statement if available
            if let Some((_, create_sql)) = self
                .cached_create_sqls
                .iter()
                .find(|(t, _)| t.to_lowercase() == word_lower)
            {
                contents.push_str(&format!("\n```sql\n{}\n```", create_sql));
            }

            let range = Range::on_line(
                pos.line,
                word_start as u32,
                (word_start + word.len()) as u32,
            );

            return Some(HoverResult {
                contents,
                range: Some(range),
            });
        }

        // Check if it's a column name
        let matching_columns: Vec<&ColumnInfo> = self
            .cached_columns
            .iter()
            .filter(|c| c.name.to_lowercase() == word_lower)
            .collect();

        if !matching_columns.is_empty() {
            let mut contents = format!("**Column: {}**\n\n", word);

            if matching_columns.len() == 1 {
                let col = matching_columns[0];
                contents.push_str(&format!("- **Table:** {}\n", col.table));
                contents.push_str(&format!("- **Type:** {}\n", col.type_));
                if col.is_pk {
                    contents.push_str("- **Primary Key:** Yes\n");
                }
                if !col.is_nullable {
                    contents.push_str("- **Nullable:** No\n");
                }
                if let Some(ref def) = col.default_value {
                    contents.push_str(&format!("- **Default:** {}\n", def));
                }
            } else {
                contents.push_str("Found in multiple tables:\n\n");
                for col in matching_columns {
                    contents.push_str(&format!(
                        "- **{}.{}** ({})\n",
                        col.table, col.name, col.type_
                    ));
                }
            }

            let range = Range::on_line(
                pos.line,
                word_start as u32,
                (word_start + word.len()) as u32,
            );

            return Some(HoverResult {
                contents,
                range: Some(range),
            });
        }

        // Check if it's a SQL keyword
        let keyword_upper = word.to_uppercase();
        if Self::get_sql_keywords().contains(&keyword_upper.as_str()) {
            return Some(HoverResult {
                contents: format!("**SQL Keyword:** {}", keyword_upper),
                range: Some(Range::on_line(
                    pos.line,
                    word_start as u32,
                    (word_start + word.len()) as u32,
                )),
            });
        }

        // Check if it's a function
        if let Some((_, desc)) = Self::get_sql_functions()
            .iter()
            .find(|(f, _)| f.to_uppercase() == keyword_upper)
        {
            return Some(HoverResult {
                contents: format!("**SQL Function**\n\n{}", desc),
                range: Some(Range::on_line(
                    pos.line,
                    word_start as u32,
                    (word_start + word.len()) as u32,
                )),
            });
        }

        None
    }

    /// Validate SQL and return diagnostics.
    pub fn diagnostics(&self, text: &str, conn: &Connection) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Split by semicolons to handle multiple statements
        let mut offset = 0;
        for stmt in text.split(';') {
            let trimmed = stmt.trim();
            if !trimmed.is_empty() {
                // Try to prepare the statement
                if let Err(e) = conn.prepare(trimmed) {
                    let error_msg = e.to_string();

                    // Try to extract line/column info from error
                    // SQLite errors sometimes include position info
                    let (line, col) = self.find_error_position(text, offset, &error_msg);

                    diagnostics.push(Diagnostic {
                        range: Range::on_line(line, col, col + 1),
                        severity: DiagnosticSeverity::Error,
                        message: error_msg,
                    });
                }
            }
            offset += stmt.len() + 1; // +1 for semicolon
        }

        diagnostics
    }

    /// Try to find the position of an error in the text.
    fn find_error_position(&self, text: &str, stmt_offset: usize, _error_msg: &str) -> (u32, u32) {
        // Calculate line and column from offset
        let mut line = 0u32;
        let mut col = 0u32;
        let mut current_offset = 0;

        for ch in text.chars() {
            if current_offset >= stmt_offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
            current_offset += ch.len_utf8();
        }

        (line, col)
    }

    /// Find the definition of a symbol at the given position.
    pub fn goto_definition(&self, text: &str, pos: Position) -> Option<SymbolLocation> {
        let offset = self.position_to_offset(text, pos);
        let (_, word) = self.get_word_at_offset(text, offset);

        if word.is_empty() {
            return None;
        }

        let word_lower = word.to_lowercase();

        // Check if it's a table name
        if let Some(table) = self
            .cached_tables
            .iter()
            .find(|t| t.to_lowercase() == word_lower)
        {
            let create_sql = self
                .cached_create_sqls
                .iter()
                .find(|(t, _)| t == table)
                .map(|(_, sql)| sql.clone());

            return Some(SymbolLocation {
                table_name: table.clone(),
                column_name: None,
                create_sql,
            });
        }

        // Check if it's a column name
        if let Some(col) = self
            .cached_columns
            .iter()
            .find(|c| c.name.to_lowercase() == word_lower)
        {
            let create_sql = self
                .cached_create_sqls
                .iter()
                .find(|(t, _)| *t == col.table)
                .map(|(_, sql)| sql.clone());

            return Some(SymbolLocation {
                table_name: col.table.clone(),
                column_name: Some(col.name.clone()),
                create_sql,
            });
        }

        None
    }

    /// Get all tables in the database.
    pub fn get_tables(&self) -> &[String] {
        &self.cached_tables
    }

    /// Get all columns for a specific table.
    pub fn get_columns(&self, table: &str) -> Vec<(&str, &str)> {
        self.cached_columns
            .iter()
            .filter(|c| c.table == table)
            .map(|c| (c.name.as_str(), c.type_.as_str()))
            .collect()
    }
}

impl Default for SqlLspService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
