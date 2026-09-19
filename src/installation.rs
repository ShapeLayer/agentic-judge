//! Repository-free runtime provisioning and checks. No implicit image downloads.
use crate::{
    model::{Config, uuid},
    runtime::Program,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

pub fn install(config: &Config, args: &[String]) -> Result<()> {
    ensure!(config.container, "runtime-install requires container=true");
    let action = args.first().map(String::as_str).unwrap_or("build");
    let mut cmd = Command::new(&config.container_command);
    let temporary;
    match action {
        "build" => {
            ensure!(args.len() <= 1, "usage: runtime-install build");
            fs::create_dir_all(&config.runtime_root)?;
            let dir = config
                .runtime_root
                .canonicalize()?
                .join(format!("install-{}", uuid()));
            fs::create_dir(&dir)?;
            temporary = Some(Program { dir, run: vec![] });
            let dir = &temporary.as_ref().unwrap().dir;
            fs::write(
                dir.join("Containerfile"),
                include_str!("../runtime/Containerfile"),
            )?;
            cmd.args(["build", "-t", &config.image]).arg(dir);
        }
        "pull" => {
            ensure!(args.len() == 1, "usage: runtime-install pull");
            temporary = None;
            cmd.args(["pull", &config.image]);
        }
        "load" => {
            ensure!(args.len() == 2, "usage: runtime-install load <archive>");
            let archive = Path::new(&args[1])
                .canonicalize()
                .context("read runtime archive")?;
            temporary = None;
            cmd.args(["load", "--input"]).arg(archive);
        }
        _ => anyhow::bail!("runtime-install supports build, pull, or load <archive>"),
    }
    ensure!(
        cmd.status()
            .context("start container command; install Podman and start its machine first")?
            .success(),
        "runtime installation failed"
    );
    drop(temporary);
    ensure!(
        Command::new(&config.container_command)
            .args(["image", "exists", &config.image])
            .status()?
            .success(),
        "installed archive does not provide configured image {}; update image in config or load the matching archive",
        config.image
    );
    eprintln!(
        "Runtime installed: {}. Run agentic-judge doctor to verify execution and shared paths.",
        config.image
    );
    Ok(())
}

fn executable(name: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let valid = |path: &Path| {
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if name.contains('/') {
        return valid(Path::new(name));
    }
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| valid(&dir.join(name))))
}

pub fn doctor(config: &Config) -> Result<Value> {
    fs::create_dir_all(&config.runtime_root).context("create runtime directory")?;
    let mut checks = Vec::new();
    if config.container {
        let info = Command::new(&config.container_command).arg("info").output();
        let available = info.as_ref().is_ok_and(|o| o.status.success());
        checks.push(json!({"check":"podman","ok":available,"action":"Install Podman; on macOS run podman machine init once, then podman machine start","detail":info.map(|o|String::from_utf8_lossy(&o.stderr).into_owned()).unwrap_or_else(|e|e.to_string())}));
        if available {
            let image = Command::new(&config.container_command)
                .args(["image", "exists", &config.image])
                .status()?
                .success();
            checks.push(json!({"check":"image","ok":image,"image":config.image,"action":"Run agentic-judge runtime-install (or load an offline archive)"}));
            if image {
                let dir = config
                    .runtime_root
                    .canonicalize()?
                    .join(format!("probe-{}", uuid()));
                fs::create_dir(&dir)?;
                let guard = Program { dir, run: vec![] };
                fs::write(guard.dir.join("probe"), "agentic-judge")?;
                let output = Command::new(&config.container_command)
                    .args([
                        "run",
                        "--rm",
                        "--pull=never",
                        "--network=none",
                        "--read-only",
                        "--cap-drop=ALL",
                        "--security-opt=no-new-privileges",
                        "--userns=keep-id:uid=65534,gid=65534",
                        "--user",
                        "65534:65534",
                        "--mount",
                        &format!(
                            "type=bind,src={},dst=/work,relabel=private",
                            guard.dir.display()
                        ),
                        &config.image,
                        "sh",
                        "-c",
                        "test \"$(cat /work/probe)\" = agentic-judge && echo ready > /work/result",
                    ])
                    .output()?;
                let ok = output.status.success()
                    && fs::read_to_string(guard.dir.join("result"))
                        .is_ok_and(|s| s.trim() == "ready");
                checks.push(json!({"check":"runtime_mount","ok":ok,"detail":String::from_utf8_lossy(&output.stderr),"action":"Ensure runtime_root is writable and shared with the Podman VM"}));
            }
        }
    } else {
        for (name, language) in &config.languages {
            let commands: Vec<_> = [language.compile.first(), language.run.first()]
                .into_iter()
                .flatten()
                .filter(|s| !s.starts_with("./"))
                .collect();
            checks.push(json!({"check":format!("language:{name}"),"ok":commands.iter().all(|s| executable(s)),"commands":commands}));
        }
    }
    Ok(
        json!({"ready":checks.iter().all(|v|v["ok"]==true),"container":config.container,"workspace":config.workspace_root,"storage_root":config.storage_root,"runtime_root":config.runtime_root,"checks":checks}),
    )
}
