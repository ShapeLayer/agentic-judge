use std::{env, fs, path::PathBuf};
fn main() {
    for path in [
        "ui/app",
        "ui/build.mjs",
        "ui/styles/tokens.css",
        "ui/tsconfig.json",
        "ui/package.json",
        "ui/package-lock.json",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let ui = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("ui");
    let status = std::process::Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" })
        .args(["run", "build", "--offline"])
        .current_dir(ui)
        .status()
        .expect("UI build requires Node.js and npm; install them and run `npm ci --prefix ui`");
    assert!(
        status.success(),
        "UI TypeScript build failed; run `npm ci --prefix ui` and `npm run check --prefix ui`"
    );
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
