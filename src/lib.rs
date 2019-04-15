pub fn now_naive_date_time() -> chrono::NaiveDateTime {
    /* return the "now" value of naivedatetime */
    use chrono::*;
    let ndt = Local::now().naive_local();
    ndt
}

pub fn ndt_add_seconds(ndt: chrono::NaiveDateTime, sec: i32) -> chrono::NaiveDateTime {
    use chrono::prelude::*;
    use chrono::*;
    ndt.checked_add_signed(Duration::seconds(sec as i64))
        .unwrap()
}
pub fn ndt_add_seconds_safe(ndt: chrono::NaiveDateTime, sec: i32) -> Option<chrono::NaiveDateTime> {
    use chrono::prelude::*;
    use chrono::*;
    ndt.checked_add_signed(Duration::seconds(sec as i64))
}

pub fn thread_sleep_ms(ms: u64) {
    use std::thread;
    use std::time::Duration;
    thread::sleep(Duration::from_millis(ms));
}

use std::fs::File;

pub fn open_log_file(verb: &str) -> std::io::Result<File> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let logfname = format!("{}", verb);
    let file = OpenOptions::new().append(true).create(true).open(logfname);
    file
}

pub fn spawn_simple_command(command_dir: &str, cmd_file: &str) {
    use std::process::Command;
    use std::process::Stdio;

    let cmd = format!("{}/{}", command_dir, cmd_file);
    let log_file = open_log_file(cmd_file).unwrap();
    let stderr_cmd = format!("{}-stderr", cmd_file);
    let log_file_stderr = log_file.try_clone().unwrap(); // open_log_file(&stderr_cmd).unwrap();
                                                         // let errors = outputs.try_clone()?;
    let mut child = Command::new(cmd)
        // .arg("--debug")
        // .arg(verb)
        .stdin(Stdio::null())
        .stdout(log_file)
        .stderr(log_file_stderr)
        .env("RUST_BACKTRACE", "1")
        .spawn()
        .expect("failed to execute child");
}

pub fn spawn_command_x(command_dir: &str, cmd_file: &str, arg_ref: &str) {
    use std::process::Command;
    use std::process::Stdio;

    let cmd = format!("{}/{}", command_dir, cmd_file);
    let log_file = open_log_file(cmd_file).unwrap();
    let stderr_cmd = format!("{}-stderr", cmd_file);
    let log_file_stderr = log_file.try_clone().unwrap(); // open_log_file(&stderr_cmd).unwrap();
                                                         // let errors = outputs.try_clone()?;
    let mut child = Command::new(cmd)
        // .arg("--debug")
        // .arg(verb)
        .stdin(Stdio::null())
        .stdout(log_file)
        .stderr(log_file_stderr)
        .env("RUST_BACKTRACE", "1")
        .env("ARG_REF", arg_ref)
        .spawn()
        .expect("failed to execute child");
}
