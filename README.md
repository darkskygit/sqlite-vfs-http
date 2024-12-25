# sqlite-vfs-http

The `sqlite-vfs-http` is a library based on the SQLite VFS extension, designed to access static SQLite files located on a CDN via HTTP/HTTPS protocol.

By using this library, you can host SQLite database files on a remote server and perform queries without downloading the files locally.

### Requirements

-   any crate that link SQLite3 to your binary, such as `rusqlite`, `sqlx` or `libsqlite3-sys`

### Usage

1. add the following to your `Cargo.toml`:

```toml
[dependencies]
sqlite-vfs-http = "0.1.0"
```

2. use the library in your code:

```rust
use rusqlite::{Connection, NO_PARAMS};
use sqlite_vfs_http::{register_http_vfs, HTTP_VFS};


// Register the HTTP VFS for sqlite
register_http_vfs();

let base = "https://example.com";
let conn = Connection::open_with_flags_and_vfs(
    format!("{base}/0.db"),
    OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    // Use HTTP VFS
    HTTP_VFS,
)?;
conn.query_row(
    "SELECT count(1) FROM sqlite_master WHERE type = 'table'",
    [], |row| row.get::<usize>(0)
).unwrap();
```

### Limitations

-   Before uploading to the CDN, the database needs to change the journal mode to `MEMORY`:

```sql
PRAGMA journal_mode = MEMORY;
```

### License

> This project is licensed under the AGPL-3.0 license.
