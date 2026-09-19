use agentic_judge::{engine::Engine, model::Config};
use serde_json::json;
use std::{
    fs,
    io::{Cursor, Write},
};
fn setup() -> (tempfile::TempDir, Engine) {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::new(Config {
        storage_root: dir.path().join("data"),
        runtime_root: dir.path().join("run"),
        container: false,
        ..Config::default()
    })
    .unwrap();
    engine
        .call(
            "problem_create",
            json!({"problem_id":"source","name":"한글 문제"}),
        )
        .unwrap();
    engine
        .store
        .put_test("source", "general", 1, "1 2\n")
        .unwrap();
    fs::write(
        engine
            .store
            .test_path("source", "general", 1, "out")
            .unwrap(),
        "3\n",
    )
    .unwrap();
    (dir, engine)
}
#[test]
fn round_trip_all_scopes_and_collisions() {
    let (_dir, e) = setup();
    e.call("statement_set", json!({"problem_id":"source","section":"legend","markdown":"![그림](/api/problems/source/assets/test.png)"})).unwrap();
    let p = e.store.problem("source").unwrap();
    fs::create_dir_all(p.join("statements/assets")).unwrap();
    fs::write(p.join("statements/assets/test.png"), [0, 1, 2, 255]).unwrap();
    let a = e
        .store
        .save_answer("source", None, "python", "print(3)", "AC")
        .unwrap();
    let mut m = e.store.manifest("source").unwrap();
    m.main_solution = Some(a.id.clone());
    e.store.save_manifest(&m).unwrap();
    e.call("suite_add", json!({"problem_id":"source","suite":"empty"}))
        .unwrap();
    let archive = e.export_archive("problem", "source", "", 0).unwrap();
    assert_eq!(&archive[..2], b"PK");
    e.import_archive(&archive, "problem", "copy", "", 0)
        .unwrap();
    let copy = e.call("problem_get", json!({"problem_id":"copy"})).unwrap();
    assert!(
        copy["statements"]["legend"]["markdown"]
            .as_str()
            .unwrap()
            .contains("/copy/assets/")
    );
    assert_eq!(copy["ready"], false);
    assert_eq!(e.store.answer("copy", &a.id).unwrap().source, "print(3)");
    assert_eq!(
        fs::read(
            e.store
                .problem("copy")
                .unwrap()
                .join("statements/assets/test.png")
        )
        .unwrap(),
        [0, 1, 2, 255]
    );
    assert!(
        e.import_archive(&archive, "problem", "copy", "", 0)
            .is_err()
    );
    let suite = e.export_archive("suite", "source", "general", 0).unwrap();
    e.import_archive(&suite, "suite", "copy", "extra", 0)
        .unwrap();
    assert!(
        e.import_archive(&suite, "suite", "copy", "extra", 0)
            .is_err()
    );
    let test = e.export_archive("test", "source", "general", 1).unwrap();
    e.import_archive(&test, "test", "copy", "extra", 7).unwrap();
    assert_eq!(
        fs::read_to_string(e.store.test_path("copy", "extra", 7, "in").unwrap()).unwrap(),
        "1 2\n"
    );
    assert_eq!(
        fs::read_to_string(e.store.test_path("copy", "extra", 7, "out").unwrap()).unwrap(),
        "3\n"
    );
    assert!(e.import_archive(&test, "test", "copy", "extra", 7).is_err());
    assert!(
        e.import_archive(&suite, "test", "copy", "extra", 8)
            .is_err()
    );
    assert!(e.export_archive("test", "source", "general", 99).is_err());
    let empty = e.export_archive("suite", "source", "empty", 0).unwrap();
    e.import_archive(&empty, "suite", "copy", "empty-copy", 0)
        .unwrap();
}
fn zip(value: &serde_json::Value) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("package.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
    zip.finish().unwrap().into_inner()
}
#[test]
fn malformed_archives_leave_storage_unchanged() {
    let (_dir, e) = setup();
    let bytes = e.export_archive("problem", "source", "", 0).unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let value: serde_json::Value =
        serde_json::from_reader(z.by_name("package.json").unwrap()).unwrap();
    let mut bad = value.clone();
    bad["files"]["statements/../../escape"] = json!([1]);
    assert!(
        e.import_archive(&zip(&bad), "problem", "bad", "", 0)
            .is_err()
    );
    assert!(!e.store.problem("bad").unwrap().exists());
    let mut bad = value.clone();
    bad["tests"][0]["number"] = json!(0);
    assert!(
        e.import_archive(&zip(&bad), "problem", "bad", "", 0)
            .is_err()
    );
    let mut bad = value;
    bad["version"] = json!(999);
    assert!(
        e.import_archive(&zip(&bad), "problem", "bad", "", 0)
            .is_err()
    );
    assert!(
        e.import_archive(b"not a zip", "problem", "bad", "", 0)
            .is_err()
    );
    assert!(!e.store.problem("bad").unwrap().exists());
}
