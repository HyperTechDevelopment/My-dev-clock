use std::{
    io::Read,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// Environment variable handed to every command widget, so a widget can match
/// the palette the app is currently showing.
pub const THEME_ENV: &str = "MY_DEV_CLOCK_THEME";

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A command as configured: a single element is a shell line, more than one is
/// a program plus its arguments.
fn build(command: &[String], theme: &str) -> Option<Command> {
    let mut cmd = match command {
        [] => return None,
        [line] => {
            let mut cmd = Command::new(shell_program());
            cmd.arg(shell_flag()).arg(line);
            cmd
        }
        [program, args @ ..] => {
            let mut cmd = Command::new(program);
            cmd.args(args);
            cmd
        }
    };

    cmd.env(THEME_ENV, theme)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    Some(cmd)
}

#[cfg(windows)]
fn shell_program() -> &'static str {
    "cmd"
}

#[cfg(windows)]
fn shell_flag() -> &'static str {
    "/C"
}

#[cfg(not(windows))]
fn shell_program() -> &'static str {
    "sh"
}

#[cfg(not(windows))]
fn shell_flag() -> &'static str {
    "-c"
}

/// Runs a command, captures what it prints, and returns it as widget text.
///
/// A command that outlives `timeout` is killed along with everything it
/// started, and a set `cancel` flag kills it the same way, which is how closing
/// the app or switching a widget off cannot leave stray processes behind.
pub fn run_capture(
    command: &[String],
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    theme: &str,
) -> String {
    let Some(mut cmd) = build(command, theme) else {
        return "[erro] comando não configurado".to_string();
    };

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            return format!("[erro] não consegui executar {}: {error}", command[0]);
        }
    };

    // Best effort: if the job cannot be created the kill falls back to the
    // direct child, which is still better than nothing.
    let mut job = Job::create();
    let assigned = job.as_ref().is_none_or(|job| job.assign(&child).is_ok());
    if !assigned {
        // Sem job, o encerramento cai para o processo filho direto.
        job = None;
    }

    let stdout = child
        .stdout
        .take()
        .map(|mut stdout| thread::spawn(move || read_bounded(&mut stdout)));
    let stderr = child
        .stderr
        .take()
        .map(|mut stderr| thread::spawn(move || read_bounded(&mut stderr)));

    let started_at = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            kill(&mut child, job.as_ref());
            return "[cancelado]".to_string();
        }

        match child.try_wait() {
            Ok(Some(_)) => {
                let status = child.wait();
                let stdout = join(stdout);
                let stderr = join(stderr);
                let success = status.map(|status| status.success()).unwrap_or(false);
                return format_output(success, &stdout, &stderr);
            }
            Ok(None) if started_at.elapsed() >= timeout => {
                kill(&mut child, job.as_ref());
                return format!("[timeout] o comando passou de {}s", timeout.as_secs());
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                kill(&mut child, job.as_ref());
                return format!("[erro] falha ao esperar o comando: {error}");
            }
        }
    }
}

fn kill(child: &mut Child, job: Option<&Job>) {
    match job {
        // Killing the job takes the whole tree with it.
        Some(job) => job.kill(),
        None => {
            let _ = child.kill();
        }
    }
    let _ = child.wait();
}

fn read_bounded(reader: &mut impl Read) -> Vec<u8> {
    let mut output = Vec::new();
    let mut buffer = [0u8; 8 * 1024];

    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let room = MAX_OUTPUT_BYTES.saturating_sub(output.len());
        if room == 0 {
            // Keep draining so the child never blocks on a full pipe.
            continue;
        }
        output.extend_from_slice(&buffer[..read.min(room)]);
    }

    output
}

fn join(handle: Option<thread::JoinHandle<Vec<u8>>>) -> String {
    let bytes = handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    String::from_utf8_lossy(&bytes)
        .replace('\r', "")
        .trim_end()
        .to_string()
}

fn format_output(success: bool, stdout: &str, stderr: &str) -> String {
    if success {
        if stdout.is_empty() {
            "[ok]".to_string()
        } else {
            stdout.to_string()
        }
    } else if stderr.is_empty() {
        format!("[erro] {stdout}")
    } else {
        format!("[erro] {stderr}")
    }
}

#[cfg(windows)]
mod job_impl {
    use std::{io, mem, process::Child, ptr};

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };

    /// A Windows job object holding one widget command and everything it
    /// starts, with `KILL_ON_JOB_CLOSE` so dropping the handle takes the tree
    /// down even if the app exits without cleaning up.
    pub struct Job(HANDLE);

    impl Job {
        pub fn create() -> Option<Self> {
            unsafe {
                let handle = CreateJobObjectW(ptr::null(), ptr::null());
                if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                    return None;
                }

                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                let ok = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    ptr::from_ref(&info).cast(),
                    mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    CloseHandle(handle);
                    return None;
                }

                Some(Self(handle))
            }
        }

        pub fn assign(&self, child: &Child) -> io::Result<()> {
            unsafe {
                let pid = child.id();
                let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
                if process.is_null() {
                    return Err(io::Error::last_os_error());
                }

                let ok = AssignProcessToJobObject(self.0, process);
                CloseHandle(process);
                if ok == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            }
        }

        pub fn kill(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(windows)]
pub use job_impl::Job;

#[cfg(not(windows))]
pub struct Job;

#[cfg(not(windows))]
impl Job {
    pub fn create() -> Option<Self> {
        None
    }

    pub fn assign(&self, _child: &Child) -> std::io::Result<()> {
        Ok(())
    }

    pub fn kill(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn never_cancelled() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn an_empty_command_reports_the_problem_instead_of_panicking() {
        let output = run_capture(&[], DEFAULT_TIMEOUT, never_cancelled(), "default");
        assert!(output.starts_with("[erro]"), "{output}");
    }

    #[test]
    fn shell_lines_run_and_their_output_comes_back() {
        let command = vec!["echo ola".to_string()];
        let output = run_capture(&command, DEFAULT_TIMEOUT, never_cancelled(), "default");
        assert_eq!(output, "ola");
    }

    #[test]
    fn a_direct_program_with_arguments_runs() {
        let command: Vec<String> = if cfg!(windows) {
            vec!["cmd".into(), "/C".into(), "echo direto".into()]
        } else {
            vec!["echo".into(), "direto".into()]
        };
        let output = run_capture(&command, DEFAULT_TIMEOUT, never_cancelled(), "default");
        assert_eq!(output, "direto");
    }

    #[test]
    fn a_failing_command_reports_stderr() {
        let command: Vec<String> = if cfg!(windows) {
            vec![
                "cmd".into(),
                "/C".into(),
                "echo falhou 1>&2 & exit /B 3".into(),
            ]
        } else {
            vec!["sh".into(), "-c".into(), "echo falhou 1>&2; exit 3".into()]
        };
        let output = run_capture(&command, DEFAULT_TIMEOUT, never_cancelled(), "default");
        assert!(output.starts_with("[erro]"), "{output}");
        assert!(output.contains("falhou"), "{output}");
    }

    #[test]
    fn the_theme_is_exported_to_the_command() {
        let command: Vec<String> = if cfg!(windows) {
            vec!["cmd".into(), "/C".into(), format!("echo %{THEME_ENV}%")]
        } else {
            vec!["sh".into(), "-c".into(), format!("echo ${THEME_ENV}")]
        };
        let output = run_capture(&command, DEFAULT_TIMEOUT, never_cancelled(), "matrix");
        assert_eq!(output, "matrix");
    }

    #[test]
    fn a_command_that_overstays_its_timeout_is_killed() {
        let command: Vec<String> = if cfg!(windows) {
            vec![
                "cmd".into(),
                "/C".into(),
                "ping -n 30 127.0.0.1 >NUL".into(),
            ]
        } else {
            vec!["sh".into(), "-c".into(), "sleep 30".into()]
        };

        let started = Instant::now();
        let output = run_capture(
            &command,
            Duration::from_millis(400),
            never_cancelled(),
            "default",
        );

        assert!(output.starts_with("[timeout]"), "{output}");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "demorou demais"
        );
    }

    #[test]
    fn cancelling_kills_the_command_instead_of_waiting_for_it() {
        let command: Vec<String> = if cfg!(windows) {
            vec![
                "cmd".into(),
                "/C".into(),
                "ping -n 30 127.0.0.1 >NUL".into(),
            ]
        } else {
            vec!["sh".into(), "-c".into(), "sleep 30".into()]
        };

        let cancel = Arc::new(AtomicBool::new(true));
        let started = Instant::now();
        let output = run_capture(&command, DEFAULT_TIMEOUT, cancel, "default");

        assert_eq!(output, "[cancelado]");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "demorou demais"
        );
    }
}
