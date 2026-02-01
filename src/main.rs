mod cli_state;
mod completion;
mod db;
mod dot_commands;
mod import_export;
mod lsp;
mod output;
mod repl;
mod sql_executor;
mod sql_highlight;

use std::io::{self, Read};
use std::path::PathBuf;
use std::process;

use anyhow::{Context, Result};
use clap::Parser;
use cli_state::CliState;
use rusqlite::Connection;

#[derive(Parser, Debug)]
#[command(
    name = "rsqlite3",
    author,
    version,
    about = "A drop-in replacement for sqlite3 CLI with enhanced features",
    long_about = None
)]
struct Args {
    /// Path to the SQLite database file
    #[arg(value_name = "DATABASE")]
    database: Option<PathBuf>,

    /// SQL command to execute
    #[arg(value_name = "SQL")]
    sql: Option<String>,

    /// Show column headers
    #[arg(short = 'H', long = "header")]
    header: bool,

    /// Do not show column headers
    #[arg(long = "noheader")]
    noheader: bool,

    /// Set output mode (list, csv, column, json, line, table, markdown)
    #[arg(short = 'm', long = "mode", value_name = "MODE")]
    mode: Option<String>,

    /// Set column separator for list mode
    #[arg(short = 's', long = "separator", value_name = "SEP")]
    separator: Option<String>,

    /// Set NULL value display string
    #[arg(short = 'n', long = "nullvalue", value_name = "TEXT")]
    nullvalue: Option<String>,

    /// Open database in read-only mode
    #[arg(short = 'r', long = "readonly")]
    readonly: bool,

    /// Execute SQL from init file before processing
    #[arg(long = "init", value_name = "FILE")]
    init: Option<PathBuf>,

    /// Run command before reading stdin
    #[arg(long = "cmd", value_name = "COMMAND")]
    cmd: Option<String>,

    /// Echo commands before executing
    #[arg(short = 'e', long = "echo")]
    echo: bool,

    /// Stop after hitting an error
    #[arg(short = 'b', long = "bail")]
    bail: bool,

    /// Enable color output
    #[arg(long = "color")]
    color: bool,

    /// Disable color output
    #[arg(long = "no-color")]
    no_color: bool,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();

    // Determine database path
    let db_path = args
        .database
        .clone()
        .unwrap_or_else(|| PathBuf::from("database.db"));

    // Determine mode
    let is_interactive = args.sql.is_none() && is_terminal::is_terminal(io::stdin());

    if is_interactive {
        // Interactive REPL mode
        run_interactive(db_path, &args)
    } else if let Some(sql) = &args.sql {
        // One-shot SQL mode
        run_one_shot(db_path, &args, sql)
    } else {
        // Piped input mode
        run_piped(db_path, &args)
    }
}

fn run_interactive(db_path: PathBuf, args: &Args) -> Result<()> {
    let mut repl = repl::Repl::new(db_path)?;

    // Apply CLI options to state
    configure_state(&mut repl, args)?;

    // Run init file if specified
    if let Some(init_file) = &args.init {
        let content = std::fs::read_to_string(init_file)
            .with_context(|| format!("Failed to read init file: {}", init_file.display()))?;

        let conn = Connection::open(&repl.state.database_path)?;
        for stmt in content.split(';') {
            let trimmed = stmt.trim();
            if !trimmed.is_empty() {
                let sql = format!("{};", trimmed);
                sql_executor::execute(&conn, &sql, &mut repl.state)?;
            }
        }
    }

    // Run command if specified
    if let Some(cmd) = &args.cmd {
        let conn = Connection::open(&repl.state.database_path)?;
        if cmd.starts_with('.') {
            dot_commands::execute(&conn, cmd, &mut repl.state)?;
        } else {
            sql_executor::execute(&conn, cmd, &mut repl.state)?;
        }
    }

    // Start REPL
    repl.run()
}

fn run_one_shot(db_path: PathBuf, args: &Args, sql: &str) -> Result<()> {
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

    let mut state = CliState::new(db_path);
    configure_cli_state(&mut state, args)?;

    // Execute the SQL
    sql_executor::execute(&conn, sql, &mut state)?;

    Ok(())
}

fn run_piped(db_path: PathBuf, args: &Args) -> Result<()> {
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

    let mut state = CliState::new(db_path);
    configure_cli_state(&mut state, args)?;

    // Read from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    // Execute each statement
    for stmt in input.split(';') {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() {
            if trimmed.starts_with('.') {
                dot_commands::execute(&conn, trimmed, &mut state)?;
            } else {
                let sql = format!("{};", trimmed);
                sql_executor::execute(&conn, &sql, &mut state)?;
            }
        }
    }

    Ok(())
}

fn configure_state(repl: &mut repl::Repl, args: &Args) -> Result<()> {
    configure_cli_state(&mut repl.state, args)
}

fn configure_cli_state(state: &mut CliState, args: &Args) -> Result<()> {
    // Headers
    if args.header {
        state.set_headers(true);
    }
    if args.noheader {
        state.set_headers(false);
    }

    // Output mode
    if let Some(mode_str) = &args.mode {
        if let Some(mode) = cli_state::OutputMode::from_str(mode_str) {
            state.set_mode(mode);
        } else {
            eprintln!("Error: mode should be one of: csv column json line list markdown table");
            process::exit(1);
        }
    }

    // Separator
    if let Some(sep) = &args.separator {
        state.set_separator(sep.clone());
    }

    // Null value
    if let Some(null) = &args.nullvalue {
        state.set_null_value(null.clone());
    }

    // Echo
    if args.echo {
        state.set_echo(true);
    }

    // Bail
    if args.bail {
        state.set_bail(true);
    }

    // Color
    if args.color {
        state.color_enabled = true;
    }
    if args.no_color {
        state.color_enabled = false;
    }

    Ok(())
}
