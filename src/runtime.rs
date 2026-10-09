use crate::model::*;
use anyhow::{Context, Result, ensure};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Runtime {
    pub config: Config,
}
pub struct Program {
    pub dir: PathBuf,
    pub run: Vec<String>,
}
impl Drop for Program {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
impl Runtime {
    /// Reuse a compiled problem checker, retaining private working files per submission.
    pub fn prepare_checker(&self, problem: &Path, source: &str) -> Result<Program> {
        use fs2::FileExt;
        let lang = self
            .config
            .languages
            .get("cpp")
            .context("unsupported checker language")?;
        let key = hash(serde_json::to_vec(&(
            source,
            hash(include_bytes!("../vendor/testlib/testlib.h")),
            lang,
            self.config.container,
            &self.config.container_command,
            &self.config.image,
            env!("CARGO_PKG_VERSION"),
        ))?);
        let root = problem.join(".checker-cache");
        fs::create_dir_all(&root)?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(".lock"))?;
        lock.lock_exclusive()?;
        let cached = root.join(&key);
        if !cached.is_dir() {
            // Failed compilations never publish a cache entry. Rename publishes atomically.
            let compiled = self.prepare("cpp", source)?;
            let staging = root.join(format!(".build-{}", uuid()));
            fs::create_dir(&staging)?;
            let publish = copy_tree(&compiled.dir, &staging)
                .and_then(|()| Ok(fs::rename(&staging, &cached)?));
            if publish.is_err() {
                let _ = fs::remove_dir_all(&staging);
            }
            publish?;
        }
        fs::create_dir_all(&self.config.runtime_root)?;
        let dir = self.config.runtime_root.canonicalize()?.join(uuid());
        fs::create_dir(&dir)?;
        let program = Program {
            dir,
            run: lang.run.clone(),
        };
        // Never write test input/output into the shared cache.
        copy_tree(&cached, &program.dir)?;
        Ok(program)
    }
    pub fn prepare(&self, language: &str, source: &str) -> Result<Program> {
        let lang = self
            .config
            .languages
            .get(language)
            .context("unsupported language")?;
        ensure!(
            !lang.filename.contains('/') && !lang.filename.starts_with('.'),
            "invalid language filename"
        );
        fs::create_dir_all(&self.config.runtime_root)?;
        let dir = self.config.runtime_root.canonicalize()?.join(uuid());
        fs::create_dir(&dir)?;
        let program = Program {
            dir,
            run: lang.run.clone(),
        };
        fs::write(program.dir.join(&lang.filename), source)?;
        fs::write(
            program.dir.join("testlib.h"),
            include_bytes!("../vendor/testlib/testlib.h"),
        )?;
        if !lang.compile.is_empty() {
            let r = self.execute(&program.dir, &lang.compile, "", 60000, 2048)?;
            ensure!(
                r.verdict == "AC",
                "compile error: {} {}",
                r.verdict,
                r.stderr
            );
        }
        Ok(program)
    }
    pub fn run(
        &self,
        p: &Program,
        input: &str,
        args: &[String],
        time_ms: u64,
        memory_mb: u64,
    ) -> Result<RunResult> {
        let mut argv = p.run.clone();
        argv.extend_from_slice(args);
        let run_dir = p.dir.parent().unwrap().join(uuid());
        fs::create_dir(&run_dir)?;
        let isolated = Program {
            dir: run_dir,
            run: vec![],
        };
        copy_tree(&p.dir, &isolated.dir)?;
        self.execute(&isolated.dir, &argv, input, time_ms, memory_mb)
    }
    fn execute(
        &self,
        dir: &Path,
        argv: &[String],
        input: &str,
        time_ms: u64,
        memory_mb: u64,
    ) -> Result<RunResult> {
        ensure!(!argv.is_empty(), "empty command");
        let token = uuid();
        let stdin = dir.join(format!("{token}.stdin"));
        let stdout = dir.join(format!("{token}.stdout"));
        let stderr = dir.join(format!("{token}.stderr"));
        fs::write(&stdin, input)?;
        let container_name = format!("judge-{token}");
        let mut cmd = if self.config.container {
            let mut c = Command::new(&self.config.container_command);
            c.args([
                "run",
                "--name",
                &container_name,
                "--network=none",
                "--pull=never",
                "--read-only",
                "--cap-drop=ALL",
                "--security-opt=no-new-privileges",
                "--pids-limit=64",
                "--cpus=1",
                "--ulimit",
                "fsize=16777216:16777216",
                "--memory",
                &format!("{memory_mb}m"),
                "--memory-swap",
                &format!("{memory_mb}m"),
                "--tmpfs",
                "/tmp:rw,noexec,nosuid,size=256m",
                "--mount",
                &format!("type=bind,src={},dst=/work,relabel=private", dir.display()),
                "--workdir",
                "/work",
                "--userns=keep-id:uid=65534,gid=65534",
                "--user",
                "65534:65534",
                "-i",
                &self.config.image,
            ]);
            c.args(argv);
            c
        } else {
            let mut c = Command::new(&argv[0]);
            c.args(&argv[1..]);
            c.current_dir(dir);
            c
        };
        // A separate process group permits killing descendants on timeout, including local mode.
        cmd.process_group(0);
        cmd.stdin(fs::File::open(&stdin)?)
            .stdout(Stdio::from(fs::File::create(&stdout)?))
            .stderr(Stdio::from(fs::File::create(&stderr)?));
        let local = !self.config.container;
        unsafe {
            cmd.pre_exec(move || {
                // Podman must be free to copy large image layers during user-ID remapping.
                // Container workloads get their file limit from --ulimit above.
                if local {
                    let file_limit = libc::rlimit {
                        rlim_cur: 16 * 1024 * 1024,
                        rlim_max: 16 * 1024 * 1024,
                    };
                    if libc::setrlimit(libc::RLIMIT_FSIZE, &file_limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let mem = libc::rlimit {
                            rlim_cur: memory_mb * 1024 * 1024,
                            rlim_max: memory_mb * 1024 * 1024,
                        };
                        if libc::setrlimit(libc::RLIMIT_AS, &mem) != 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                    }
                }
                Ok(())
            });
        }
        if self.config.container {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        }
        let started = Instant::now();
        let child = cmd
            .spawn()
            .context("could not start runtime (install configured runtime or container engine)")?;
        let pid = child.id() as i32;
        let mut timed_out = false;
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        let status = loop {
            let mut raw_status = 0;
            let waited = unsafe { libc::wait4(pid, &mut raw_status, libc::WNOHANG, &mut usage) };
            if waited == pid {
                break std::process::ExitStatus::from_raw(raw_status);
            }
            if waited < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            if started.elapsed() > Duration::from_millis(time_ms) {
                timed_out = true;
                if self.config.container {
                    let _ = Command::new(&self.config.container_command)
                        .args(["kill", &container_name])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                    libc::wait4(pid, &mut raw_status, 0, &mut usage);
                }
                break std::process::ExitStatus::from_raw(raw_status);
            }
            thread::sleep(Duration::from_millis(5));
        };
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let mut oom = false;
        if self.config.container {
            let inspect = Command::new(&self.config.container_command)
                .args([
                    "inspect",
                    "--format",
                    "{{.State.OOMKilled}}",
                    &container_name,
                ])
                .output()?;
            oom = String::from_utf8_lossy(&inspect.stdout).trim() == "true";
            let _ = Command::new(&self.config.container_command)
                .args(["rm", "-f", &container_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let out = String::from_utf8_lossy(&fs::read(&stdout)?).into_owned();
        let err = String::from_utf8_lossy(&fs::read(&stderr)?).into_owned();
        #[cfg(target_os = "macos")]
        let peak_kb = usage.ru_maxrss as u64 / 1024;
        #[cfg(not(target_os = "macos"))]
        let peak_kb = usage.ru_maxrss as u64;
        let verdict = if timed_out {
            "TLE"
        } else if oom || (local && peak_kb > memory_mb * 1024) {
            "MLE"
        } else if status.success() {
            "AC"
        } else {
            "RE"
        };
        let _ = fs::remove_file(stdin);
        let _ = fs::remove_file(stdout);
        let _ = fs::remove_file(stderr);
        Ok(RunResult {
            verdict: verdict.into(),
            time_ms: elapsed_ms,
            memory_kb: if local { peak_kb } else { 0 },
            stdout: out,
            stderr: err,
            exit_code: status.code(),
        })
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir(&target)?;
            copy_tree(&entry.path(), &target)?;
        } else if entry.file_type()?.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            anyhow::bail!("runtime artifact must be a regular file or directory");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn checker_runtime(root: &Path) -> Runtime {
        let mut config = Config {
            container: false,
            runtime_root: root.join("runtime"),
            ..Config::default()
        };
        config.languages.get_mut("cpp").unwrap().compile = vec![
            "sh".into(),
            "-c".into(),
            "printf x >> \"$1\"; cp main.cpp program; chmod +x program".into(),
            "compiler".into(),
            root.join("compiles").to_str().unwrap().into(),
        ];
        Runtime { config }
    }

    #[test]
    fn checker_cache_reuses_builds_and_invalidates_source_and_settings() {
        let dir = tempfile::tempdir().unwrap();
        let mut rt = checker_runtime(dir.path());
        let problem = dir.path().join("problem");
        let source = "#!/bin/sh\nexit 0\n";
        let first = rt.prepare_checker(&problem, source).unwrap();
        fs::write(first.dir.join("input.txt"), "private input").unwrap();
        drop(first);
        let second = rt.prepare_checker(&problem, source).unwrap();
        assert!(!second.dir.join("input.txt").exists());
        assert_eq!(rt.run(&second, "", &[], 1000, 128).unwrap().verdict, "AC");
        drop(second);
        assert_eq!(fs::read(dir.path().join("compiles")).unwrap(), b"x");
        drop(rt.prepare_checker(&problem, "#!/bin/sh\nexit 1\n").unwrap());
        assert_eq!(fs::read(dir.path().join("compiles")).unwrap(), b"xx");
        rt.config.image = "changed-runtime".into();
        drop(rt.prepare_checker(&problem, source).unwrap());
        assert_eq!(fs::read(dir.path().join("compiles")).unwrap(), b"xxx");
        assert_eq!(fs::read_dir(&rt.config.runtime_root).unwrap().count(), 0);
    }

    #[test]
    fn concurrent_checker_preparation_compiles_once() {
        let dir = tempfile::tempdir().unwrap();
        let rt = checker_runtime(dir.path());
        let problem = dir.path().join("problem");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let rt = rt.clone();
                let problem = problem.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    let p = rt.prepare_checker(&problem, "#!/bin/sh\nexit 0\n").unwrap();
                    assert!(!p.dir.join("input.txt").exists());
                    fs::write(p.dir.join("input.txt"), "private").unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(fs::read(dir.path().join("compiles")).unwrap(), b"x");
    }

    #[test]
    fn container_engine_can_create_large_storage_files() {
        let dir = tempfile::tempdir().unwrap();
        let engine = dir.path().join("engine");
        fs::write(
            &engine,
            r#"#!/bin/sh
case "$1" in
    run)
        case " $* " in
            *" --ulimit fsize=16777216:16777216 "*) ;;
            *) exit 2 ;;
        esac
        cd "$(dirname "$0")" || exit 3
        exec dd if=/dev/zero of=layer bs=1048576 count=17
        ;;
    inspect) echo false ;;
    rm) exit 0 ;;
esac
"#,
        )
        .unwrap();
        fs::set_permissions(&engine, fs::Permissions::from_mode(0o700)).unwrap();
        let rt = Runtime {
            config: Config {
                container: true,
                container_command: engine.to_str().unwrap().into(),
                ..Config::default()
            },
        };
        let result = rt
            .execute(dir.path(), &["true".into()], "", 10000, 128)
            .unwrap();
        assert_eq!(result.verdict, "AC", "{}", result.stderr);
        assert_eq!(
            fs::metadata(dir.path().join("layer")).unwrap().len(),
            17 * 1024 * 1024
        );
    }

    #[test]
    fn local_workload_still_has_file_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let rt = Runtime {
            config: Config {
                container: false,
                ..Config::default()
            },
        };
        let result = rt
            .execute(
                dir.path(),
                &[
                    "sh".into(),
                    "-c".into(),
                    "exec dd if=/dev/zero of=output bs=1048576 count=17".into(),
                ],
                "",
                10000,
                128,
            )
            .unwrap();
        assert_eq!(result.verdict, "RE");
        assert_eq!(
            fs::metadata(dir.path().join("output")).unwrap().len(),
            16 * 1024 * 1024
        );
    }
}
