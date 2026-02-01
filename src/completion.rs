use std::borrow::Cow;

use rusqlite::Connection;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::Result as RustylineResult;
use rustyline::{Context, Helper};

use crate::cli_state::OutputMode;
use crate::dot_commands::DotCommand;
use crate::lsp::{Position, SqlLspService};
use crate::sql_highlight::SqlHighlighter;

pub struct SqlCompleter {
    db_path: String,
    lsp: SqlLspService,
    highlighter: SqlHighlighter,
}

impl Helper for SqlCompleter {}
impl Hinter for SqlCompleter {
    type Hint = String;
}
impl Validator for SqlCompleter {}
impl Highlighter for SqlCompleter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if line.is_empty() {
            return Cow::Borrowed(line);
        }
        Cow::Owned(self.highlighter.highlight_line(line))
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        true
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        let _ = default;
        Cow::Borrowed(prompt)
    }
}

impl SqlCompleter {
    pub fn new(db_path: String) -> Self {
        Self {
            db_path,
            lsp: SqlLspService::new(),
            highlighter: SqlHighlighter::new(),
        }
    }

    /// Create a completer with pre-populated caches (useful for testing)
    #[cfg(test)]
    pub fn with_cache(
        db_path: String,
        tables: Vec<String>,
        columns: Vec<(String, String)>,
    ) -> Self {
        // Convert to LSP column format: (table, name, type, pk, nullable)
        let lsp_columns: Vec<(String, String, String, bool, bool)> = columns
            .into_iter()
            .map(|(table, name)| (table, name, "TEXT".to_string(), false, true))
            .collect();

        Self {
            db_path,
            lsp: SqlLspService::with_cache(tables, lsp_columns),
            highlighter: SqlHighlighter::new(),
        }
    }

    /// Refresh table and column cache from database
    pub fn refresh_cache(&mut self) -> rusqlite::Result<()> {
        if let Ok(conn) = Connection::open(&self.db_path) {
            self.lsp.refresh_schema(&conn)?;
        }
        Ok(())
    }

    /// Get dot commands
    fn get_dot_commands() -> Vec<&'static str> {
        DotCommand::all().iter().map(|c| c.as_str()).collect()
    }

    /// Get output mode names
    fn get_output_modes() -> Vec<&'static str> {
        OutputMode::all().iter().map(|m| m.as_str()).collect()
    }

    /// Get on/off options
    fn get_on_off() -> Vec<&'static str> {
        vec!["on", "off"]
    }

    /// Handle dot command completions (REPL-specific)
    fn get_dot_command_completions(&self, line: &str, pos: usize) -> Option<Vec<String>> {
        let before_cursor = &line[..pos];
        let words: Vec<&str> = before_cursor.split_whitespace().collect();

        // Only handle lines starting with dot
        if !before_cursor.trim_start().starts_with('.') {
            return None;
        }

        if !words.is_empty() {
            let dot_cmd = words[0].to_lowercase();

            // Check if there's a space after the dot command
            let has_space_after_cmd = before_cursor.ends_with(' ') || words.len() >= 2;

            if has_space_after_cmd {
                // .schema [TABLE] - suggest table names
                if dot_cmd == ".schema" {
                    return Some(self.lsp.get_tables().to_vec());
                }

                // .dump [TABLE] - suggest table names
                if dot_cmd == ".dump" {
                    return Some(self.lsp.get_tables().to_vec());
                }

                // .mode [MODE] - suggest output modes
                if dot_cmd == ".mode" {
                    return Some(
                        Self::get_output_modes()
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    );
                }

                // .headers on|off - suggest on/off
                if dot_cmd == ".headers" {
                    return Some(Self::get_on_off().iter().map(|s| s.to_string()).collect());
                }

                // .timer on|off - suggest on/off
                if dot_cmd == ".timer" {
                    return Some(Self::get_on_off().iter().map(|s| s.to_string()).collect());
                }

                // .echo on|off - suggest on/off
                if dot_cmd == ".echo" {
                    return Some(Self::get_on_off().iter().map(|s| s.to_string()).collect());
                }

                // .import FILE TABLE - suggest table names on second argument
                if dot_cmd == ".import" && words.len() >= 3 {
                    return Some(self.lsp.get_tables().to_vec());
                }
            }
        }

        // Default: suggest dot commands themselves
        Some(
            Self::get_dot_commands()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )
    }

    /// Detect context and provide appropriate completions
    fn get_context_completions(&self, line: &str, pos: usize) -> Vec<String> {
        // First check if this is a dot command
        if let Some(completions) = self.get_dot_command_completions(line, pos) {
            return completions;
        }

        // Use LSP for SQL completions
        let lsp_completions = self.lsp.completion(line, Position::new(0, pos as u32));

        // Convert LSP CompletionItems to strings
        lsp_completions.into_iter().map(|item| item.label).collect()
    }

    // Keep these methods for test compatibility
    #[cfg(test)]
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
            "COUNT",
            "SUM",
            "AVG",
            "MIN",
            "MAX",
            "CAST",
            "COALESCE",
            "NULLIF",
            "IFNULL",
            "LENGTH",
            "SUBSTR",
            "UPPER",
            "LOWER",
            "TRIM",
            "REPLACE",
            "ROUND",
            "ABS",
            "DATETIME",
            "DATE",
            "TIME",
            "STRFTIME",
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
        ]
    }
}

impl Completer for SqlCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> RustylineResult<(usize, Vec<Pair>)> {
        // Get the word being completed
        let mut start = pos;
        while start > 0 {
            let ch = line.chars().nth(start - 1);
            if let Some(c) = ch {
                if c.is_alphanumeric() || c == '_' || c == '.' {
                    start -= 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let word = &line[start..pos];
        let word_lower = word.to_lowercase();

        // Get context-appropriate completions
        let candidates = self.get_context_completions(line, pos);

        // Filter candidates that match the current word
        let matches: Vec<Pair> = candidates
            .into_iter()
            .filter(|candidate| candidate.to_lowercase().starts_with(&word_lower))
            .map(|candidate| Pair {
                display: candidate.clone(),
                replacement: candidate,
            })
            .collect();

        Ok((start, matches))
    }
}

#[cfg(test)]
mod tests;
