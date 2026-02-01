use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;

/// SQL syntax highlighter using syntect
pub struct SqlHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl SqlHighlighter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Highlight a single line of SQL
    pub fn highlight_line(&self, line: &str) -> String {
        if line.is_empty() {
            return line.to_string();
        }

        let syntax = self
            .syntax_set
            .find_syntax_by_extension("sql")
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        match highlighter.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => {
                let mut escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                escaped.push_str("\x1b[0m");
                escaped
            }
            Err(_) => line.to_string(),
        }
    }

    /// Highlight multi-line SQL (preserves line structure)
    pub fn highlight(&self, sql: &str) -> String {
        sql.lines()
            .map(|line| self.highlight_line(line))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for SqlHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

// Thread-local highlighter for efficient reuse
thread_local! {
    static HIGHLIGHTER: SqlHighlighter = SqlHighlighter::new();
}

/// Convenience function to highlight SQL without creating a new highlighter
pub fn highlight_sql(sql: &str) -> String {
    HIGHLIGHTER.with(|h| h.highlight(sql))
}

#[cfg(test)]
mod tests;
