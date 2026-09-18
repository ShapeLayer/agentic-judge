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
                let file_limit = libc::rlimit {
                    rlim_cur: 16 * 1024 * 1024,
                    rlim_max: 16 * 1024 * 1024,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &file_limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if local {
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
