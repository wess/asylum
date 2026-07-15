use super::*;

/// Current schema version = number of migration steps.
fn latest() -> i64 {
    MIGRATIONS.len() as i64
}

fn user_version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap()
}

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        [name],
        |_| Ok(()),
    )
    .is_ok()
}

/// Build an in-memory database whose real schema is exactly historical version
/// `k`: apply the first `k` migration steps and stamp `user_version = k`.
fn db_at_version(k: usize) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    for step in &MIGRATIONS[..k] {
        conn.execute_batch(step).unwrap();
    }
    conn.execute_batch(&format!("PRAGMA user_version = {k}"))
        .unwrap();
    conn
}

#[test]
fn fresh_database_reaches_latest_version() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    assert_eq!(user_version(&conn), latest());
    for t in [
        "projects",
        "tasks",
        "runs",
        "followups",
        "controlrequests",
        "events",
    ] {
        assert!(table_exists(&conn, t), "missing table {t}");
    }
}

#[test]
fn every_historical_version_upgrades_to_latest() {
    // Opening a database left at any prior version must bring it fully current.
    for k in 0..=MIGRATIONS.len() {
        let conn = db_at_version(k);
        migrate(&conn).unwrap();
        assert_eq!(user_version(&conn), latest(), "upgrading from v{k}");
        assert!(
            table_exists(&conn, "events"),
            "v{k} did not reach latest schema"
        );
    }
}

#[test]
fn migrate_is_idempotent_when_already_current() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    // A second pass changes nothing and must not error (no step re-runs).
    migrate(&conn).unwrap();
    assert_eq!(user_version(&conn), latest());
}

#[test]
fn a_failed_migration_rolls_back_atomically() {
    // A step whose second statement fails must leave neither the created table
    // nor a bumped version behind: the whole step rolls back.
    let conn = db_at_version(0);
    let broken = "CREATE TABLE half_applied (x INTEGER);
                  INSERT INTO does_not_exist VALUES (1);";
    let err = apply_migration(&conn, broken, 1);
    assert!(err.is_err(), "broken migration should fail");
    assert_eq!(
        user_version(&conn),
        0,
        "version must not advance on failure"
    );
    assert!(
        !table_exists(&conn, "half_applied"),
        "partial DDL must be rolled back"
    );
}

#[test]
fn restart_after_a_failed_migration_recovers() {
    // Simulate a crash mid-migration (rolled back to the old version), then a
    // normal restart: the next migrate() applies the real steps cleanly.
    let conn = db_at_version(0);
    let broken = "CREATE TABLE half (x); INSERT INTO nope VALUES (1);";
    let _ = apply_migration(&conn, broken, 1);
    assert_eq!(user_version(&conn), 0);
    // Restart path: run the real migrations.
    migrate(&conn).unwrap();
    assert_eq!(user_version(&conn), latest());
    assert!(table_exists(&conn, "projects"));
}

#[test]
fn foreign_keys_enforced_after_open() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let fk: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1, "foreign key enforcement must remain on");
}
