use rusqlite::{types::ValueRef, Connection, Result};

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub rows_affected: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct TableColumn {
    pub cid: i64,
    pub name: String,
    pub type_: String,
    pub notnull: bool,
    pub dflt_value: Option<String>,
    pub pk: bool,
}

#[derive(Debug, Clone)]
pub struct TableIndex {
    pub seq: i64,
    pub name: String,
    pub unique: bool,
    pub origin: String,
    pub partial: bool,
}

#[derive(Debug, Clone)]
pub struct TableForeignKey {
    pub id: i64,
    pub seq: i64,
    pub table: String,
    pub from: String,
    pub to: String,
    pub on_update: String,
    pub on_delete: String,
    pub match_: String,
}

#[derive(Debug, Clone)]
pub struct SchemaInfo {
    pub columns: Vec<TableColumn>,
    pub create_sql: String,
}

/// Get list of all tables in the database
pub fn get_tables(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    let mut tables = Vec::new();
    for table in rows {
        tables.push(table?);
    }
    Ok(tables)
}

/// Execute a SQL query or command
pub fn execute_query(conn: &Connection, sql: &str) -> Result<QueryResult> {
    // Trim the SQL to check if it's actually empty
    let trimmed_sql = sql.trim();
    if trimmed_sql.is_empty() {
        return Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: None,
        });
    }

    let mut stmt = conn.prepare(sql)?;

    if stmt.column_count() > 0 {
        // It's a query that returns data
        let columns: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let col_count = columns.len();

        let mut rows_iter = stmt.query([])?;
        let mut result_rows = Vec::new();

        while let Some(row) = rows_iter.next()? {
            let mut row_data = Vec::with_capacity(col_count);
            for idx in 0..col_count {
                let val_ref = row.get_ref(idx)?;
                let val_str = value_to_string(val_ref);
                row_data.push(val_str);
            }
            result_rows.push(row_data);
        }

        Ok(QueryResult {
            columns,
            rows: result_rows,
            rows_affected: None,
        })
    } else {
        // It's a modification (UPDATE, DELETE, INSERT, etc.)
        drop(stmt);
        let affected = conn.execute(sql, [])?;
        Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: Some(affected),
        })
    }
}

/// Convert a SQLite value reference to a string for display
pub fn value_to_string(val_ref: ValueRef) -> String {
    match val_ref {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => f.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
        ValueRef::Blob(_) => "<BLOB>".to_string(),
    }
}

/// Get schema information for a table
pub fn get_schema(conn: &Connection, table: &str) -> Result<SchemaInfo> {
    // 1. Get Columns
    let mut stmt = conn.prepare(&format!("PRAGMA table_info('{}')", table))?;
    let rows = stmt.query_map([], |row| {
        Ok(TableColumn {
            cid: row.get(0)?,
            name: row.get(1)?,
            type_: row.get(2)?,
            notnull: row.get::<_, i64>(3)? != 0,
            dflt_value: row.get(4)?,
            pk: row.get::<_, i64>(5)? != 0,
        })
    })?;
    let mut columns = Vec::new();
    for col in rows {
        columns.push(col?);
    }

    // 2. Get Indexes
    let mut stmt = conn.prepare(&format!("PRAGMA index_list('{}')", table))?;
    let rows = stmt.query_map([], |row| {
        Ok(TableIndex {
            seq: row.get(0)?,
            name: row.get(1)?,
            unique: row.get::<_, i64>(2)? != 0,
            origin: row.get(3)?,
            partial: row.get::<_, i64>(4)? != 0,
        })
    })?;
    let mut indexes = Vec::new();
    for idx in rows {
        indexes.push(idx?);
    }

    // 3. Get Foreign Keys
    let mut stmt = conn.prepare(&format!("PRAGMA foreign_key_list('{}')", table))?;
    let rows = stmt.query_map([], |row| {
        Ok(TableForeignKey {
            id: row.get(0)?,
            seq: row.get(1)?,
            table: row.get(2)?,
            from: row.get(3)?,
            to: row.get(4)?,
            on_update: row.get(5)?,
            on_delete: row.get(6)?,
            match_: row.get(7)?,
        })
    })?;
    let mut foreign_keys = Vec::new();
    for fk in rows {
        foreign_keys.push(fk?);
    }

    // 4. Get Create SQL
    let mut stmt = conn.prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name=?1")?;
    let mut rows = stmt.query(rusqlite::params![table])?;
    let create_sql = if let Some(row) = rows.next()? {
        row.get::<_, String>(0)?
    } else {
        String::new()
    };

    Ok(SchemaInfo {
        columns,
        create_sql,
    })
}

#[cfg(test)]
mod tests;
