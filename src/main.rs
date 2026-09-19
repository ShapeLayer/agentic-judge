use agentic_judge::{config, engine::Engine, installation};
use anyhow::{Context, Result, ensure};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let auto_open = args.is_empty();
    let mode = if args.first().is_some_and(|a| !a.starts_with('-')) {
        args.remove(0)
    } else {
        "serve".into()
    };
    let mut config_path = None;
    let mut workspace = None;
    let mut scratch = false;
    let mut open = auto_open;
    let mut listen = None;
    let mut positional = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--config" => {
                config_path = Some(PathBuf::from(iter.next().context("--config needs a path")?))
            }
            "--workspace" => {
                workspace = Some(PathBuf::from(
                    iter.next().context("--workspace needs a directory")?,
                ))
            }
            "--scratch" => scratch = true,
            "--open" => open = true,
            "--listen" => listen = Some(iter.next().context("--listen needs an address")?),
            "--help" | "-h" => {
                help();
                return Ok(());
            }
            "--version" | "-V" => {
                println!("agentic-judge {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ if arg.starts_with('-') => anyhow::bail!("unknown option {arg}"),
            _ => positional.push(arg),
        }
    }
    ensure!(
        !(scratch && workspace.is_some()),
        "choose --scratch or --workspace"
    );
    match mode.as_str() {
        "help" => {
            help();
            return Ok(());
        }
        "schema" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&agentic_judge::mcp::tools())?
            );
            return Ok(());
        }
        "init-config" => {
            ensure!(positional.len() <= 1, "init-config accepts one path");
            let path = match config_path.or_else(|| positional.first().map(PathBuf::from)) {
                Some(path) => path,
                None => config::UserPaths::discover()?.config,
            };
            config::initialize(&path)?;
            eprintln!("Created {}", path.display());
            return Ok(());
        }
        "mcp" | "serve" | "call" | "doctor" | "runtime-install" => {}
        _ => anyhow::bail!("unknown mode {mode}"),
    }
    // Preserve the original CLI's explicit positional config form.
    if config_path.is_none() && positional.first().is_some_and(|p| p.ends_with(".toml")) {
        config_path = Some(PathBuf::from(positional.remove(0)));
    }
    let cfg = config::load(config_path.as_deref(), workspace.as_deref(), scratch)?;
    match mode.as_str() {
        "doctor" => {
            ensure!(
                positional.is_empty(),
                "doctor takes no positional arguments"
            );
            let result = installation::doctor(&cfg)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            ensure!(
                result["ready"] == true,
                "runtime is not ready; see diagnostics above"
            );
            Ok(())
        }
        "runtime-install" => installation::install(&cfg, &positional),
        "mcp" => {
            ensure!(positional.is_empty(), "unexpected MCP arguments");
            agentic_judge::mcp::serve(Engine::new(cfg)?)
        }
        "serve" => {
            ensure!(positional.len() <= 1, "serve accepts one listen address");
            let addr = listen
                .or_else(|| positional.first().cloned())
                .unwrap_or_else(|| "127.0.0.1:27121".into());
            agentic_judge::api::serve_with_options(Engine::new(cfg)?, &addr, open)
        }
        "call" => {
            ensure!(
                !positional.is_empty() && positional.len() <= 2,
                "call needs a tool and optional JSON"
            );
            let value =
                serde_json::from_str(positional.get(1).map(String::as_str).unwrap_or("{}"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&Engine::new(cfg)?.call(&positional[0], value)?)?
            );
            Ok(())
        }
        _ => unreachable!(),
    }
}
fn help() {
    println!(
        "agentic-judge [serve|mcp|call|doctor|runtime-install|init-config|schema]\n\
      --config <file>       Explicit configuration (otherwise per-user config)\n\
      --workspace <dir>     Attach a solving folder, independently of config\n\
      --scratch             No filesystem workspace\n\
      --listen <ip:port>    Loopback HTTP address (default 127.0.0.1:27121)\n\
      --open                Open the bundled app in your browser\n\
    call <tool> [JSON]\n\
    runtime-install [build | pull | load <archive>]\n\
    init-config [file]      Create config without overwriting\n\
    No arguments: start the app and open a browser.\n\
    AGENTIC_JUDGE_HOME overrides per-user config/data/cache locations.\n\
    Legacy <mode> <config.toml> ... remains supported."
    );
}
