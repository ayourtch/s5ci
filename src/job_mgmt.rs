use crate::runtime_data::s5ciRuntimeData;
use crate::s5ci_config::s5ciConfig;
use crate::unix_process::*;
use diesel;
use regex::Regex;
use s5ci::*;
use std::collections::HashMap;
use std::path::Path;

use crate::database::db_get_next_counter_value_with_min;
use crate::html_gen::finished_job;
use crate::html_gen::starting_job;

fn get_job_url(config: &s5ciConfig, rtdt: &s5ciRuntimeData, job_id: &str) -> String {
    format!("{}/{}/", config.jobs.root_url, job_id)
}
fn get_job_name(config: &s5ciConfig, rtdt: &s5ciRuntimeData, job_id: &str) -> String {
    let re = Regex::new(r"[^A-Za-z0-9_]").unwrap();

    let job_name = re.replace_all(&format!("{}", job_id), "_").to_string();
    job_name
}

pub fn job_group_name_from_cmd(cmd: &str) -> String {
    basename_from_cmd(cmd)
}

fn basename_from_cmd(cmd: &str) -> String {
    let mut cmd_pieces = cmd.split(' ');
    let verb: &str = match cmd_pieces.next() {
        Some(p) => p,
        None => cmd,
    };
    let path = Path::new(verb);
    format!("{}", path.file_name().unwrap().to_str().unwrap())
}

fn get_min_job_counter(config: &s5ciConfig, jobname: &str) -> i32 {
    use std::fs;
    let jobpath = format!("{}/{}", &config.jobs.rootdir, jobname);
    let path = Path::new(&jobpath);
    if !path.is_dir() {
        println!("Creating directory {}", &jobpath);
        fs::create_dir(&jobpath).unwrap();
    }
    let file_count = fs::read_dir(path).unwrap().count();
    file_count as i32
}

fn get_workspace_path(config: &s5ciConfig, job_id: &str) -> String {
    format!("{}/{}/workspace", &config.jobs.rootdir, job_id)
}

fn get_job_console_log_path(config: &s5ciConfig, job_id: &str) -> String {
    format!("{}/{}/console.txt", &config.jobs.rootdir, job_id)
}

fn get_existing_workspace_path(config: &s5ciConfig, job_id: &str) -> String {
    let workspace_path = get_workspace_path(config, job_id);
    let path = Path::new(&workspace_path);
    if !path.is_dir() {
        panic!(format!(
            "Path {} does not exist or is not directory",
            &workspace_path
        ));
    }
    workspace_path
}

fn get_next_job_number(config: &s5ciConfig, jobname: &str) -> i32 {
    use std::fs;
    let a_min = get_min_job_counter(config, jobname);
    let job_number = db_get_next_counter_value_with_min(jobname, a_min).unwrap();
    let job_id = format!("{}/{}", jobname, job_number);

    let jobpath = format!("{}/{}", &config.jobs.rootdir, jobname);
    let path = Path::new(&jobpath);
    if !path.is_dir() {
        fs::create_dir(&path).unwrap();
    }
    let new_path = format!("{}/{}", &config.jobs.rootdir, job_id);
    println!("CREATING DIR {}", &new_path);
    fs::create_dir(&new_path).unwrap();
    let workspace_path = get_workspace_path(config, &job_id);
    fs::create_dir(&workspace_path).unwrap();
    job_number
}

fn find_best_command(config: &s5ciConfig, job_group: &str) -> String {
    let mut best_command = job_group;
    debug!("Finding best command for {}", job_group);
    loop {
        debug!("Current best candidate: {}", &best_command);
        if best_command == "" {
            debug!("Best command candidate empty, return full command");
            return job_group.to_string();
        }
        let full_cmd = format!("{}/{}", &config.command_rootdir, &best_command);
        let fc_path = Path::new(&full_cmd);
        if fc_path.is_file() {
            debug!("Best command: {}", &best_command);
            return best_command.to_string();
        }
        let mut cut_len = best_command.len();
        while cut_len > 0 && &best_command[cut_len - 1..cut_len] != "-" {
            cut_len = cut_len - 1;
        }
        while cut_len > 0 && &best_command[cut_len - 1..cut_len] == "-" {
            cut_len = cut_len - 1;
        }
        best_command = &best_command[0..cut_len];
    }
}

fn prepare_child_command<'a>(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    child0: &'a mut std::process::Command,
    cmd: &str,
    suffix: &str,
) -> (String, i32, &'a mut std::process::Command) {
    use regex::Regex;
    use std::env;
    use std::process::Command;
    use std::process::Stdio;
    let args: Vec<String> = env::args().collect();

    // let re = Regex::new(r"[^A-Za-z0-9_]").unwrap();

    let job_group = basename_from_cmd(cmd);
    let job_nr = get_next_job_number(config, &job_group);
    let job_id = format!("{}/{}", job_group, job_nr);
    let log_fname = format!("{}/{}/console{}.txt", config.jobs.rootdir, job_id, suffix);
    println!("LOG file: {}", &log_fname);
    let log_file = open_log_file(&log_fname).unwrap();
    let stderr_cmd = format!("{}-stderr", job_group);
    let log_file_stderr = log_file.try_clone().unwrap(); // open_log_file(&stderr_cmd).unwrap();

    let cmd_file = find_best_command(config, &job_group);
    let final_cmd = if cmd_file != job_group {
        // insert a space at the appropriate place
        format!("{} {}", &cmd[0..cmd_file.len()], &cmd[cmd_file.len() + 1..])
    } else {
        cmd.to_string()
    };

    // let errors = outputs.try_clone()?;
    // let mut child0 = Command::new("/bin/sh");
    let mut child = child0
        .arg(format!("{}", final_cmd))
        .stdin(Stdio::null())
        .stdout(log_file)
        .stderr(log_file_stderr)
        .env("RUST_BACKTRACE", "1")
        .env(
            "PATH",
            &format!(
                "{}:{}",
                &config.command_rootdir,
                std::env::var("PATH").unwrap_or("".to_string())
            ),
        )
        .env("S5CI_EXE", &rtdt.real_s5ci_exe)
        .env("S5CI_JOB_ID", &job_id)
        .env(
            "S5CI_WORKSPACE",
            &get_existing_workspace_path(config, &job_id),
        )
        .env(
            "S5CI_CONSOLE_LOG",
            &get_job_console_log_path(config, &job_id),
        )
        .env("S5CI_JOB_NAME", &get_job_name(config, rtdt, &job_id))
        .env("S5CI_JOB_URL", &get_job_url(config, rtdt, &job_id))
        .env("S5CI_SANDBOX_LEVEL", format!("{}", rtdt.sandbox_level))
        .env("S5CI_CONFIG", &rtdt.config_path);

    // see if we can stuff the parent job variables
    let env_pj_id = env::var("S5CI_JOB_ID");
    let env_pj_name = env::var("S5CI_JOB_NAME");
    let env_pj_url = env::var("S5CI_JOB_URL");
    if env_pj_id.is_ok() && env_pj_name.is_ok() && env_pj_url.is_ok() {
        child = child
            .env("S5CI_PARENT_JOB_ID", env_pj_id.unwrap())
            .env("S5CI_PARENT_JOB_NAME", env_pj_name.unwrap())
            .env("S5CI_PARENT_JOB_URL", env_pj_url.unwrap());
    }
    if let Some(trig_ev) = &rtdt.trigger_event_id {
        child = child.env("S5CI_TRIGGER_EVENT_ID", trig_ev.clone());
    }
    return (job_group, job_nr, child0);
}

/* background run a command, simply starts S5CI with a request to run a job in foreground */

pub fn spawn_command(config: &s5ciConfig, rtdt: &s5ciRuntimeData, cmd: &str) {
    use std::env;
    use std::process::Command;
    let args: Vec<String> = env::args().collect();
    let env_changeset_id = format!("{}", rtdt.changeset_id.unwrap_or(0));
    let env_patchset_id = format!("{}", rtdt.patchset_id.unwrap_or(0));
    let mut child0 = Command::new(&args[0]);
    let mut child = child0
        .arg("run-job")
        .arg("-c")
        .arg(format!("{}", cmd))
        .arg("-k")
        .env("S5CI_CONFIG", &rtdt.config_path)
        .env("S5CI_GERRIT_CHANGESET_ID", &env_changeset_id)
        .env("S5CI_GERRIT_PATCHSET_ID", &env_patchset_id)
        .env("S5CI_GERRIT_COMMENT_VALUE", &rtdt.comment_value);
    if let Some(a_trig) = &rtdt.trigger_event_id {
        println!("Set the event id to: {}", &a_trig);
        child = child.env("S5CI_TRIGGER_EVENT_ID", &a_trig);
    }
    println!("Spawning {:#?}", child);
    if rtdt.sandbox_level < 2 {
        let res = child.spawn().expect("failed to execute child");
        println!("Spawned pid {}", res.id());
    } else {
        println!(
            "Sandbox level {}, not actually spawning a child",
            &rtdt.sandbox_level
        );
    }
}

pub fn db_set_job_finished(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    a_job_id: &str,
    status_code: Option<i32>,
) {
    let db = get_db();
    println!("Set job {} as finished with status {:?}", a_job_id, &status_code);
    {
        use diesel::expression_methods::*;
        use diesel::query_dsl::QueryDsl;
        use diesel::query_dsl::RunQueryDsl;
        use schema::jobs;
        use schema::jobs::dsl::*;

        let some_ndt_now = Some(now_naive_date_time());
        let a_status_success = (status_code.unwrap_or(4242) == 0);

        let updated_rows = diesel::update(jobs.filter(job_id.eq(&a_job_id)))
            .set((
                finished_at.eq(some_ndt_now),
                return_success.eq(a_status_success),
                return_code.eq(status_code),
            ))
            .execute(db.conn())
            .unwrap();
    }
    finished_job(config, rtdt, a_job_id);
}

/* foreground run a command */

pub fn exec_command(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    cmd: &str,
) -> (String, Option<i32>) {
    use std::env;
    use std::process::Command;
    use uuid::Uuid;

    let env_changeset_id = rtdt.changeset_id.unwrap() as i32;
    let env_patchset_id = rtdt.patchset_id.unwrap() as i32;
    let mut child0 = Command::new("/bin/sh");
    let mut child = child0.arg("-c");
    let (a_job_group_name, a_instance_id, mut child) =
        prepare_child_command(config, rtdt, child, cmd, "");
    let a_full_job_id = format!("{}/{}", &a_job_group_name, a_instance_id);

    /* change dir to job workspace */
    std::env::set_current_dir(&get_existing_workspace_path(config, &a_full_job_id)).unwrap();

    let my_uuid = Uuid::new_v4().to_simple().to_string();
    /* in our environment the job ID, if set, is set by parent */
    let env_pj_id = env::var("S5CI_JOB_ID").ok();

    let mut new_job = models::job {
        record_uuid: my_uuid.clone(),
        job_group_name: a_job_group_name,
        instance_id: a_instance_id,
        job_id: a_full_job_id.clone(),
        job_pid: mypid() as i32,
        parent_job_id: env_pj_id.clone(),
        changeset_id: env_changeset_id,
        patchset_id: env_patchset_id,
        command: format!("{}", cmd),
        command_pid: None,
        status_message: format!(""),
        status_updated_at: None,
        remote_host: None,
        started_at: Some(now_naive_date_time()),
        finished_at: None,
        return_success: false,
        return_code: None,
        trigger_event_id: rtdt.trigger_event_id.clone(),
        ..Default::default()
    };
    let db = get_db();
    {
        use diesel::query_dsl::RunQueryDsl;
        use schema::jobs;
        use schema::jobs::dsl::*;

        diesel::insert_into(jobs::table)
            .values(&new_job)
            .execute(db.conn())
            .expect(&format!("Error inserting new job {}", &a_full_job_id));
    }
    println!("Executing {}", &a_full_job_id);
    setsid();
    let mut child_spawned = child.spawn().expect("failed to execute process");
    {
        use diesel::expression_methods::*;
        use diesel::query_dsl::QueryDsl;
        use diesel::query_dsl::RunQueryDsl;
        use schema::jobs;
        use schema::jobs::dsl::*;

        let updated_rows = diesel::update(jobs.filter(record_uuid.eq(&my_uuid)))
            .set((command_pid.eq(Some(child_spawned.id() as i32)),))
            .execute(db.conn())
            .unwrap();
    }
    starting_job(config, rtdt, &a_full_job_id);
    use std::process::ExitStatus;
    let mut maybe_status: Option<ExitStatus> = None;

    loop {
        match child_spawned.try_wait() {
            Ok(Some(status)) => {
                /* done */
                maybe_status = Some(status);
                break;
            }
            Ok(None) => {
                debug!("Status not ready yet from pid {}", child_spawned.id());
            }
            Err(e) => {
                panic!("Error attempting to wait: {:?}", e);
            }
        }
        s5ci::thread_sleep_ms(5000);
    }
    let status = maybe_status.unwrap();
    match status.code() {
        Some(code) => println!("Finished {} with status code {}", &a_full_job_id, code),
        None => println!("Finished {} due to signal", &a_full_job_id),
    }
    db_set_job_finished(config, rtdt, &a_full_job_id, status.code());
    /*
    {
        use diesel::expression_methods::*;
        use diesel::query_dsl::QueryDsl;
        use diesel::query_dsl::RunQueryDsl;
        use schema::jobs;
        use schema::jobs::dsl::*;

        let some_ndt_now = Some(now_naive_date_time());
        let a_status_success = (status.code().unwrap_or(4242) == 0);

        let updated_rows = diesel::update(jobs.filter(record_uuid.eq(&my_uuid)))
            .set((
                finished_at.eq(some_ndt_now),
                return_success.eq(a_status_success),
                return_code.eq(status.code()),
            ))
            .execute(db.conn())
            .unwrap();
    }
    finished_job(config, rtdt, &a_full_job_id);
    */
    return (a_full_job_id, status.code());
}
