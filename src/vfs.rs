use super::*;
use rand::{thread_rng, Rng};
use sqlite_vfs::{OpenKind, OpenOptions, Vfs};
use std::{
    io::{Error, ErrorKind},
    time::Duration,
};

pub const HTTP_VFS: &str = "http";

#[derive(Default)]
pub struct HttpVfs;

impl Vfs for HttpVfs {
    type Handle = Connection;

    fn open(&self, db: &str, opts: OpenOptions) -> Result<Self::Handle, Error> {
        if opts.kind != OpenKind::MainDb {
            return Err(Error::new(
                ErrorKind::ReadOnlyFilesystem,
                "only main database supported",
            ));
        }

        Ok(Connection::new(db)?)
    }

    fn delete(&self, _db: &str) -> Result<(), Error> {
        Err(Error::new(
            ErrorKind::ReadOnlyFilesystem,
            "delete operation is not supported",
        ))
    }

    fn exists(&self, _db: &str) -> Result<bool, Error> {
        Ok(false)
    }

    fn temporary_name(&self) -> String {
        String::from("main.db")
    }

    fn random(&self, buffer: &mut [i8]) {
        Rng::fill(&mut thread_rng(), buffer);
    }

    fn sleep(&self, duration: Duration) -> Duration {
        std::thread::sleep(duration);
        duration
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;

    use super::*;
    use rusqlite::{Connection, OpenFlags};
    use tokio::time::sleep;

    const QUERY_SQLITE_MASTER: &str = "SELECT count(1) FROM sqlite_master WHERE type = 'table'";
    const QUERY_TEST: &str = "SELECT name FROM test";

    mod server {
        use rocket::{custom, figment::Figment, get, routes, Config, Shutdown, State};
        use rocket_seek_stream::SeekStream;
        use rusqlite::Connection;
        use std::{collections::HashMap, fs::read, io::Cursor, thread::JoinHandle};
        use tempfile::tempdir;
        use tokio::runtime::Runtime;

        fn init_database() -> HashMap<i64, Vec<u8>> {
            let schemas = [
                vec![
                    "PRAGMA journal_mode = MEMORY;",
                    "CREATE TABLE test1 (id INTEGER PRIMARY KEY, name TEXT);",
                    "CREATE TABLE test2 (id INTEGER PRIMARY KEY, name TEXT);",
                ],
                vec![
                    "PRAGMA journal_mode = MEMORY;",
                    "CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT);",
                    "INSERT INTO test (name) VALUES ('Alice');",
                    "INSERT INTO test (name) VALUES ('Bob');",
                ],
            ];
            let mut database = HashMap::new();

            let temp = tempdir().unwrap();

            for (i, schema) in schemas.into_iter().enumerate() {
                let path = temp.path().join(format!("{i}.db"));
                let conn = Connection::open(&path).unwrap();
                conn.execute_batch(&schema.join("\n")).unwrap();
                conn.close().unwrap();
                database.insert(i as i64, read(&path).unwrap());
            }

            database
        }

        #[get("/<id>")]
        pub async fn database(
            db: &State<HashMap<i64, Vec<u8>>>,
            id: i64,
        ) -> Option<SeekStream<'static>> {
            if let Some(buffer) = db.get(&id) {
                let cursor = Cursor::new(buffer.clone());
                Some(SeekStream::with_opts(cursor, buffer.len() as u64, None))
            } else {
                None
            }
        }

        #[get("/shutdown")]
        pub async fn shutdown(shutdown: Shutdown) -> &'static str {
            shutdown.notify();
            "Shutting down..."
        }

        pub fn launch() -> JoinHandle<Result<(), rocket::Error>> {
            std::thread::spawn(|| {
                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    custom(Figment::from(Config::default()).merge(("port", 4096)))
                        .manage(init_database())
                        .mount("/", routes![database, shutdown])
                        .launch()
                        .await?;

                    Ok(())
                })
            })
        }
    }

    async fn init_server<C, F>(future: C) -> anyhow::Result<()>
    where
        C: FnOnce(String) -> F,
        F: Future<Output = anyhow::Result<()>>,
    {
        let base = "http://127.0.0.1:4096";
        let server = server::launch();

        // wait for server to start
        loop {
            let resp = reqwest::get(base).await;
            if let Ok(resp) = resp {
                if resp.status() == 404 {
                    break;
                }
            }
            sleep(Duration::from_millis(100)).await;
        }

        future(base.into()).await?;

        reqwest::get(format!("{base}/shutdown").as_str()).await?;
        server.join().unwrap()?;

        Ok(())
    }

    #[tokio::test]
    async fn test_http_vfs() {
        init_server(|base| async move {
            vfs::register_http_vfs();

            {
                let conn = Connection::open_with_flags_and_vfs(
                    format!("{base}/0"),
                    OpenFlags::SQLITE_OPEN_READ_WRITE
                        | OpenFlags::SQLITE_OPEN_CREATE
                        | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                    HTTP_VFS,
                )?;
                assert_eq!(
                    conn.query_row::<usize, _, _>(QUERY_SQLITE_MASTER, [], |row| row.get(0))?,
                    2
                );
            }

            {
                let conn = Connection::open_with_flags_and_vfs(
                    format!("{base}/1"),
                    OpenFlags::SQLITE_OPEN_READ_WRITE
                        | OpenFlags::SQLITE_OPEN_CREATE
                        | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                    HTTP_VFS,
                )?;
                let mut stmt = conn.prepare(QUERY_TEST)?;
                assert_eq!(
                    stmt.query_map([], |row| row.get::<_, String>(0))?
                        .collect::<Result<Vec<_>, _>>()?,
                    vec!["Alice".to_string(), "Bob".to_string()]
                );
            }

            Ok(())
        })
        .await
        .unwrap();
    }
}
