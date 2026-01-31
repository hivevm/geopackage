use std::env;
use std::path::PathBuf;

fn main() {
    let sqlite_dir = PathBuf::from("../sqlite");

    cc::Build::new()
        .file(sqlite_dir.join("sqlite3.c"))
        .include(&sqlite_dir)
        // .flag("-DSQLITE_ENABLE_FTS5")   // Full-text search
        // .flag("-DSQLITE_ENABLE_JSON1")  // JSON functions
        // .flag("-DSQLITE_ENABLE_RTREE")  // R*Tree index
        // .flag("-DSQLITE_ENABLE_COLUMN_METADATA")    // Column metadata
        // .flag("-DSQLITE_ENABLE_MATH_FUNCTIONS")// Math functions
        // .flag("-DSQLITE_THREADSAFE=1")  // Thread-safe mode
        // .define("SQLITE_ENABLE_LOAD_EXTENSION", None)
        // .define("SQLITE_THREADSAFE", Some("1"))
        .define("SQLITE_ENABLE_FTS5", None)
        .define("SQLITE_ENABLE_JSON1", None)
        .compile("sqlite3");

    // Compile extension
    // println!("cargo:rustc-link-lib=static=sqlite3");
    println!("cargo:rerun-if-changed=vendor/sqlite/sqlite3.c");
    println!("cargo:rerun-if-changed=vendor/sqlite/sqlite3.h");
}