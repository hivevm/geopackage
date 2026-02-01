use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::Connection;
use rustyline::config::Builder as ConfigBuilder;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use rustyline::Editor;

use crate::cli_state::CliState;
use crate::completion::SqlCompleter;
use crate::dot_commands::{self, CommandResult, DotCommand};
use crate::sql_executor;

pub struct Repl {
    conn: Connection,
    pub state: CliState,
    editor: Editor<SqlCompleter, DefaultHistory>,
    sql_buffer: String,
}

impl Repl {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

        let state = CliState::new(db_path.clone());

        // Create completer with database path
        let mut completer = SqlCompleter::new(db_path.display().to_string());
        let _ = completer.refresh_cache(); // Initial cache refresh

        // Configure editor with completion
        let config = ConfigBuilder::new().auto_add_history(true).build();

        let mut editor = Editor::with_config(config)?;
        editor.set_helper(Some(completer));

        Ok(Self {
            conn,
            state,
            editor,
            sql_buffer: String::new(),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        // Print welcome message
        self.print_welcome();

        loop {
            // Determine prompt based on whether we're in a multi-line statement
            let prompt = if self.sql_buffer.is_empty() {
                "rsqlite3> "
            } else {
                "     ...> "
            };

            // Read a line
            let readline = self.editor.readline(prompt);

            match readline {
                Ok(line) => {
                    // Process the line
                    match self.process_line(&line) {
                        Ok(CommandResult::Quit) => break,
                        Ok(CommandResult::ChangeDb(path)) => {
                            // Reconnect
                            match Connection::open(&path) {
                                Ok(conn) => {
                                    self.conn = conn;
                                    self.state.database_path = path.clone();
                                    // Refresh completer
                                    let mut completer =
                                        SqlCompleter::new(path.display().to_string());
                                    let _ = completer.refresh_cache();
                                    self.editor.set_helper(Some(completer));
                                    println!("Connected to {}", path.display());
                                }
                                Err(e) => eprintln!("Error opening database: {}", e),
                            }
                        }
                        Ok(CommandResult::Continue) => {}
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            // Clear buffer on error if bail mode is off
                            if self.state.bail {
                                return Err(e);
                            }
                            self.sql_buffer.clear();
                        }
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    // Ctrl-C: Clear current buffer
                    self.sql_buffer.clear();
                    println!("^C");
                }
                Err(ReadlineError::Eof) => {
                    // Ctrl-D: Exit
                    break;
                }
                Err(err) => {
                    eprintln!("Error: {}", err);
                    break;
                }
            }
        }

        Ok(())
    }

    fn process_line(&mut self, line: &str) -> Result<CommandResult> {
        let trimmed = line.trim();

        // Check if it's a dot command
        if trimmed.starts_with('.') && self.sql_buffer.is_empty() {
            return self.process_dot_command(trimmed);
        }

        // Add to SQL buffer
        if !self.sql_buffer.is_empty() {
            self.sql_buffer.push(' ');
        }
        self.sql_buffer.push_str(line);

        // Check if the statement is complete (ends with semicolon)
        if self.is_complete_statement(&self.sql_buffer) {
            let sql = self.sql_buffer.trim().to_string();
            self.sql_buffer.clear();

            // Echo if enabled
            if self.state.echo {
                println!("{}", sql);
            }

            // Execute the SQL
            sql_executor::execute(&self.conn, &sql, &mut self.state)?;
        }

        Ok(CommandResult::Continue)
    }

    fn process_dot_command(&mut self, command: &str) -> Result<CommandResult> {
        let result = dot_commands::execute(&self.conn, command, &mut self.state);

        // Refresh completion cache after certain commands
        if command.starts_with(DotCommand::Schema.as_str())
            || command.starts_with(DotCommand::Tables.as_str())
        {
            if let Some(helper) = self.editor.helper_mut() {
                let _ = helper.refresh_cache();
            }
        }

        result
    }

    fn is_complete_statement(&self, sql: &str) -> bool {
        let trimmed = sql.trim();

        // Simple check: ends with semicolon
        // TODO: More sophisticated parsing to handle semicolons in strings/comments
        trimmed.ends_with(';')
    }

    fn print_welcome(&self) {
        println!("rsqlite3 version {}", env!("CARGO_PKG_VERSION"));
        println!("Enter \".help\" for usage hints.");
        println!("Connected to {}", self.state.database_path.display());
    }
}

/// Run the REPL with the given database path
pub fn run_repl(db_path: PathBuf) -> Result<()> {
    let mut repl = Repl::new(db_path)?;
    repl.run()
}
