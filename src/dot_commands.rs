use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use rusqlite::Connection;

use crate::cli_state::{CliState, OutputMode};
use crate::db;
use crate::sql_highlight;

/// Result of executing a dot command
#[derive(Debug, PartialEq)]
pub enum CommandResult {
    /// Continue REPL loop
    Continue,
    /// Exit REPL loop
    Quit,
    /// Change database connection
    ChangeDb(PathBuf),
}

/// Enum representing all supported dot commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotCommand {
    Quit,
    Exit,
    Help,
    Tables,
    Schema,
    Mode,
    Headers,
    Show,
    Dump,
    Output,
    Read,
    Databases,
    Separator,
    NullValue,
    Import,
    Timer,
    Echo,
    Width,
    Bail,
    Open,
}

impl DotCommand {
    pub fn all() -> &'static [DotCommand] {
        &[
            DotCommand::Quit,
            DotCommand::Exit,
            DotCommand::Help,
            DotCommand::Tables,
            DotCommand::Schema,
            DotCommand::Mode,
            DotCommand::Headers,
            DotCommand::Show,
            DotCommand::Dump,
            DotCommand::Output,
            DotCommand::Read,
            DotCommand::Databases,
            DotCommand::Separator,
            DotCommand::NullValue,
            DotCommand::Import,
            DotCommand::Timer,
            DotCommand::Echo,
            DotCommand::Width,
            DotCommand::Bail,
            DotCommand::Open,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DotCommand::Quit => ".quit",
            DotCommand::Exit => ".exit",
            DotCommand::Help => ".help",
            DotCommand::Tables => ".tables",
            DotCommand::Schema => ".schema",
            DotCommand::Mode => ".mode",
            DotCommand::Headers => ".headers",
            DotCommand::Show => ".show",
            DotCommand::Dump => ".dump",
            DotCommand::Output => ".output",
            DotCommand::Read => ".read",
            DotCommand::Databases => ".databases",
            DotCommand::Separator => ".separator",
            DotCommand::NullValue => ".nullvalue",
            DotCommand::Import => ".import",
            DotCommand::Timer => ".timer",
            DotCommand::Echo => ".echo",
            DotCommand::Width => ".width",
            DotCommand::Bail => ".bail",
            DotCommand::Open => ".open",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            ".quit" => Some(DotCommand::Quit),
            ".exit" => Some(DotCommand::Exit),
            ".help" => Some(DotCommand::Help),
            ".tables" => Some(DotCommand::Tables),
            ".schema" => Some(DotCommand::Schema),
            ".mode" => Some(DotCommand::Mode),
            ".headers" => Some(DotCommand::Headers),
            ".show" => Some(DotCommand::Show),
            ".dump" => Some(DotCommand::Dump),
            ".output" => Some(DotCommand::Output),
            ".read" => Some(DotCommand::Read),
            ".databases" => Some(DotCommand::Databases),
            ".separator" => Some(DotCommand::Separator),
            ".nullvalue" => Some(DotCommand::NullValue),
            ".import" => Some(DotCommand::Import),
            ".timer" => Some(DotCommand::Timer),
            ".echo" => Some(DotCommand::Echo),
            ".width" => Some(DotCommand::Width),
            ".bail" => Some(DotCommand::Bail),
            ".open" => Some(DotCommand::Open),
            _ => None,
        }
    }
}

/// Execute a dot command
pub fn execute(conn: &Connection, command: &str, state: &mut CliState) -> Result<CommandResult> {
    let parts: Vec<&str> = command.split_whitespace().collect();

    if parts.is_empty() {
        return Err(anyhow!("Empty command"));
    }

    let cmd_enum = DotCommand::from_str(parts[0])
        .ok_or_else(|| anyhow!("Unknown command: {}. Enter \".help\" for help", parts[0]))?;

    match cmd_enum {
        DotCommand::Quit | DotCommand::Exit => {
            return Ok(CommandResult::Quit);
        }
        DotCommand::Help => {
            print_help(state)?;
        }
        DotCommand::Tables => {
            cmd_tables(conn, state, parts.get(1).copied())?;
        }
        DotCommand::Schema => {
            cmd_schema(conn, state, parts.get(1).copied())?;
        }
        DotCommand::Mode => {
            cmd_mode(state, parts.get(1).copied())?;
        }
        DotCommand::Headers => {
            cmd_headers(state, parts.get(1).copied())?;
        }
        DotCommand::Show => {
            cmd_show(state)?;
        }
        DotCommand::Dump => {
            cmd_dump(conn, state, parts.get(1).copied())?;
        }
        DotCommand::Output => {
            cmd_output(state, parts.get(1).copied())?;
        }
        DotCommand::Read => {
            if let Some(file) = parts.get(1) {
                cmd_read(conn, state, file)?;
            } else {
                return Err(anyhow!("Usage: .read FILE"));
            }
        }
        DotCommand::Databases => {
            cmd_databases(conn, state)?;
        }
        DotCommand::Separator => {
            cmd_separator(state, parts.get(1).copied())?;
        }
        DotCommand::NullValue => {
            cmd_nullvalue(state, parts.get(1).copied())?;
        }
        DotCommand::Import => {
            if parts.len() < 3 {
                return Err(anyhow!("Usage: .import FILE TABLE"));
            }
            cmd_import(conn, state, parts[1], parts[2])?;
        }
        DotCommand::Timer => {
            cmd_timer(state, parts.get(1).copied())?;
        }
        DotCommand::Echo => {
            cmd_echo(state, parts.get(1).copied())?;
        }
        DotCommand::Width => {
            cmd_width(state, &parts[1..])?;
        }
        DotCommand::Bail => {
            cmd_bail(state, parts.get(1).copied())?;
        }
        DotCommand::Open => {
            if let Some(path) = parts.get(1) {
                return Ok(CommandResult::ChangeDb(PathBuf::from(path)));
            } else {
                return Err(anyhow!("Usage: .open FILENAME"));
            }
        }
    }

    Ok(CommandResult::Continue)
}

fn print_help(state: &mut CliState) -> Result<()> {
    let modes = OutputMode::all()
        .iter()
        .map(|m| m.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let help_text = format!(
        r#"
.bail on|off           Stop after hitting an error.  Default OFF
.databases             List names and files of attached databases
.dump ?TABLE?          Render database content as SQL
.echo on|off           Turn command echo on or off
.exit                  Exit this program
.headers on|off        Turn display of headers on or off
.help                  Show this message
.import FILE TABLE     Import data from FILE into TABLE
.mode MODE             Set output mode
                       MODE is one of: {modes}
.nullvalue STRING      Use STRING in place of NULL values
.open FILE             Close existing database and reopen FILE
.output FILE           Send output to FILE (or stdout if FILE is omitted)
.quit                  Exit this program
.read FILE             Read input from FILE
.schema ?TABLE?        Show the CREATE statements
.separator SEP         Change separator for output mode "list"
.show                  Show the current values for various settings
.tables ?PATTERN?      List names of tables matching PATTERN
.timer on|off          Turn SQL timer on or off
.width NUM1 NUM2 ...   Set column widths for "column" mode
"#
    );

    state.write_output(help_text.trim())?;
    Ok(())
}

fn cmd_tables(conn: &Connection, state: &mut CliState, pattern: Option<&str>) -> Result<()> {
    let tables = db::get_tables(conn)?;

    let filtered: Vec<String> = if let Some(pat) = pattern {
        tables.into_iter().filter(|t| t.contains(pat)).collect()
    } else {
        tables
    };

    for table in filtered {
        state.write_output(&table)?;
    }

    Ok(())
}

fn cmd_schema(conn: &Connection, state: &mut CliState, table: Option<&str>) -> Result<()> {
    let use_highlight = state.color_enabled && state.output_file.is_none();

    let highlight = |sql: &str| -> String {
        if use_highlight {
            sql_highlight::highlight_sql(sql)
        } else {
            sql.to_string()
        }
    };

    let sql = if table.is_some() {
        "SELECT sql FROM sqlite_master \
             WHERE (type='table' AND name=?1) \
                OR (type='index' AND tbl_name=?1 AND sql IS NOT NULL) \
                OR (type='trigger' AND tbl_name=?1) \
                OR (type='view' AND name=?1) \
             ORDER BY CASE type \
                WHEN 'table' THEN 1 \
                WHEN 'view' THEN 2 \
                WHEN 'index' THEN 3 \
                WHEN 'trigger' THEN 4 \
             END, name"
            .to_string()
    } else {
        "SELECT sql FROM sqlite_master \
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' \
         ORDER BY CASE type \
            WHEN 'table' THEN 1 \
            WHEN 'view' THEN 2 \
            WHEN 'index' THEN 3 \
            WHEN 'trigger' THEN 4 \
         END, name"
            .to_string()
    };

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = if table.is_some() {
        stmt.query(rusqlite::params![table.unwrap()])?
    } else {
        stmt.query([])?
    };

    let mut first = true;
    while let Some(row) = rows.next()? {
        let create_sql: Option<String> = row.get(0)?;
        if let Some(sql) = create_sql {
            if !first {
                state.write_output("")?;
            }
            first = false;
            let output = highlight(&sql);
            state.write_output(&format!("{};", output))?;
        }
    }

    Ok(())
}

fn cmd_mode(state: &mut CliState, mode: Option<&str>) -> Result<()> {
    if let Some(mode_str) = mode {
        if let Some(mode) = OutputMode::from_str(mode_str) {
            state.set_mode(mode);
        } else {
            let modes = OutputMode::all()
                .iter()
                .map(|m| m.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow!("Error: mode should be one of: {}", modes));
        }
    } else {
        state.write_output(&format!(
            "current output mode: {}",
            state.output_mode.as_str()
        ))?;
    }

    Ok(())
}

fn cmd_headers(state: &mut CliState, value: Option<&str>) -> Result<()> {
    match value {
        Some("on") | Some("1") | Some("yes") | Some("true") => {
            state.set_headers(true);
        }
        Some("off") | Some("0") | Some("no") | Some("false") => {
            state.set_headers(false);
        }
        Some(v) => {
            return Err(anyhow!("Usage: .headers on|off (got: {})", v));
        }
        None => {
            state.write_output(&format!(
                "headers: {}",
                if state.show_headers { "on" } else { "off" }
            ))?;
        }
    }

    Ok(())
}

fn cmd_show(state: &mut CliState) -> Result<()> {
    let settings = state.get_settings();
    state.write_output(&settings)?;
    Ok(())
}

fn cmd_dump(conn: &Connection, state: &mut CliState, table: Option<&str>) -> Result<()> {
    use crate::import_export;

    let tables = if let Some(t) = table {
        vec![t.to_string()]
    } else {
        db::get_tables(conn)?
    };

    let dump = import_export::generate_sql_dump(conn, Some(&tables), true)?;
    state.write_output(&dump)?;

    Ok(())
}

fn cmd_output(state: &mut CliState, file: Option<&str>) -> Result<()> {
    if let Some(msg) = state.set_output_file(file.map(|s| s.to_string()))? {
        println!("{}", msg);
    }
    Ok(())
}

fn cmd_read(conn: &Connection, state: &mut CliState, file: &str) -> Result<()> {
    use std::fs;

    let content =
        fs::read_to_string(file).with_context(|| format!("Failed to read file: {}", file))?;

    for stmt in content.split(';') {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('.') {
            let sql = format!("{};", trimmed);
            crate::sql_executor::execute(conn, &sql, state)?;
        } else if trimmed.starts_with('.') {
            match execute(conn, trimmed, state)? {
                CommandResult::ChangeDb(_) => {
                    return Err(anyhow!("Cannot change database inside .read"));
                }
                CommandResult::Quit => {
                    std::process::exit(0);
                }
                CommandResult::Continue => {}
            }
        }
    }

    Ok(())
}

fn cmd_databases(conn: &Connection, state: &mut CliState) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA database_list")?;
    let mut rows = stmt.query([])?;

    let mut output = String::new();
    output.push_str("seq  name             file\n");
    output.push_str(
        "---  ---------------  --------------------------------------------------------\n",
    );

    while let Some(row) = rows.next()? {
        let seq: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let file: String = row.get(2)?;
        output.push_str(&format!("{:<3}  {:<15}  {}\n", seq, name, file));
    }

    state.write_output(&output.trim_end())?;
    Ok(())
}

fn cmd_separator(state: &mut CliState, sep: Option<&str>) -> Result<()> {
    if let Some(s) = sep {
        state.set_separator(s.to_string());
    } else {
        state.write_output(&format!("current separator: \"{}\"", state.separator))?;
    }

    Ok(())
}

fn cmd_nullvalue(state: &mut CliState, value: Option<&str>) -> Result<()> {
    if let Some(v) = value {
        state.set_null_value(v.to_string());
    } else {
        state.write_output(&format!("current nullvalue: \"{}\"", state.null_value))?;
    }

    Ok(())
}

fn cmd_import(conn: &Connection, _state: &mut CliState, file: &str, table: &str) -> Result<()> {
    use crate::import_export;
    import_export::import_csv(conn, file, table)?;
    Ok(())
}

fn cmd_timer(state: &mut CliState, value: Option<&str>) -> Result<()> {
    match value {
        Some("on") | Some("1") | Some("yes") | Some("true") => {
            state.set_timer(true);
        }
        Some("off") | Some("0") | Some("no") | Some("false") => {
            state.set_timer(false);
        }
        Some(v) => {
            return Err(anyhow!("Usage: .timer on|off (got: {})", v));
        }
        None => {
            println!("timer: {}", if state.timer { "on" } else { "off" });
        }
    }

    Ok(())
}

fn cmd_echo(state: &mut CliState, value: Option<&str>) -> Result<()> {
    match value {
        Some("on") | Some("1") | Some("yes") | Some("true") => {
            state.set_echo(true);
        }
        Some("off") | Some("0") | Some("no") | Some("false") => {
            state.set_echo(false);
        }
        Some(v) => {
            return Err(anyhow!("Usage: .echo on|off (got: {})", v));
        }
        None => {
            println!("echo: {}", if state.echo { "on" } else { "off" });
        }
    }

    Ok(())
}

fn cmd_width(state: &mut CliState, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        return Err(anyhow!("Usage: .width NUM1 NUM2 ..."));
    }

    let mut widths = Vec::new();
    for arg in args {
        let w = arg.parse::<usize>().context("Invalid width")?;
        widths.push(w);
    }
    state.set_column_widths(widths);
    Ok(())
}

fn cmd_bail(state: &mut CliState, value: Option<&str>) -> Result<()> {
    match value {
        Some(v) => {
            if let Some(b) = parse_bool_arg(v) {
                state.set_bail(b);
            } else {
                return Err(anyhow!("Usage: .bail on|off (got: {})", v));
            }
        }
        None => {
            state.write_output(&format!("bail: {}", if state.bail { "on" } else { "off" }))?;
        }
    }
    Ok(())
}

pub fn parse_bool_arg(value: &str) -> Option<bool> {
    match value.to_lowercase().as_str() {
        "on" | "1" | "yes" | "true" => Some(true),
        "off" | "0" | "no" | "false" => Some(false),
        _ => None,
    }
}

pub fn parse_command(input: &str) -> Option<(&str, Vec<&str>)> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() || !parts[0].starts_with('.') {
        return None;
    }
    Some((parts[0], parts[1..].to_vec()))
}

#[cfg(test)]
mod tests;
