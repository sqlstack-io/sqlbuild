#[cfg(unix)]
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Child;
use std::process::Command;

const FAKE_SQB: &str = r#"
use std::io::{Read, Write};

fn main() {
    let mut out = std::io::stdout();
    for argument in std::env::args_os().skip(1) {
        writeln!(out, "arg:{:?}", argument).unwrap();
    }
    if std::env::var_os("FAKE_STDIN").is_some() {
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input).unwrap();
        write!(out, "stdin:{input}").unwrap();
    }
    out.flush().unwrap();
    eprint!("fake stderr");
    if std::env::var_os("FAKE_WAIT").is_some() {
        #[cfg(windows)]
        console::exit_on_break();
        writeln!(out, "ready:{}", std::process::id()).unwrap();
        out.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
    let code: i32 = std::env::var("FAKE_EXIT").ok().and_then(|code| code.parse().ok()).unwrap_or(0);
    std::process::exit(code);
}

#[cfg(windows)]
mod console {
    type Handler = unsafe extern "system" fn(u32) -> i32;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<Handler>, add: i32) -> i32;
    }

    unsafe extern "system" fn on_break(_event: u32) -> i32 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        std::process::exit(42)
    }

    pub fn exit_on_break() {
        unsafe {
            SetConsoleCtrlHandler(Some(on_break), 1);
        }
    }
}
"#;

#[cfg(windows)]
const BREAKER: &str = r#"
use std::io::BufRead;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

#[link(name = "kernel32")]
extern "system" {
    fn GenerateConsoleCtrlEvent(event: u32, group: u32) -> i32;
}

fn main() {
    let forwarder = std::env::args_os().nth(1).unwrap();
    let result = std::env::args_os().nth(2).unwrap();
    let mut child = Command::new(forwarder)
        .creation_flags(0x0000_0200)
        .env("FAKE_WAIT", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut ready = String::new();
    std::io::BufReader::new(child.stdout.take().unwrap()).read_line(&mut ready).unwrap();
    let sent = unsafe { GenerateConsoleCtrlEvent(1, child.id()) };
    let status = child.wait().unwrap();
    std::fs::write(result, format!("{sent} {}", status.code().unwrap())).unwrap();
}
"#;

fn compile(source: &str, directory: &Path, name: &str) -> PathBuf {
    let path = directory.join(format!("{name}.rs"));
    std::fs::write(&path, source).expect("program source");
    let binary = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args(["--edition", "2021", "-o"])
        .arg(&binary)
        .arg(&path)
        .output()
        .expect("run rustc");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

/// The file name of `sqb` on this platform.
pub(crate) fn sqb_file() -> String {
    format!("sqb{}", std::env::consts::EXE_SUFFIX)
}

/// The real `sqlbuild` forwarder, alone in `directory`.
pub(crate) fn forwarder_in(directory: &Path) -> PathBuf {
    let forwarder = directory.join(format!("sqlbuild{}", std::env::consts::EXE_SUFFIX));
    std::fs::hard_link(env!("CARGO_BIN_EXE_sqlbuild"), &forwarder).expect("link the forwarder");
    forwarder
}

/// The real forwarder with a fake `sqb` beside it that reports what it received.
pub(crate) fn forwarder_with_fake_sqb(directory: &Path) -> PathBuf {
    compile(FAKE_SQB, directory, "sqb");
    forwarder_in(directory)
}

/// The pid the waiting fake `sqb` reports on its first stdout line.
#[cfg(unix)]
pub(crate) fn ready_pid(child: &mut Child) -> u32 {
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("piped stdout"))
        .read_line(&mut line)
        .expect("ready line");
    line.trim()
        .strip_prefix("ready:")
        .and_then(|pid| pid.parse().ok())
        .expect("the fake sqb reports its pid")
}

/// Ctrl+Break the forwarder's process group in a fresh console; returns `<sent> <forwarder exit code>`.
#[cfg(windows)]
pub(crate) fn break_forwarder(directory: &Path, forwarder: &Path) -> String {
    use std::os::windows::process::CommandExt;

    let breaker = compile(BREAKER, directory, "breaker");
    let result = directory.join("result.txt");
    let status = Command::new(breaker)
        .arg(forwarder)
        .arg(&result)
        .creation_flags(0x0000_0010)
        .status()
        .expect("run the breaker");
    assert!(status.success(), "breaker failed: {status:?}");
    std::fs::read_to_string(result).expect("breaker result")
}
