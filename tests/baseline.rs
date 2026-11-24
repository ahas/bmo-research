//! The schema's starting point (`docs/store.md`): a store with no schema takes the 1.0.0 baseline whole with
//! its three first categories, an install from the last release before 1.0.0 is read as it stands, and one of
//! an older version is refused.

use std::path::Path;

use research_server::store::{Store, schema_version};

use crate::support;

const BASELINE: &str = include_str!("../crates/server/src/baseline.sql");

// Every object of the store as SQLite keeps it, and its user version.
fn shape(path: &Path) -> (i64, Vec<(String, String)>) {
  let conn = rusqlite::Connection::open(path).unwrap();
  let version = conn.query_row("pragma user_version", [], |r| r.get(0)).unwrap();
  let objects = conn
    .prepare("select name, coalesce(sql, '') from sqlite_master where name not like 'sqlite_%' order by type, name")
    .unwrap()
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
    .unwrap()
    .collect::<rusqlite::Result<_>>()
    .unwrap();
  (version, objects)
}

// A store at `version` holding the baseline's schema and an item, as an install left it.
fn installed(dir: &Path, version: i64) {
  std::fs::create_dir_all(dir).unwrap();
  rusqlite::Connection::open(dir.join("research.db"))
    .unwrap()
    .execute_batch(&format!(
      "{BASELINE}
       insert into queue (category, key, name, kind, shape, asked, priority, state, found_at, created_at,
         changed_at) values ('it', 'example.org/a', 'A', 'model', 'manual', 1, 0, 'waiting', 1, 1, 1);
       pragma user_version = {version};"
    ))
    .unwrap();
}

#[test]
fn a_store_with_no_schema_takes_the_baseline_whole_with_its_first_categories() {
  let dir = support::scratch().join("research");
  drop(Store::open(&dir).unwrap());
  let laid = support::scratch().join("laid.db");
  rusqlite::Connection::open(&laid)
    .unwrap()
    .execute_batch(&format!("{BASELINE} pragma user_version = 20;"))
    .unwrap();

  assert_eq!(shape(&dir.join("research.db")), shape(&laid));
  assert_eq!(schema_version(), 20);
  let conn = rusqlite::Connection::open(dir.join("research.db")).unwrap();
  let names: String = conn
    .query_row(
      "select group_concat(name, ',') from (select name from categories order by name)",
      [],
      |r| r.get(0),
    )
    .unwrap();
  assert_eq!(names, "economy,it,security");
}

#[test]
fn an_install_at_the_last_release_before_the_baseline_is_read_as_it_stands() {
  let dir = support::scratch().join("research");
  installed(&dir, 20);
  let path = dir.join("research.db");
  let before = shape(&path);

  for _ in 0..2 {
    let store = Store::open(&dir).unwrap();
    assert_eq!(store.version().unwrap(), 20);
    assert!(store.categories().unwrap().is_empty(), "no category is seeded into an install");
    drop(store);
    assert_eq!(shape(&path), before);
    let names: String = rusqlite::Connection::open(&path)
      .unwrap()
      .query_row("select group_concat(name) from queue", [], |r| r.get(0))
      .unwrap();
    assert_eq!(names, "A");
  }
}

#[test]
fn an_install_short_of_the_last_release_before_the_baseline_is_refused() {
  let dir = support::scratch().join("research");
  installed(&dir, 19);
  let path = dir.join("research.db");
  let before = shape(&path);

  let refused = Store::open(&dir).err().expect("an older store is refused");
  assert!(refused.contains("version 19"), "{refused}");
  assert_eq!(shape(&path), before);
}
