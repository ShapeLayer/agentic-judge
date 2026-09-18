use agentic_judge::{engine::Engine, model::Config};
use anyhow::{Context, Result};
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "help".into());
    if mode == "help" {
        eprintln!(
            "agentic-judge <mcp|serve|call|init-config> [config.toml] [tool + JSON / listen address]\nDefault config: judge.toml. Container execution is enabled by default."
        );
        return Ok(());
    }
    if mode == "schema" {
        println!(
            "{}",
            serde_json::to_string_pretty(&agentic_judge::mcp::tools())?
        );
        return Ok(());
    }
    let path = args.next().unwrap_or_else(|| "judge.toml".into());
    if mode == "init-config" {
        anyhow::ensure!(
            !std::path::Path::new(&path).exists(),
            "config already exists"
        );
        std::fs::write(path, toml::to_string_pretty(&Config::default())?)?;
        return Ok(());
    }
    let mut config: Config = toml::from_str(
        &std::fs::read_to_string(&path).with_context(|| format!("read config {path}"))?,
    )?;
    let config_dir = std::path::Path::new(&path)
        .canonicalize()?
        .parent()
        .unwrap()
        .to_path_buf();
    for root in [
        &mut config.storage_root,
        &mut config.runtime_root,
        &mut config.workspace_root,
    ] {
        if root.is_relative() {
            *root = config_dir.join(&*root);
        }
    }
    let engine = Engine::new(config)?;
    match mode.as_str() {
        "mcp" => agentic_judge::mcp::serve(engine),
        "serve" => agentic_judge::api::serve(
            engine,
            &args.next().unwrap_or_else(|| "127.0.0.1:27121".into()),
        ),
        "call" => {
            let name = args.next().context("tool name required")?;
            let v = serde_json::from_str(&args.next().unwrap_or_else(|| "{}".into()))?;
            println!("{}", serde_json::to_string_pretty(&engine.call(&name, v)?)?);
            Ok(())
        }
        _ => anyhow::bail!("unknown mode"),
    }
}
