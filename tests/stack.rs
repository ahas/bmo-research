//! The platform's recorded stack (`docs/design.md`, 「The age floor」 and `docs/api.md`, 「The questions」): the
//! floor the oldest recorded component gives, an advisory older than it skipped while a newer one is not, an
//! empty or unknown inventory skipping nothing, the block and the floor line an advisory's question carries,
//! and the deploy records the inventory reads.

use crate::support;

use research_server::config::Settings;
use research_server::stack::{self, AgeFloor, Recorded};

/// One recorded component, as a registration or a deployment leaves it.
fn recorded(component: &str, version: Option<&str>, at: i64) -> Recorded {
  Recorded {
    component: component.into(),
    version: version.map(str::to_string),
    place: None,
    repository: None,
    worker: None,
    at: Some(at),
  }
}

#[test]
fn an_advisory_older_than_the_oldest_recorded_component_is_stale_and_a_newer_one_is_not() {
  let rows = [
    recorded("Node.js", Some("22.20.0"), 1_000),
    recorded("Node.js", Some("24.22.2"), 2_000),
    recorded("OpenSSL", Some("3.5.2"), 1_500),
  ];
  let floor = stack::age_floor(&rows).expect("the oldest recorded component gives a floor");
  assert_eq!(
    floor,
    AgeFloor {
      component: "Node.js".into(),
      version: "22.20.0".into(),
      at: 1_000
    }
  );

  assert!(
    stack::stale(Some(999), Some(&floor)),
    "an advisory from before the oldest build is skipped"
  );
  assert!(
    !stack::stale(Some(1_000), Some(&floor)),
    "an advisory from that same moment is not older"
  );
  assert!(!stack::stale(Some(2_001), Some(&floor)), "a newer advisory is not skipped");
  assert!(!stack::stale(None, Some(&floor)), "an advisory with no date is never skipped on age");
}

#[test]
fn an_empty_or_unknown_inventory_skips_nothing() {
  assert_eq!(stack::age_floor(&[]), None, "an empty inventory gives no floor");
  assert!(!stack::stale(Some(0), None), "no floor skips nothing");

  for unknown in [None, Some(""), Some("unknown"), Some("UNKNOWN")] {
    let rows = [recorded("Node.js", unknown, 1_000), recorded("Node.js", Some("24.22.2"), 2_000)];
    assert_eq!(
      stack::age_floor(&rows),
      None,
      "one unrecorded version leaves the floor unknown: {unknown:?}"
    );
    assert!(!stack::stale(Some(0), stack::age_floor(&rows).as_ref()));
  }
}

#[test]
fn the_block_names_every_row_and_writes_an_unrecorded_version_unknown() {
  let rows = [
    Recorded {
      component: "libexample".into(),
      version: Some("1.4.1".into()),
      place: None,
      repository: Some("services/alpha".into()),
      worker: Some("worker-a".into()),
      at: Some(1_000),
    },
    Recorded {
      component: "openssl".into(),
      version: None,
      place: Some("node-a".into()),
      repository: None,
      worker: None,
      at: Some(2_000),
    },
  ];
  let block = stack::block(&rows);
  assert!(block.contains("- libexample 1.4.1 (services/alpha)"), "{block}");
  assert!(block.contains("- openssl unknown (node-a)"), "{block}");
  assert_eq!(stack::block(&[]), "The inventory records no component.");
}

#[test]
fn the_floor_line_names_the_component_and_version_and_the_unknown_case_says_no_floor_stands() {
  let floor = AgeFloor {
    component: "Node.js".into(),
    version: "22.20.0".into(),
    at: 1_790_000_000_000,
  };
  let named = stack::floor_text(Some(&floor));
  assert!(named.contains("The oldest recorded component is Node.js 22.20.0"), "{named}");
  assert!(named.contains("out of scope"), "{named}");

  let none = stack::floor_text(None);
  assert!(none.contains("No age floor stands"), "{none}");
  assert!(none.contains("neither old nor new"), "{none}");
}

#[test]
fn a_unit_s_deploy_record_is_a_component_with_its_version_or_its_commit() {
  let root = support::scratch();
  std::fs::create_dir_all(root.join("bmo-alpha")).unwrap();
  std::fs::write(
    root.join("bmo-alpha/deployed.json"),
    r#"{"commit":"abcdef123456","version":"1.2.3","at":"2026-09-01T00:00:00Z"}"#,
  )
  .unwrap();
  std::fs::create_dir_all(root.join("bmo-beta")).unwrap();
  std::fs::write(
    root.join("bmo-beta/deployed.json"),
    r#"{"commit":"fedcba654321","version":"","at":"2026-09-02T00:00:00Z"}"#,
  )
  .unwrap();

  let settings = Settings {
    stack: research_server::config::Stack {
      roots: vec![root.to_string_lossy().into_owned()],
    },
    ..Settings::default()
  };
  let rows = stack::deployed(&settings);
  let named: Vec<(&str, Option<&str>)> =
    rows.iter().map(|row| (row.component.as_str(), row.version.as_deref())).collect();
  assert_eq!(named, [("alpha", Some("1.2.3")), ("beta", Some("fedcba654321"))]);
  assert!(rows.iter().all(|row| row.at.is_some() && row.repository.is_some()));
}

#[test]
fn the_deploy_records_stand_beside_a_default_data_root() {
  let root = support::scratch();
  std::fs::create_dir_all(root.join("bmo-gamma")).unwrap();
  std::fs::write(
    root.join("bmo-gamma/deployed.json"),
    r#"{"commit":"0123456789ab","at":"2026-10-01T00:00:00Z"}"#,
  )
  .unwrap();

  let settings = Settings {
    data_root: root.join("bmo/data"),
    ..Settings::default()
  };
  let rows = stack::deployed(&settings);
  assert_eq!(rows.len(), 1, "{rows:?}");
  assert_eq!(rows[0].component, "gamma");
  assert_eq!(rows[0].version.as_deref(), Some("0123456789ab"));
}
