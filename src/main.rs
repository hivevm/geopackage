use rusqlite::{Connection, Result, ffi, params};
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
//use libsqlite3_sys as ffi;

// Callback-Funktion für eine benutzerdefinierte SQL-Funktion
unsafe extern "C" fn my_function(
    context: *mut ffi::sqlite3_context,
    argc: c_int,
    argv: *mut *mut ffi::sqlite3_value,
) {
    if argc != 2 {
        let err = CString::new("Expected 2 arguments").unwrap();
        ffi::sqlite3_result_error(context, err.as_ptr(), -1);
        return;
    }

    let arg1 = ffi::sqlite3_value_int(*argv.offset(0));
    let arg2 = ffi::sqlite3_value_int(*argv.offset(1));
    
    let result = arg1 + arg2;
    ffi::sqlite3_result_int(context, result);
}

unsafe extern "C" fn my_number(
    ctx: *mut ffi::sqlite3_context,
    _argc: c_int,
    _argv: *mut *mut ffi::sqlite3_value,
) {
    ffi::sqlite3_result_int64(ctx, 42);
}

fn main() -> Result<()> {
    // Create an in-memory database or file-based database
    let conn = Connection::open_in_memory()?;
    // let conn = Connection::open("my_database.db")?;
    
    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    // conn.execute("PRAGMA journal_mode = WAL", [])?;  // Write-Ahead Logging
    // conn.execute("PRAGMA synchronous = NORMAL", [])?;
    // conn.execute("PRAGMA cache_size = -64000", [])?;  // 64MB cache

    // unsafe {
    //     conn.load_extension_enable()?;
    //     conn.load_extension(
    //         "./target/release/libgpkg_lib",
    //         Some("sqlite3_extension_init")  // Explicitly specify the entry point
    //     )?;
    //     conn.load_extension_disable()?;
    // }

    // Register function directly - no .so file needed!
    unsafe {
        let name = CString::new("my_number").unwrap();
        ffi::sqlite3_create_function_v2(
            conn.handle(),
            name.as_ptr(),
            0,
            ffi::SQLITE_UTF8,
            ptr::null_mut(),
            Some(my_number),
            None, None, None,
        );

        let fn_name = CString::new("add_numbers").unwrap();
        ffi::sqlite3_create_function_v2(
            conn.handle(),
            fn_name.as_ptr(),
            2,  // Anzahl der Argumente
            ffi::SQLITE_UTF8 | ffi::SQLITE_DETERMINISTIC,
            std::ptr::null_mut(),
            Some(my_function),
            None,
            None,
            None,
        );
    }

    // Create tables
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            content TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id)
        )",
        [],
    )?;
    
    // Insert a user
    conn.execute(
        "INSERT INTO users (username, email) VALUES (?1, ?2)",
        params!["alice", "alice@example.com"],
    )?;
    
    let user_id = conn.last_insert_rowid();
    
    // Insert a post
    conn.execute(
        "INSERT INTO posts (user_id, title, content) VALUES (?1, ?2, ?3)",
        params![user_id, "My First Post", "Hello, SQLite with Rust!"],
    )?;
    
    // Query with joins
    let mut stmt = conn.prepare(
        "SELECT u.username, p.title, p.content, p.created_at 
         FROM posts p 
         JOIN users u ON p.user_id = u.id"
    )?;
    
    let posts = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    
    println!("Posts:");
    for post in posts {
        let (username, title, content, created_at) = post?;
        println!("  [{}] {} by {}: {}", created_at, title, username, content);
    }
    
    let result: i64 = conn.query_row("SELECT my_number()", [], |row| row.get(0))?;
    println!("{}", result);
    
    // Transaction example
    conn.execute_batch(
        "BEGIN;
         UPDATE users SET email = 'newemail@example.com' WHERE id = 1;
         COMMIT;"
    )?;    // Insert a post


    let result: i64 = conn.query_row(
        "SELECT add_numbers(?1, ?2)", 
        params![1, 5], |row| row.get(0))?;
    println!("{}", result);  // Prints: 6

    // Use transactions for bulk inserts:
    // let tx = conn.transaction()?;
    // for i in 0..1000 {
    //     tx.execute("INSERT INTO data (value) VALUES (?1)", [i])?;
    // }
    // tx.commit()?;

    // // Use transactions for bulk inserts:
    // let mut stmt = conn.prepare("INSERT INTO data (value) VALUES (?1)")?;
    // for i in 0..1000 {
    //     stmt.execute([i])?;
    // }
    
    Ok(())
}