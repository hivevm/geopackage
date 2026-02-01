use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum OutputMode {
    List,
    Csv,
    Column,
    Line,
    Json,
    Jsonl,
    Table,
    Markdown,
}

impl OutputMode {
    pub fn all() -> &'static [OutputMode] {
        &[
            OutputMode::List,
            OutputMode::Csv,
            OutputMode::Column,
            OutputMode::Line,
            OutputMode::Json,
            OutputMode::Jsonl,
            OutputMode::Table,
            OutputMode::Markdown,
        ]
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "list" => Some(OutputMode::List),
            "csv" => Some(OutputMode::Csv),
            "column" | "columns" => Some(OutputMode::Column),
            "line" => Some(OutputMode::Line),
            "json" => Some(OutputMode::Json),
            "jsonl" => Some(OutputMode::Jsonl),
            "table" | "box" => Some(OutputMode::Table),
            "markdown" | "md" => Some(OutputMode::Markdown),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            OutputMode::List => "list",
            OutputMode::Csv => "csv",
            OutputMode::Column => "column",
            OutputMode::Line => "line",
            OutputMode::Json => "json",
            OutputMode::Jsonl => "jsonl",
            OutputMode::Table => "table",
            OutputMode::Markdown => "markdown",
        }
    }
}

pub struct CliState {
    pub output_mode: OutputMode,
    pub show_headers: bool,
    pub separator: String,
    pub null_value: String,
    pub echo: bool,
    pub bail: bool,
    pub output_file: Option<File>,
    pub timer: bool,
    pub database_path: PathBuf,
    pub color_enabled: bool,
    pub column_widths: Vec<usize>,
    // Store original mode when temporarily switched by .output
    pub saved_output_mode: Option<OutputMode>,
}

impl CliState {
    pub fn new(database_path: PathBuf) -> Self {
        Self {
            output_mode: OutputMode::Table,
            show_headers: true,
            separator: "|".to_string(),
            null_value: "".to_string(),
            echo: false,
            bail: false,
            output_file: None,
            timer: false,
            database_path,
            color_enabled: is_color_supported(),
            column_widths: Vec::new(),
            saved_output_mode: None,
        }
    }

    /// Set the output mode
    pub fn set_mode(&mut self, mode: OutputMode) {
        self.output_mode = mode;
    }

    /// Toggle headers
    pub fn set_headers(&mut self, show: bool) {
        self.show_headers = show;
    }

    /// Set separator for list mode
    pub fn set_separator(&mut self, sep: String) {
        self.separator = sep;
    }

    /// Set NULL value display
    pub fn set_null_value(&mut self, value: String) {
        self.null_value = value;
    }

    /// Set echo mode
    pub fn set_echo(&mut self, echo: bool) {
        self.echo = echo;
    }

    /// Set bail mode (stop on error)
    pub fn set_bail(&mut self, bail: bool) {
        self.bail = bail;
    }

    /// Set timer mode
    pub fn set_timer(&mut self, timer: bool) {
        self.timer = timer;
    }

    /// Set column widths
    pub fn set_column_widths(&mut self, widths: Vec<usize>) {
        self.column_widths = widths;
    }

    /// Redirect output to a file
    /// Returns an optional status message
    pub fn set_output_file(&mut self, path: Option<String>) -> io::Result<Option<String>> {
        if let Some(p) = path {
            let path_buf = PathBuf::from(&p);
            let file = File::create(&p)?;
            self.output_file = Some(file);

            // Guess mode from extension
            let extension = path_buf
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase());
            let new_mode = match extension.as_deref() {
                Some("csv") => Some(OutputMode::Csv),
                Some("json") => Some(OutputMode::Json),
                Some("jsonl") | Some("ndjson") => Some(OutputMode::Jsonl),
                Some("md") | Some("markdown") => Some(OutputMode::Markdown),
                Some("html") => None, // Not supported yet
                _ => None,
            };

            if let Some(mode) = new_mode {
                if mode != self.output_mode {
                    // Save current mode if not already saved
                    if self.saved_output_mode.is_none() {
                        self.saved_output_mode = Some(self.output_mode.clone());
                    }
                    self.output_mode = mode;
                    return Ok(Some(format!(
                        "Output mode temporarily set to '{}' based on file extension.",
                        self.output_mode.as_str()
                    )));
                }
            }
        } else {
            self.output_file = None;
            // Restore saved mode
            if let Some(original_mode) = self.saved_output_mode.take() {
                self.output_mode = original_mode;
                // Since we are back to stdout (usually), maybe we don't need to be noisy?
                // But the requirement says "it should switch back".
                // Implicitly, it's good to confirm.
                // But .output stdout usually assumes silence.
                // However, since we changed state implicitly, reverting implicitly is symmetrical.
            }
        }
        Ok(None)
    }

    /// Write to output (file or stdout)
    pub fn write_output(&mut self, content: &str) -> io::Result<()> {
        if let Some(ref mut file) = self.output_file {
            file.write_all(content.as_bytes())?;
            file.write_all(b"\n")?;
            file.flush()?;
        } else {
            println!("{}", content);
        }
        Ok(())
    }

    /// Get current settings as a formatted string (for .show command)
    pub fn get_settings(&self) -> String {
        format!(
            r#"        echo: {}
         eqp: off
     explain: auto
     headers: {}
        mode: {}
   nullvalue: "{}"
      output: {}
colseparator: "{}"
rowseparator: "\n"
       stats: off
       width: {}
    filename: {}"#,
            if self.echo { "on" } else { "off" },
            if self.show_headers { "on" } else { "off" },
            self.output_mode.as_str(),
            self.null_value,
            if self.output_file.is_some() {
                "file"
            } else {
                "stdout"
            },
            self.separator,
            self.column_widths
                .iter()
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
                .join(" "),
            self.database_path.display()
        )
    }
}

/// Check if color output is supported
fn is_color_supported() -> bool {
    // Use is-terminal to check if we're outputting to a terminal
    is_terminal::is_terminal(&io::stdout())
}

#[cfg(test)]
mod tests;
