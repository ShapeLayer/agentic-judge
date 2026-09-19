//! Installation paths are independent of the working directory and source checkout.
use crate::model::Config;
use anyhow::{Context, Result, ensure};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub struct UserPaths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
}
impl UserPaths {
    pub fn discover() -> Result<Self> {
        if let Some(root) = env::var_os("AGENTIC_JUDGE_HOME") {
            let root = PathBuf::from(root);
            ensure!(root.is_absolute(), "AGENTIC_JUDGE_HOME must be absolute");
            return Ok(Self {
                config: root.join("config/judge.toml"),
                data: root.join("data"),
                cache: root.join("cache/run"),
            });
        }
        let home =
            PathBuf::from(env::var_os("HOME").context("HOME is unset; set AGENTIC_JUDGE_HOME")?);
        ensure!(home.is_absolute(), "HOME must be absolute");
        if cfg!(target_os = "macos") {
            let base = home.join("Library/Application Support/Agentic Judge");
            Ok(Self {
                config: base.join("judge.toml"),
                data: base.join("data"),
                cache: home.join("Library/Caches/Agentic Judge/run"),
            })
        } else {
            let xdg = |key: &str, fallback: &str| {
                env::var_os(key)
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .unwrap_or_else(|| home.join(fallback))
            };
            Ok(Self {
                config: xdg("XDG_CONFIG_HOME", ".config").join("agentic-judge/judge.toml"),
                data: xdg("XDG_DATA_HOME", ".local/share").join("agentic-judge"),
                cache: xdg("XDG_CACHE_HOME", ".cache").join("agentic-judge/run"),
            })
        }
    }
}

pub fn installed_defaults() -> Result<Config> {
    let paths = UserPaths::discover()?;
    Ok(Config {
        storage_root: paths.data,
        runtime_root: paths.cache,
        ..Config::default()
    })
}

pub fn initialize(path: &Path) -> Result<()> {
    use std::io::Write;
    let text = toml::to_string_pretty(&installed_defaults()?)?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    // Publish complete contents atomically without replacing another initializer's file.
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = parent.join(format!(".judge-config-{}", crate::model::uuid()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::hard_link(&temporary, path).with_context(|| {
            format!(
                "create config {} (existing files are never overwritten)",
                path.display()
            )
        })?;
        Ok(())
    })();
    let _ = fs::remove_file(temporary);
    result?;
    Ok(())
}

pub fn load(explicit: Option<&Path>, workspace: Option<&Path>, scratch: bool) -> Result<Config> {
    let default_path;
    let path = if let Some(path) = explicit {
        path
    } else {
        default_path = UserPaths::discover()?.config;
        if !default_path.exists() {
            // Another starting MCP/HTTP process may have initialized it concurrently.
            if let Err(error) = initialize(&default_path)
                && !default_path.exists()
            {
                return Err(error);
            }
        }
        &default_path
    };
    let mut config: Config = toml::from_str(
        &fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?,
    )?;
    let parent = path.canonicalize()?.parent().unwrap().to_owned();
    for root in [&mut config.storage_root, &mut config.runtime_root] {
        if root.is_relative() {
            *root = parent.join(&*root);
        }
    }
    if let Some(root) = &mut config.workspace_root
        && root.is_relative()
    {
        *root = parent.join(&*root);
    }
    if scratch {
        config.workspace_root = None;
    }
    if let Some(root) = workspace {
        config.workspace_root = Some(root.canonicalize().context("open workspace")?);
    }
    if let Some(root) = &mut config.workspace_root {
        *root = root.canonicalize().context("open configured workspace")?;
        ensure!(root.is_dir(), "workspace must be a directory");
    }
    Ok(config)
}
