//! Schema evolution tests copied from `defs/src/schema_test.rs` (uses [`catalog_aws::compat_models::ModuleResp`]).
//!
//! Source: `defs/src/schema_test.rs` — JSON fixtures under `schema_test/module_resp/`.

use catalog_aws::compat_models::ModuleResp;
use std::fs;

const SCHEMA_TEST_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/schema_test/module_resp");

#[test]
fn schema_evolution_module_resp() {
    let dir = fs::read_dir(SCHEMA_TEST_DIR)
        .unwrap_or_else(|e| panic!("failed to read schema test dir {}: {}", SCHEMA_TEST_DIR, e));
    let mut entries: Vec<_> = dir
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    entries.sort_by_key(|p| p.file_name().unwrap().to_owned());

    if entries.is_empty() {
        panic!("no files in {}", SCHEMA_TEST_DIR);
    }

    let mut last_contents = None;
    for path in &entries {
        let name = path.file_name().unwrap().to_string_lossy();
        let contents =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {}: {}", name, e));
        let _resp: ModuleResp = serde_json::from_str(&contents)
            .unwrap_or_else(|e| panic!("failed to deserialize {}: {}", name, e));
        last_contents = Some(contents);
    }

    let last_json = last_contents.unwrap();
    let last_parsed: ModuleResp =
        serde_json::from_str(&last_json).expect("last schema file must deserialize to ModuleResp");
    assert_eq!(
        last_parsed,
        ModuleResp::default(),
        "last file {} must deserialize to ModuleResp::default() (schema evolution anchor)",
        entries
            .last()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
    );
}
