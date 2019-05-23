use crate::runtime_data::s5ciRuntimeData;
use crate::s5ci_config::s5ciConfig;
use s5ci::now_naive_date_time;
use std::time::SystemTime;

pub struct AutorestartState {
    pub config_mtime: Option<SystemTime>,
    pub exe_mtime: Option<SystemTime>,
}

fn restart_ourselves() {
    use std::env;
    use std::process;
    let argv_real: Vec<String> = env::args().collect();
    let err = exec::Command::new(&argv_real[0])
        .args(&argv_real[1..])
        .exec();
    // normally not reached
    println!("Error: {}", err);
    process::exit(1);
}

fn get_mtime(fname: &str) -> Option<std::time::SystemTime> {
    let mtime = std::fs::metadata(fname).ok().map(|x| x.modified().unwrap());
    mtime
}

fn file_changed_since(fname: &str, since: Option<std::time::SystemTime>) -> bool {
    use std::time::{Duration, SystemTime};
    let new_mtime = get_mtime(fname);
    let few_seconds = Duration::from_secs(10);
    if let (Some(old_t), Some(new_t)) = (since, new_mtime) {
        if new_t.duration_since(old_t).unwrap_or(few_seconds) > few_seconds {
            return true;
        }
    }
    // be conservative if we didn't have either of mtimes
    return false;
}

pub fn autorestart_init(config: &s5ciConfig, rtdt: &s5ciRuntimeData) -> AutorestartState {
    let config_mtime = get_mtime(&rtdt.config_path);
    let exe_mtime = get_mtime(&rtdt.real_s5ci_exe);
    AutorestartState {
        config_mtime,
        exe_mtime,
    }
}

pub fn autorestart_check(config: &s5ciConfig, rtdt: &s5ciRuntimeData, ars: &AutorestartState) {
    if config.autorestart.on_config_change
        && file_changed_since(&rtdt.config_path, ars.config_mtime)
    {
        println!(
            "Config changed, attempt restart at {}...",
            now_naive_date_time()
        );
        restart_ourselves();
    }
    if config.autorestart.on_exe_change && file_changed_since(&rtdt.real_s5ci_exe, ars.exe_mtime) {
        println!(
            "Executable changed, attempt restart at {}... ",
            now_naive_date_time()
        );
        restart_ourselves();
    }
}
