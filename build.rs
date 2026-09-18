use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=vendor/testlib/checkers");
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let mut entries: Vec<_> = fs::read_dir(root.join("vendor/testlib/checkers"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
        .collect();
    entries.sort();
    let mut code = String::from("pub static COMMON_CHECKERS: &[(&str, &str)] = &[\n");
    for path in entries {
        code.push_str(&format!(
            "({:?}, include_str!({:?})),\n",
            path.file_stem().unwrap().to_str().unwrap(),
            path.to_str().unwrap()
        ));
    }
    code.push_str("];\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("checkers.rs"),
        code,
    )
    .unwrap();
}
