use anyhow::Result;

use crate::cli_state::{CliState, OutputMode};
use crate::db::QueryResult;

/// Format a query result according to the current output mode
pub fn format_result(result: &QueryResult, state: &CliState) -> Result<String> {
    match state.output_mode {
        OutputMode::List => format_list(result, state),
        OutputMode::Csv => format_csv(result, state),
        OutputMode::Column => format_column(result, state),
        OutputMode::Line => format_line(result, state),
        OutputMode::Json => format_json(result, state),
        OutputMode::Jsonl => format_jsonl(result, state),
        OutputMode::Table => format_table(result, state),
        OutputMode::Markdown => format_markdown(result, state),
    }
}

/// Format as pipe-separated values (default)
fn format_list(result: &QueryResult, state: &CliState) -> Result<String> {
    let mut output = String::new();
    let sep = &state.separator;
    let null_val = &state.null_value;

    // Headers if enabled
    if state.show_headers {
        output.push_str(&result.columns.join(sep));
        output.push('\n');
    }

    // Rows
    for row in &result.rows {
        let formatted_row: Vec<String> = row
            .iter()
            .map(|cell| {
                if cell == "NULL" {
                    null_val.clone()
                } else {
                    cell.clone()
                }
            })
            .collect();
        output.push_str(&formatted_row.join(sep));
        output.push('\n');
    }

    Ok(output.trim_end().to_string())
}

/// Format as CSV
fn format_csv(result: &QueryResult, state: &CliState) -> Result<String> {
    let mut output = String::new();
    let null_val = &state.null_value;

    // Helper to escape CSV values
    let escape_csv = |s: &str| -> String {
        if s.contains('"') || s.contains(',') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };

    // Headers if enabled
    if state.show_headers {
        let headers: Vec<String> = result.columns.iter().map(|h| escape_csv(h)).collect();
        output.push_str(&headers.join(","));
        output.push('\n');
    }

    // Rows
    for row in &result.rows {
        let formatted_row: Vec<String> = row
            .iter()
            .map(|cell| {
                let val = if cell == "NULL" { null_val } else { cell };
                escape_csv(val)
            })
            .collect();
        output.push_str(&formatted_row.join(","));
        output.push('\n');
    }

    Ok(output.trim_end().to_string())
}

/// Format as aligned columns
fn format_column(result: &QueryResult, state: &CliState) -> Result<String> {
    if result.rows.is_empty() {
        return Ok(String::new());
    }

    let null_val = &state.null_value;

    // Calculate column widths
    let mut widths: Vec<usize> = result.columns.iter().map(|h| h.len()).collect();

    for row in &result.rows {
        for (i, cell) in row.iter().enumerate() {
            let val = if cell == "NULL" { null_val } else { cell };
            if i < widths.len() {
                widths[i] = widths[i].max(val.len());
            } else {
                // This shouldn't happen if result is consistent, but safeguard
                widths.push(val.len());
            }
        }
    }

    // Apply manual widths from state
    for (i, &width) in state.column_widths.iter().enumerate() {
        if i < widths.len() && width > 0 {
            widths[i] = width;
        }
    }

    let mut output = String::new();

    // Headers if enabled
    if state.show_headers {
        for (i, header) in result.columns.iter().enumerate() {
            if i > 0 {
                output.push_str("  ");
            }
            let w = widths[i];
            let content = if header.len() > w {
                // Truncate if fixed width is smaller
                &header[..w]
            } else {
                header
            };
            output.push_str(&format!("{:<width$}", content, width = w));
        }
        output.push('\n');

        // Separator line
        for (i, &width) in widths.iter().enumerate() {
            if i > 0 {
                output.push_str("  ");
            }
            output.push_str(&"-".repeat(width));
        }
        output.push('\n');
    }

    // Rows
    for row in &result.rows {
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                output.push_str("  ");
            }
            let val = if cell == "NULL" { null_val } else { cell };
            let w = widths[i];
            let content = if val.len() > w { &val[..w] } else { val };
            output.push_str(&format!("{:<width$}", content, width = w));
        }
        output.push('\n');
    }

    Ok(output.trim_end().to_string())
}

/// Format as one value per line
fn format_line(result: &QueryResult, state: &CliState) -> Result<String> {
    let mut output = String::new();
    let null_val = &state.null_value;

    // Find the longest column name for alignment
    let max_col_len = result.columns.iter().map(|c| c.len()).max().unwrap_or(0);

    for (row_idx, row) in result.rows.iter().enumerate() {
        if row_idx > 0 {
            output.push('\n');
        }

        for (col, cell) in result.columns.iter().zip(row.iter()) {
            let val = if cell == "NULL" { null_val } else { cell };
            output.push_str(&format!("{:>width$} = {}\n", col, val, width = max_col_len));
        }
    }

    Ok(output.trim_end().to_string())
}

/// Format as JSON array
fn format_json(result: &QueryResult, state: &CliState) -> Result<String> {
    use serde_json::json;

    let null_val = &state.null_value;

    let rows: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (col, cell) in result.columns.iter().zip(row.iter()) {
                let val = if cell == "NULL" {
                    if null_val.is_empty() {
                        serde_json::Value::Null
                    } else {
                        json!(null_val)
                    }
                } else {
                    json!(cell)
                };
                obj.insert(col.clone(), val);
            }
            serde_json::Value::Object(obj)
        })
        .collect();

    let json = serde_json::to_string_pretty(&rows)?;
    Ok(json)
}

/// Format as JSON Lines (one JSON object per line)
fn format_jsonl(result: &QueryResult, state: &CliState) -> Result<String> {
    use serde_json::json;

    let mut output = String::new();
    let null_val = &state.null_value;

    for row in &result.rows {
        let mut obj = serde_json::Map::new();
        for (col, cell) in result.columns.iter().zip(row.iter()) {
            let val = if cell == "NULL" {
                if null_val.is_empty() {
                    serde_json::Value::Null
                } else {
                    json!(null_val)
                }
            } else {
                json!(cell)
            };
            obj.insert(col.clone(), val);
        }
        let json_line = serde_json::to_string(&obj)?;
        output.push_str(&json_line);
        output.push('\n');
    }

    Ok(output.trim_end().to_string())
}

/// Format as table with box-drawing characters
fn format_table(result: &QueryResult, state: &CliState) -> Result<String> {
    if result.rows.is_empty() {
        return Ok(String::new());
    }

    let null_val = &state.null_value;

    // Calculate column widths with terminal size awareness
    let _term_width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80);

    let mut widths: Vec<usize> = result.columns.iter().map(|h| h.len()).collect();

    for row in &result.rows {
        for (i, cell) in row.iter().enumerate() {
            let val = if cell == "NULL" { null_val } else { cell };
            widths[i] = widths[i].max(val.len());
        }
    }

    // Limit column widths to prevent overflow
    let max_col_width = 50;
    for width in &mut widths {
        *width = (*width).min(max_col_width);
    }

    let mut output = String::new();

    // Use color if enabled
    let use_color = state.color_enabled;

    // Top border
    output.push_str("┌");
    for (i, &width) in widths.iter().enumerate() {
        if i > 0 {
            output.push_str("┬");
        }
        output.push_str(&"─".repeat(width + 2));
    }
    output.push_str("┐\n");

    // Headers if enabled
    if state.show_headers {
        output.push_str("│");
        for (i, header) in result.columns.iter().enumerate() {
            if i > 0 {
                output.push_str("│");
            }
            let truncated = truncate_string(header, widths[i]);
            let header_text = if use_color {
                format!(" {} ", colorize_header(&truncated, widths[i]))
            } else {
                format!(" {:<width$} ", truncated, width = widths[i])
            };
            output.push_str(&header_text);
        }
        output.push_str("│\n");

        // Header separator
        output.push_str("├");
        for (i, &width) in widths.iter().enumerate() {
            if i > 0 {
                output.push_str("┼");
            }
            output.push_str(&"─".repeat(width + 2));
        }
        output.push_str("┤\n");
    }

    // Rows with alternating colors
    for (row_idx, row) in result.rows.iter().enumerate() {
        output.push_str("│");
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                output.push_str("│");
            }
            let val = if cell == "NULL" { null_val } else { cell };
            let truncated = truncate_string(val, widths[i]);

            // Colorize functions handle padding internally to avoid ANSI code width issues
            let cell_text = if use_color && cell == "NULL" {
                format!(" {} ", colorize_null(&truncated, widths[i]))
            } else if use_color && row_idx % 2 == 1 {
                format!(" {} ", colorize_alt_row(&truncated, widths[i]))
            } else {
                format!(" {:<width$} ", truncated, width = widths[i])
            };
            output.push_str(&cell_text);
        }
        output.push_str("│\n");
    }

    // Bottom border
    output.push_str("└");
    for (i, &width) in widths.iter().enumerate() {
        if i > 0 {
            output.push_str("┴");
        }
        output.push_str(&"─".repeat(width + 2));
    }
    output.push_str("┘");

    // Add row count footer
    if use_color {
        output.push_str(&format!(
            "\n{}",
            colorize_footer(&format!(
                "({} row{})",
                result.rows.len(),
                if result.rows.len() == 1 { "" } else { "s" }
            ))
        ));
    } else {
        output.push_str(&format!(
            "\n({} row{})",
            result.rows.len(),
            if result.rows.len() == 1 { "" } else { "s" }
        ));
    }

    Ok(output)
}

/// Truncate string to fit width, adding ellipsis if needed
fn truncate_string(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else if max_width <= 3 {
        s.chars().take(max_width).collect()
    } else {
        let mut result: String = s.chars().take(max_width - 3).collect();
        result.push_str("...");
        result
    }
}

/// Colorize header text
fn colorize_header(text: &str, width: usize) -> String {
    use nu_ansi_term::Color;
    // Pad FIRST, then colorize to avoid ANSI codes affecting width
    let padded = format!("{:<width$}", text, width = width);
    Color::Cyan.bold().paint(padded).to_string()
}

/// Colorize NULL values
fn colorize_null(text: &str, width: usize) -> String {
    use nu_ansi_term::Color;
    // Pad FIRST, then colorize to avoid ANSI codes affecting width
    let padded = format!("{:<width$}", text, width = width);
    Color::DarkGray.italic().paint(padded).to_string()
}

/// Colorize alternating rows
fn colorize_alt_row(text: &str, width: usize) -> String {
    use nu_ansi_term::Color;
    // Pad FIRST, then colorize to avoid ANSI codes affecting width
    let padded = format!("{:<width$}", text, width = width);
    Color::White.dimmed().paint(padded).to_string()
}

/// Colorize footer
fn colorize_footer(text: &str) -> String {
    use nu_ansi_term::Color;
    Color::Green.dimmed().paint(text).to_string()
}

/// Format as Markdown table
fn format_markdown(result: &QueryResult, state: &CliState) -> Result<String> {
    if result.rows.is_empty() {
        return Ok(String::new());
    }

    let null_val = &state.null_value;

    let mut output = String::new();

    // Headers (always shown in markdown)
    output.push_str("| ");
    output.push_str(&result.columns.join(" | "));
    output.push_str(" |\n");

    // Separator
    output.push_str("|");
    for _ in &result.columns {
        output.push_str(" --- |");
    }
    output.push('\n');

    // Rows
    for row in &result.rows {
        output.push_str("| ");
        let formatted_row: Vec<String> = row
            .iter()
            .map(|cell| {
                let val = if cell == "NULL" { null_val } else { cell };
                val.replace('|', "\\|")
            })
            .collect();
        output.push_str(&formatted_row.join(" | "));
        output.push_str(" |\n");
    }

    Ok(output.trim_end().to_string())
}

#[cfg(test)]
mod tests;
