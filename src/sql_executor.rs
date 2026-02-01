use std::time::Instant;

use anyhow::Result;
use rusqlite::Connection;

use crate::cli_state::CliState;
use crate::db;
use crate::output;

/// Execute a SQL statement and display the results
pub fn execute(conn: &Connection, sql: &str, state: &mut CliState) -> Result<()> {
    let start = if state.timer {
        Some(Instant::now())
    } else {
        None
    };

    // Execute the query
    let result = db::execute_query(conn, sql)?;

    // If it's a query with results, format and display
    if !result.columns.is_empty() {
        let output_str = output::format_result(&result, state)?;
        state.write_output(&output_str)?;
    } else if let Some(affected) = result.rows_affected {
        // For INSERT/UPDATE/DELETE, show rows affected
        if affected > 0 {
            // Only show message if rows were actually affected
            // sqlite3 CLI doesn't print anything for successful modifications
        }
    }

    // Show timer if enabled
    if let Some(start_time) = start {
        let elapsed = start_time.elapsed();
        eprintln!("Run Time: real {:.3}", elapsed.as_secs_f64());
    }

    Ok(())
}
