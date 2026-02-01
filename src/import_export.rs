use std::fs::File;

use rusqlite::{Connection, Result};

/// Generate SQL dump as a string (for .dump command)
pub fn generate_sql_dump(
    conn: &Connection,
    tables: Option<&[String]>,
    include_data: bool,
) -> Result<String> {
    let mut output = String::new();

    // Header
    output.push_str("PRAGMA foreign_keys=OFF;\n");
    output.push_str("BEGIN TRANSACTION;\n");

    // Get list of tables to export
    let table_list = if let Some(tables) = tables {
        tables.to_vec()
    } else {
        let mut stmt =
            conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut result = Vec::new();
        for table in rows {
            result.push(table?);
        }
        result
    };

    // Export each table
    for table_name in table_list {
        // Get CREATE statement
        let mut stmt =
            conn.prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name=?1")?;
        let create_sql: String = stmt.query_row([&table_name], |row| row.get(0))?;

        output.push_str(&format!("{};", create_sql));
        output.push('\n');

        if include_data {
            // Export data as INSERT statements
            let quoted_table = format!("\"{}\"", table_name.replace('"', "\"\""));
            let select_sql = format!("SELECT * FROM {}", quoted_table);
            let mut stmt = conn.prepare(&select_sql)?;
            let column_names: Vec<String> = stmt
                .column_names()
                .into_iter()
                .map(|s| s.to_string())
                .collect();

            let col_count = column_names.len();
            let mut rows = stmt.query([])?;

            while let Some(row) = rows.next()? {
                let mut values = Vec::new();

                for idx in 0..col_count {
                    let val_ref = row.get_ref(idx)?;
                    let val_str = match val_ref {
                        rusqlite::types::ValueRef::Null => "NULL".to_string(),
                        rusqlite::types::ValueRef::Integer(i) => i.to_string(),
                        rusqlite::types::ValueRef::Real(f) => f.to_string(),
                        rusqlite::types::ValueRef::Text(t) => {
                            format!("'{}'", String::from_utf8_lossy(t).replace('\'', "''"))
                        }
                        rusqlite::types::ValueRef::Blob(_) => "X''".to_string(),
                    };
                    values.push(val_str);
                }

                output.push_str(&format!(
                    "INSERT INTO {} VALUES ({});\n",
                    quoted_table,
                    values.join(", ")
                ));
            }
        }
    }

    // Export indexes
    let mut stmt = conn.prepare(
        "SELECT sql FROM sqlite_master WHERE type='index' AND sql IS NOT NULL ORDER BY name",
    )?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let index_sql: String = row.get(0)?;
        output.push_str(&format!("{};\n", index_sql));
    }

    output.push_str("COMMIT;\n");

    Ok(output)
}

/// Import CSV file into a table
pub fn import_csv(conn: &Connection, file_path: &str, table: &str) -> Result<()> {
    use csv::ReaderBuilder;

    let file =
        File::open(file_path).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);

    // Get headers
    let headers = reader
        .headers()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let column_names: Vec<String> = headers.iter().map(|s| s.to_string()).collect();

    // Begin transaction
    conn.execute("BEGIN TRANSACTION", [])?;

    // Import each row
    for result in reader.records() {
        let record = result.map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        // Build INSERT statement
        let placeholders: Vec<&str> = (0..record.len()).map(|_| "?").collect();
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table,
            column_names.join(", "),
            placeholders.join(", ")
        );

        // Convert record to params
        let params: Vec<&str> = record.iter().collect();
        conn.execute(&sql, rusqlite::params_from_iter(params))?;
    }

    // Commit transaction
    conn.execute("COMMIT", [])?;

    Ok(())
}

#[cfg(test)]
mod tests;
