//! SQLite: one file in the data directory, WAL, migrations embedded in the
//! binary and applied on open. Until v0.1.0 is tagged, migrations may be
//! edited in place; after that they're append-only.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

/// In order. The schema version is the number applied, kept in
/// `PRAGMA user_version`.
const MIGRATIONS: &[&str] =
    &[include_str!("../migrations/0001_init.sql"), include_str!("../migrations/0002_provider_checks.sql")];

pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("the database is at schema version {found}, newer than this kizn ({supported}); update kizn")]
    TooNew { found: u32, supported: u32 },
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// What's in a database file, read-only: for `just inspect db`.
pub struct Inspection {
    pub schema_version: u32,
    /// Table name and row count.
    pub tables: Vec<(String, i64)>,
}

/// Reads a database without migrating or writing anything.
pub fn inspect(path: &Path) -> Result<Inspection> {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut tables = Vec::new();
    for name in names {
        let n = conn.query_row(&format!("SELECT count(*) FROM \"{}\"", name.replace('"', "\"\"")), [], |r| r.get(0))?;
        tables.push((name, n));
    }
    Ok(Inspection { schema_version: user_version(&conn)?, tables })
}

pub struct CheckRow {
    pub provider: String,
    pub fingerprint: String,
    pub tested_at: String,
    pub check_json: String,
}

pub struct TopicRow {
    pub id: String,
    pub title: String,
}

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Store> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut conn)?;
        Ok(Store { conn: Mutex::new(conn) })
    }

    pub fn schema_version(&self) -> Result<u32> {
        user_version(&self.conn.lock().unwrap())
    }

    pub fn topics(&self) -> Result<Vec<TopicRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, title FROM topics ORDER BY created_at, id")?;
        let rows = stmt.query_map([], |r| Ok(TopicRow { id: r.get(0)?, title: r.get(1)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

impl Store {
    pub fn save_check(&self, row: &CheckRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO provider_checks (provider, fingerprint, tested_at, check_json) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (provider) DO UPDATE SET fingerprint = ?2, tested_at = ?3, check_json = ?4",
            (&row.provider, &row.fingerprint, &row.tested_at, &row.check_json),
        )?;
        Ok(())
    }

    pub fn checks(&self) -> Result<Vec<CheckRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT provider, fingerprint, tested_at, check_json FROM provider_checks")?;
        let rows = stmt.query_map([], |r| {
            Ok(CheckRow { provider: r.get(0)?, fingerprint: r.get(1)?, tested_at: r.get(2)?, check_json: r.get(3)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_check(&self, provider: &str) -> Result<()> {
        self.conn.lock().unwrap().execute("DELETE FROM provider_checks WHERE provider = ?1", [provider])?;
        Ok(())
    }
}

fn user_version(conn: &Connection) -> Result<u32> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let found = user_version(conn)?;
    if found > SCHEMA_VERSION {
        return Err(StoreError::TooNew { found, supported: SCHEMA_VERSION });
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(found as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as u32 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_file_is_migrated_and_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kizn.db");
        let store = Store::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        assert!(store.topics().unwrap().is_empty());
        drop(store);
        // Reopening applies nothing twice.
        assert_eq!(Store::open(&path).unwrap().schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn inspect_reads_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kizn.db");
        drop(Store::open(&path).unwrap());
        let i = inspect(&path).unwrap();
        assert_eq!(i.schema_version, SCHEMA_VERSION);
        assert_eq!(i.tables, vec![("provider_checks".to_string(), 0), ("topics".to_string(), 0)]);
    }

    #[test]
    fn checks_upsert_by_provider() {
        let store = Store::open_in_memory().unwrap();
        let row = |fp: &str| CheckRow {
            provider: "local".into(),
            fingerprint: fp.into(),
            tested_at: "2026-10-25 09:00:00".into(),
            check_json: "{}".into(),
        };
        store.save_check(&row("a")).unwrap();
        store.save_check(&row("b")).unwrap();
        let checks = store.checks().unwrap();
        assert_eq!((checks.len(), checks[0].fingerprint.as_str()), (1, "b"));
        store.delete_check("local").unwrap();
        assert!(store.checks().unwrap().is_empty());
    }

    #[test]
    fn a_newer_database_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kizn.db");
        Connection::open(&path).unwrap().pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(Store::open(&path), Err(StoreError::TooNew { found: 99, .. })));
    }
}
