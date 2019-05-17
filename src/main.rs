use ssh2::Session;
use std::collections::HashMap;
use std::io;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;

#[macro_use]
extern crate clap;
extern crate exec;
extern crate libc;
extern crate psutil;
extern crate regex;
extern crate signal_hook;
extern crate uuid;
extern crate yaml_rust;
#[macro_use]
extern crate log;
extern crate env_logger;
use clap::{App, Arg, SubCommand};

use regex::Regex;

use std::fs;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate serde_yaml;

extern crate cron;
extern crate mustache;

use chrono::NaiveDateTime;

mod gerrit_types;
mod gerrit_interact;
mod s5ci_config;
mod run_ssh_command;
mod unix_process;
mod runtime_data;
mod database;
mod comment_triggers;

use crate::gerrit_types::*;
use crate::s5ci_config::*;
use crate::run_ssh_command::*;
use crate::unix_process::*;
use crate::runtime_data::*;
use crate::gerrit_interact::*;
use crate::database::*;
use crate::comment_triggers::*;

use s5ci::*;

fn get_job_url(config: &s5ciConfig, rtdt: &s5ciRuntimeData, job_id: &str) -> String {
    format!("{}/{}/", config.jobs.root_url, job_id)
}
fn get_job_name(config: &s5ciConfig, rtdt: &s5ciRuntimeData, job_id: &str) -> String {
    let re = Regex::new(r"[^A-Za-z0-9_]").unwrap();

    let job_name = re.replace_all(&format!("{}", job_id), "_").to_string();
    job_name
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

    let cmd_file = basename_from_cmd(cmd);
    let job_nr = get_next_job_number(config, &cmd_file);
    let job_id = format!("{}/{}", cmd_file, job_nr);
    let log_fname = format!("{}/{}/console{}.txt", config.jobs.rootdir, job_id, suffix);
    println!("LOG file: {}", &log_fname);
    let log_file = open_log_file(&log_fname).unwrap();
    let stderr_cmd = format!("{}-stderr", cmd_file);
    let log_file_stderr = log_file.try_clone().unwrap(); // open_log_file(&stderr_cmd).unwrap();

    // let errors = outputs.try_clone()?;
    // let mut child0 = Command::new("/bin/sh");
    let mut child = child0
        .arg(format!("{}", cmd))
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
    return (cmd_file, job_nr, child0);
}

fn spawn_command(config: &s5ciConfig, rtdt: &s5ciRuntimeData, cmd: &str) {
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
        .env("S5CI_GERRIT_PATCHSET_ID", &env_patchset_id);
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

fn db_get_next_counter_value(a_name: &str) -> Result<i32, String> {
    db_get_next_counter_value_with_min(a_name, 0)
}

fn get_lock_path(a_name: &str) -> String {
    let lock_path = format!("/tmp/{}.lock", &a_name);
    lock_path
}

fn lock_named(a_name: &str) -> Result<(), String> {
    let lock_path = get_lock_path(a_name);
    let max_retry_count = 20;
    let mut retry_count = max_retry_count;
    while std::fs::create_dir(&lock_path).is_err() {
        if retry_count == 0 {
            return Err(format!("Failed to lock {} after several tries", a_name));
        }
        // wait for sometime
        s5ci::thread_sleep_ms(300 * (1 + max_retry_count - retry_count));
        retry_count = retry_count - 1;
    }
    Ok(())
}

fn unlock_named(a_name: &str) -> Result<(), String> {
    let lock_path = get_lock_path(a_name);
    if std::fs::remove_dir(&lock_path).is_ok() {
        Ok(())
    } else {
        Err(format!("error unlocking {}", a_name))
    }
}

fn db_get_next_counter_value_with_min(a_name: &str, a_min: i32) -> Result<i32, String> {
    use diesel::connection::Connection;
    use diesel::expression_methods::*;
    use diesel::query_dsl::QueryDsl;
    use diesel::query_dsl::RunQueryDsl;
    use diesel::result::Error;
    use schema::counters;
    use schema::counters::dsl::*;

    let db = get_db();
    let conn = db.conn();
    let mut result: Result<i32, String> = Err(format!("result unset"));
    lock_named(&a_name).unwrap();

    conn.transaction::<_, Error, _>(|| {
        let res = counters
            .filter(name.eq(a_name))
            .limit(2)
            .load::<models::counter>(conn);

        let count_val: Result<i32, String> = match res {
            Ok(r) => match r.len() {
                0 => {
                    let curr_value = a_min;
                    let new_counter = models::counter {
                        name: format!("{}", a_name),
                        value: curr_value + 1,
                    };
                    diesel::insert_into(counters::table)
                        .values(&new_counter)
                        .execute(conn);
                    Ok(curr_value)
                }
                1 => {
                    let curr_val = if r[0].value < a_min {
                        a_min
                    } else {
                        r[0].value
                    };

                    diesel::update(counters.filter(name.eq(a_name)))
                        .set((value.eq(curr_val + 1)))
                        .execute(conn);
                    Ok(curr_val)
                }
                _ => Err(format!("More than one counter of type {}", a_name)),
            },
            Err(e) => Err(format!("counter select error: {:?}", &e)),
        };
        result = count_val;
        Ok(())
    })
    .unwrap();

    if result.is_ok() {
        let r = counters
            .filter(name.eq(a_name))
            .limit(1)
            .load::<models::counter>(conn)
            .unwrap();
        if Ok(r[0].value - 1) != result {
            /* there was another transaction in parallel, retry */
            s5ci::thread_sleep_ms(r[0].value as u64 % 100);
            result = Ok(db_get_next_counter_value_with_min(a_name, a_min).unwrap());
        }
    }
    unlock_named(&a_name).unwrap();
    result
}

use mustache::{MapBuilder, Template};

fn maybe_compile_template(config: &s5ciConfig, name: &str) -> Result<Template, mustache::Error> {
    let res = mustache::compile_path(format!(
        "{}/templates/{}.mustache",
        &config.install_rootdir, name
    ));
    if res.is_err() {
        error!("Could not compile template {}: {:#?}", name, &res);
    }
    res
}

fn fill_and_write_template(
    template: &Template,
    data: MapBuilder,
    fname: &str,
) -> std::io::Result<()> {
    let mut bytes = vec![];
    let data_built = data.build();
    template
        .render_data(&mut bytes, &data_built)
        .expect("Failed to render");
    let payload = std::str::from_utf8(&bytes).unwrap();
    let res = fs::write(&fname, payload);
    res
}

const jobs_per_page: usize = 20;

fn regenerate_group_html(config: &s5ciConfig, rtdt: &s5ciRuntimeData, group_name: &str) {
    let template = maybe_compile_template(config, "group_job_page").unwrap();
    let mut data = MapBuilder::new();
    let jobs = db_get_jobs_by_group_name(group_name);
    data = data.insert("job_group_name", &group_name).unwrap();
    data = data.insert("child_jobs", &jobs).unwrap();
    let fname = format!("{}/{}/index.html", &config.jobs.rootdir, group_name);
    fill_and_write_template(&template, data, &fname).unwrap();
}

fn regenerate_root_html(config: &s5ciConfig, rtdt: &s5ciRuntimeData) {
    let template = maybe_compile_template(config, "root_job_page").unwrap();
    let rjs = db_get_root_jobs();
    let total_jobs = rjs.len();
    let max_n_first_page_jobs = total_jobs % jobs_per_page + jobs_per_page;
    let n_nonfirst_pages = if total_jobs > max_n_first_page_jobs {
        (total_jobs - max_n_first_page_jobs) / jobs_per_page
    } else {
        0
    };
    let n_first_page_jobs = if total_jobs > max_n_first_page_jobs {
        max_n_first_page_jobs
    } else {
        total_jobs
    };

    let mut data = MapBuilder::new();
    let fname = format!("{}/index.html", &config.jobs.rootdir);
    if n_first_page_jobs < total_jobs {
        let first_prev_page_number = (total_jobs - n_first_page_jobs) / jobs_per_page;
        let mut prev_page_number = first_prev_page_number;
        let prev_page_name = format!("index_{}.html", prev_page_number);
        data = data.insert("prev_page_name", &prev_page_name).unwrap();

        loop {
            let mut data_n = MapBuilder::new();
            let curr_page_number = prev_page_number;
            if curr_page_number == 0 {
                break;
            }
            prev_page_number = prev_page_number - 1;
            let next_page_number = curr_page_number + 1;
            if prev_page_number > 0 {
                let prev_page_name = format!("index_{}.html", prev_page_number);
                data_n = data_n.insert("prev_page_name", &prev_page_name).unwrap();
            }
            let next_page_name = if next_page_number <= first_prev_page_number {
                format!("index_{}.html", next_page_number)
            } else {
                format!("index.html")
            };
            data_n = data_n.insert("next_page_name", &next_page_name).unwrap();
            let start_job =
                n_first_page_jobs + (n_nonfirst_pages - curr_page_number) * jobs_per_page;
            let end_job = start_job + jobs_per_page;

            let cjobs = rjs[start_job..end_job].to_vec();

            data_n = data_n.insert("child_jobs", &cjobs).unwrap();

            let fname = format!("{}/index_{}.html", &config.jobs.rootdir, curr_page_number);
            debug!("writing template {}", &fname);
            fill_and_write_template(&template, data_n, &fname).unwrap();
        }
    }
    let cjobs = rjs[0..n_first_page_jobs].to_vec();
    data = data.insert("child_jobs", &cjobs).unwrap();
    fill_and_write_template(&template, data, &fname).unwrap();
}

fn regenerate_active_html(config: &s5ciConfig, rtdt: &s5ciRuntimeData) {
    let template = maybe_compile_template(config, "active_job_page").unwrap();
    let mut data = MapBuilder::new();
    let rjs = db_get_active_jobs();
    data = data.insert("child_jobs", &rjs).unwrap();
    let fname = format!("{}/active.html", &config.jobs.rootdir);
    fill_and_write_template(&template, data, &fname).unwrap();
}

fn regenerate_html(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    job_id: &str,
    update_parent: bool,
    update_children: bool,
    groups: &mut HashMap<String, i32>,
) {
    use mustache::{Data, MapBuilder};
    let j = db_get_job(job_id).expect(&format!("Could not get job id {} from db", job_id));
    let template = maybe_compile_template(config, "job_page").unwrap();
    let mut data = MapBuilder::new();

    data = data.insert("job", &j).unwrap();
    if let Some(pjob_id) = &j.parent_job_id {
        let pj = db_get_job(pjob_id).expect(&format!("Could not get job id {} from db", pjob_id));
        data = data.insert("parent_job", &pj).unwrap();
    }
    let cjs = db_get_child_jobs(job_id);
    data = data.insert("child_jobs", &cjs).unwrap();

    let fname = format!("{}/{}/index.html", &config.jobs.rootdir, job_id);
    fill_and_write_template(&template, data, &fname).unwrap();

    if update_children {
        for cj in cjs {
            regenerate_html(config, rtdt, &cj.job_id, false, false, groups);
        }
    }

    if update_parent {
        if let Some(pjob_id) = &j.parent_job_id {
            regenerate_html(config, rtdt, pjob_id, false, false, groups);
        } else {
            regenerate_root_html(config, rtdt);
        }
    }
    groups.insert(
        j.job_group_name.clone(),
        1 + groups.get(&j.job_group_name).unwrap_or(&0),
    );
}

fn regenerate_job_html(config: &s5ciConfig, rtdt: &s5ciRuntimeData, job_id: &str) {
    let mut groups = HashMap::new();
    regenerate_html(config, rtdt, job_id, true, true, &mut groups);
    for (group_name, count) in groups {
        println!("Regenerating group {} with {} jobs", &group_name, count);
        regenerate_group_html(config, rtdt, &group_name);
    }
    regenerate_active_html(config, rtdt);
}

fn starting_job(config: &s5ciConfig, rtdt: &s5ciRuntimeData, job_id: &str) {
    regenerate_job_html(config, rtdt, job_id);
}

fn finished_job(config: &s5ciConfig, rtdt: &s5ciRuntimeData, job_id: &str) {
    regenerate_job_html(config, rtdt, job_id);
}

fn regenerate_all_html(config: &s5ciConfig, rtdt: &s5ciRuntimeData) {
    let jobs = db_get_all_jobs();
    let mut groups = HashMap::new();
    for j in jobs {
        // println!("Regenerate HTML for {}", &j.job_id);
        regenerate_html(config, rtdt, &j.job_id, false, false, &mut groups);
    }
    for (group_name, count) in groups {
        println!("Regenerating group {} with {} jobs", &group_name, count);
        regenerate_group_html(config, rtdt, &group_name);
    }
    regenerate_root_html(config, rtdt);
    regenerate_active_html(config, rtdt);
}

fn exec_command(
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
    return (a_full_job_id, status.code());
}

fn process_change(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    cs: &GerritChangeSet,
    before_when: Option<NaiveDateTime>,
    after_when: Option<NaiveDateTime>,
) {
    let mut triggers: Vec<CommentTrigger> = vec![];
    let mut max_pset = 0;

    // eprintln!("Processing change: {:#?}", cs);
    if let Some(startline) = after_when {
        let startline_ts =
            startline.timestamp() - 1 + config.default_regex_trigger_delay_sec.unwrap_or(0) as i64;

        debug!("process change with startline timestamp: {}", startline.timestamp());
        debug!("process change with startline_ts: {}", &startline_ts);

        let mut psmap: HashMap<String, GerritPatchSet> = HashMap::new();

        if let Some(psets) = &cs.patchSets {
            for pset in psets {
                if pset.createdOn > 0 {
                    // startline_ts {
                    // println!("{:?}", &pset);
                    debug!(
                        "  #{} revision: {} ref: {}",
                        &pset.number, &pset.revision, &pset.r#ref
                    );
                    // spawn_command_x("scripts", "git-test", &pset.r#ref);
                }
                psmap.insert(format!("{}", &pset.number), pset.clone());
                psmap.insert(format!("{}", &pset.revision), pset.clone());
                if pset.number > max_pset {
                    max_pset = pset.number;
                }
            }

            // eprintln!("Patchset map: {:#?}", &psmap);
        }
        if let Some(comments_vec) = &cs.comments {
            let change_id = cs.number.unwrap() as i32;
            let all_triggers = get_comment_triggers_from_comments(
                config,
                rtdt,
                change_id,
                max_pset,
                comments_vec,
                startline_ts,
            );
            let mut final_triggers = all_triggers.clone();
            let mut suppress_map: HashMap<(String, u32), bool> = HashMap::new();
            for mut ctrig in final_triggers.iter_mut().rev() {
                let key = (ctrig.trigger_name.clone(), ctrig.patchset_id);
                if ctrig.is_suppress {
                    suppress_map.insert(key, true);
                } else if suppress_map.contains_key(&key) {
                    ctrig.is_suppressed = true;
                    suppress_map.remove(&key);
                }
            }
            if let Some(cfgt) = &config.triggers {
                final_triggers.retain(|x| {
                    let ctrig = &cfgt[&x.trigger_name];
                    let mut retain = !x.is_suppressed;
                    if let Some(proj) = &ctrig.project {
                        if let Some(cs_proj) = &cs.project {
                            if cs_proj != proj {
                                retain = false;
                            }
                        } else {
                            retain = false;
                        }
                    }
                    if let s5TriggerAction::command(cmd) = &ctrig.action {
                        retain
                    } else {
                        false
                    }
                });
                // now purge all the suppressing triggers themselves
                final_triggers.retain(|x| !x.is_suppress);
            }
            // eprintln!("all triggers: {:#?}", &final_triggers);
            eprintln!("final triggers: {:#?}", &final_triggers);
            for trig in &final_triggers {
                let template = rtdt
                    .trigger_command_templates
                    .get(&trig.trigger_name)
                    .unwrap();
                let mut data = mustache::MapBuilder::new();
                if let Some(patchset) = psmap.get(&format!("{}", trig.patchset_id)) {
                    data = data.insert("patchset", &patchset).unwrap();
                }
                data = data.insert("regex", &trig.captures).unwrap();
                let data = data.build();
                let mut bytes = vec![];

                template.render_data(&mut bytes, &data).unwrap();
                let expanded_command = String::from_utf8_lossy(&bytes);
                let change_id = cs.number.unwrap();
                let mut rtdt2 = rtdt.clone();
                rtdt2.changeset_id = Some(change_id);
                rtdt2.patchset_id = Some(trig.patchset_id);
                if (trig.is_suppress || trig.is_suppressed) {
                    panic!(format!("bug: job is not runnable: {:#?}", &trig));
                }
                let job_id = spawn_command(config, &rtdt2, &expanded_command);
            }
        }
    }
}

fn print_process(p: &psutil::process::Process) {
    println!(
        "{:>5} {:>5} {:^5} {:>8.2} {:>8.2} {:.100}",
        p.pid,
        p.ppid,
        p.state.to_string(),
        p.utime,
        p.stime,
        p.cmdline()
            .unwrap_or_else(|_| Some("no-command-line".to_string()))
            .unwrap_or_else(|| format!("[{}]", p.comm))
    );
}

fn ps() {
    println!(
        "{:>5} {:>5} {:^5} {:>8} {:>8} {:.100}",
        "PID", "PPID", "STATE", "UTIME", "STIME", "CMD"
    );

    if let Ok(processes) = &psutil::process::all() {
        for p in processes {
            print_process(p);
        }
    } else {
        println!("--- could not do ps ---");
    }
}

fn get_configs() -> (s5ciConfig, s5ciRuntimeData) {
    let matches = App::new("S5CI - S<imple> CI")
        .version("0.5")
        .author("Andrew Yourtchenko <ayourtch@gmail.com>")
        .about("A simple CI daemon")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .env("S5CI_CONFIG")
                .required(true)
                .takes_value(true)
                .help("Set custom config file"),
        )
        .arg(
            Arg::with_name("sandbox-level")
                .short("s")
                .env("S5CI_SANDBOX_LEVEL")
                .default_value("0")
                .possible_values(&["0", "1", "2", "3"])
                .help("Sandbox - inhibit various actions"),
        )
        .subcommand(SubCommand::with_name("list-jobs").about("list jobs"))
        .subcommand(SubCommand::with_name("check-config").about("check config, return 0 if ok"))
        .subcommand(
            SubCommand::with_name("kill-job")
                .about("kill a running job")
                .arg(
                    Arg::with_name("job-id")
                        .short("j")
                        .help("job-id to kill")
                        .required(true)
                        .takes_value(true),
                )
                )
        .subcommand(
            SubCommand::with_name("run-job")
                .about("run a job")
                .arg(
                    Arg::with_name("command")
                        .short("c")
                        .help("command to run")
                        .required(true)
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("omit-if-ok")
                        .short("o")
                        .help("omit the run if previous is a successful run of the same job type on the same change+patch")
                )
                .arg(
                    Arg::with_name("kill-previous")
                        .short("k")
                        .help("kill previous job of this type on the same change+patch if it is still running")
                )
                .arg(
                    Arg::with_name("changeset-id")
                        .short("s")
                        .help("changeset ID")
                        .required(true)
                        .env("S5CI_GERRIT_CHANGESET_ID")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("patchset-id")
                        .short("p")
                        .help("patchset ID")
                        .required(true)
                        .env("S5CI_GERRIT_PATCHSET_ID")
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("gerrit-command")
                .about("run arbitrary command")
                .arg(
                    Arg::with_name("command")
                        .short("c")
                        .help("command to run")
                        .required(true)
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("set-status")
                .about("set the job status to message")
                .arg(
                    Arg::with_name("message")
                        .short("m")
                        .help("message to add in a review")
                        .required(true)
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("job-id")
                        .short("j")
                        .help("job ID (group_name/number)")
                        .required(true)
                        .env("S5CI_JOB_ID")
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("review")
                .about("review with comment and maybe vote")
                .arg(
                    Arg::with_name("message")
                        .short("m")
                        .help("message to add in a review")
                        .required(true)
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("changeset-id")
                        .short("s")
                        .help("changeset ID")
                        .required(true)
                        .env("S5CI_GERRIT_CHANGESET_ID")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("patchset-id")
                        .short("p")
                        .help("patchset ID")
                        .required(true)
                        .env("S5CI_GERRIT_PATCHSET_ID")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("vote")
                        .short("v")
                        .help("vote success, failure, or clear")
                        .possible_values(&["success", "failure", "clear"])
                        .takes_value(true),
                ),
        )
        .get_matches();

    let yaml_fname = &matches.value_of("config").unwrap().to_string();
    // canonicalize config name
    let yaml_fname = std::fs::canonicalize(yaml_fname)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let s = fs::read_to_string(&yaml_fname).unwrap();
    let config: s5ciConfig = serde_yaml::from_str(&s).unwrap();
    debug!("Config: {:#?}", &config);
    set_db_url(&config.db_url);

    let args: Vec<String> = std::env::args().collect();
    let real_s5ci_exe = std::fs::canonicalize(args[0].clone())
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let trigger_regexes = get_trigger_regexes(&config);
    let cron_trigger_schedules = get_cron_trigger_schedules(&config);
    let patchset_extract_regex = Regex::new(&config.patchset_extract_regex).unwrap();
    let trigger_command_templates = get_trigger_command_templates(&config);
    let mut changeset_id: Option<u32> = None;
    let mut patchset_id: Option<u32> = None;
    let sandbox_level = value_t!(matches, "sandbox-level", u32).unwrap_or(0);

    let mut action = s5ciAction::Loop;

    if let Some(matches) = matches.subcommand_matches("gerrit-command") {
        let cmd = matches.value_of("command").unwrap().to_string();
        action = s5ciAction::GerritCommand(cmd);
    }
    if let Some(matches) = matches.subcommand_matches("kill-job") {
        let jobid = matches.value_of("job-id").unwrap().to_string();
        action = s5ciAction::KillJob(jobid);
    }
    if let Some(matches) = matches.subcommand_matches("run-job") {
        let cmd = matches.value_of("command").unwrap().to_string();
        patchset_id = Some(
            matches
                .value_of("patchset-id")
                .unwrap()
                .to_string()
                .parse::<u32>()
                .unwrap(),
        );
        changeset_id = Some(
            matches
                .value_of("changeset-id")
                .unwrap()
                .to_string()
                .parse::<u32>()
                .unwrap(),
        );
        let omit_if_ok = matches.is_present("omit-if-ok");
        let kill_previous = matches.is_present("kill-previous");
        action = s5ciAction::RunJob(s5ciRunJobArgs {
            cmd,
            omit_if_ok,
            kill_previous,
        });
    }
    if let Some(matches) = matches.subcommand_matches("list-jobs") {
        action = s5ciAction::ListJobs;
    }
    if let Some(matches) = matches.subcommand_matches("check-config") {
        // we already checked the config when loading. So if we are here, just exit with success
        std::process::exit(0);
    }
    if let Some(matches) = matches.subcommand_matches("set-status") {
        let msg = matches.value_of("message").unwrap().to_string();
        let job_id = matches.value_of("job-id").unwrap().to_string();
        action = s5ciAction::SetStatus(job_id, msg);
    }
    if let Some(matches) = matches.subcommand_matches("review") {
        let msg = matches.value_of("message").unwrap().to_string();

        let vote_value = if matches.value_of("vote").is_some() {
            let val = value_t!(matches, "vote", GerritVoteAction).unwrap();
            Some(val)
        } else {
            None
        };
        action = s5ciAction::MakeReview(vote_value, msg);
        patchset_id = Some(
            matches
                .value_of("patchset-id")
                .unwrap()
                .to_string()
                .parse::<u32>()
                .unwrap(),
        );
        changeset_id = Some(
            matches
                .value_of("changeset-id")
                .unwrap()
                .to_string()
                .parse::<u32>()
                .unwrap(),
        );
    }

    let unsafe_char_regex = Regex::new(r"([^-/_A-Za-z0-9])").unwrap();
    let unsafe_start_regex = Regex::new(r"^[^_A-Za-z0-9]").unwrap();

    let rtdt = s5ciRuntimeData {
        config_path: yaml_fname.to_string(),
        sandbox_level,
        patchset_extract_regex,
        unsafe_char_regex,
        unsafe_start_regex,
        trigger_regexes,
        trigger_command_templates,
        cron_trigger_schedules,
        action,
        changeset_id,
        patchset_id,
        real_s5ci_exe,
    };
    debug!("C-Config: {:#?}", &rtdt);
    (config, rtdt)
}

fn do_gerrit_command(config: &s5ciConfig, rtdt: &s5ciRuntimeData, cmd: &str) {
    run_ssh_command(config, cmd);
}

fn do_review(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    maybe_vote: &Option<GerritVoteAction>,
    msg: &str,
) {
    let mut vote = if let Some(act) = maybe_vote {
        let active_vote = match act {
            GerritVoteAction::success => format!(" {}", &config.default_vote.success),
            GerritVoteAction::failure => format!(" {}", &config.default_vote.failure),
            GerritVoteAction::clear => format!(" {}", &config.default_vote.clear),
        };
        if rtdt.sandbox_level > 1 {
            error!(
                "Sandbox level {}, ignoring the voting arg '{}'",
                rtdt.sandbox_level, &active_vote
            );
            format!("")
        } else {
            active_vote
        }
    } else {
        format!("")
    };
    let patchset_id = rtdt.patchset_id.unwrap();
    let cmd = if patchset_id == 0 {
        format!(
            "gerrit review {} {} --message \"{}\"",
            rtdt.changeset_id.unwrap(),
            vote,
            msg
        )
    } else {
        format!(
            "gerrit review {},{} {} --message \"{}\"",
            rtdt.changeset_id.unwrap(),
            patchset_id,
            vote,
            msg
        )
    };
    if rtdt.sandbox_level > 0 {
        error!(
            "Sandbox level {}, not running command '{}'",
            rtdt.sandbox_level, &cmd
        );
    } else {
        run_ssh_command(config, &cmd);
    }
}

fn do_list_jobs(config: &s5ciConfig, rtdt: &s5ciRuntimeData) {
    let jobs = db_get_all_jobs();
    for j in jobs {
        if j.finished_at.is_some() {
            // show jobs finished up to 10 seconds ago
            let ndt_horizon = ndt_add_seconds(now_naive_date_time(), -10);
            if j.finished_at.clone().unwrap() < ndt_horizon {
                continue;
            }
        }
        println!("{:#?}", &j);
    }
}
fn do_kill_job(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    jobid: &str,
    terminator: &str,
) {
    let job = db_get_job(jobid).unwrap();
    if job.finished_at.is_none() {
        if let Some(pid) = job.command_pid {
            println!(
                "Requested to kill a job, sending signal to pid {} from job {:?}",
                pid, &job
            );
            kill_process(pid);
            do_set_job_status(
                config,
                rtdt,
                &job.job_id,
                &format!("Terminated by the {}", terminator),
            );
        }
    }
}

fn do_run_job(config: &s5ciConfig, rtdt: &s5ciRuntimeData, args: &s5ciRunJobArgs) {
    use signal_hook::{iterator::Signals, SIGABRT, SIGHUP, SIGINT, SIGPIPE, SIGQUIT, SIGTERM};
    use std::{error::Error, thread};
    let cmd = &args.cmd;

    let signals = Signals::new(&[SIGINT, SIGPIPE, SIGHUP, SIGQUIT, SIGABRT, SIGTERM]).unwrap();

    thread::spawn(move || {
        for sig in signals.forever() {
            println!("Received signal {:?}", sig);
        }
    });
    println!("Requested to run job '{}'", cmd);
    let group_name = basename_from_cmd(cmd);
    let jobs = db_get_jobs_by_group_name_and_csps(
        &group_name,
        rtdt.changeset_id.unwrap(),
        rtdt.patchset_id.unwrap(),
    );
    if args.omit_if_ok {
        if jobs.len() > 0 {
            if jobs[0].return_success {
                println!("Requested to omit if success, existing success job: {:?}, exit no-op with success", &jobs[0]);
                std::process::exit(0);
            }
        }
    }
    if args.kill_previous {
        if jobs.len() > 0 && jobs[0].finished_at.is_none() {
            do_kill_job(config, rtdt, &jobs[0].job_id, "next job");
        }
    }
    let (job_id, status) = exec_command(config, rtdt, cmd);
    let mut ret_status = 4242;
    if let Some(st) = status {
        ret_status = st;
    }
    println!("Exiting job '{}' with status {}", cmd, &ret_status);
    std::process::exit(ret_status);
}

fn do_set_job_status(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    a_job_id: &str,
    a_msg: &str,
) {
    let j = db_get_job(a_job_id);
    if j.is_err() {
        error!("Could not find job {}", a_job_id);
        return;
    }
    let j = j.unwrap();

    {
        use diesel::expression_methods::*;
        use diesel::query_dsl::QueryDsl;
        use diesel::query_dsl::RunQueryDsl;
        use schema::jobs;
        use schema::jobs::dsl::*;

        let some_ndt_now = Some(now_naive_date_time());
        let db = get_db();

        let updated_rows = diesel::update(jobs.filter(job_id.eq(&a_job_id)))
            .set((
                status_message.eq(a_msg.to_string()),
                status_updated_at.eq(some_ndt_now),
            ))
            .execute(db.conn())
            .unwrap();
    }
    regenerate_job_html(config, rtdt, &a_job_id);
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
    let mtime = fs::metadata(fname).ok().map(|x| x.modified().unwrap());
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

fn process_cron_triggers(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    since: &NaiveDateTime,
    now: &NaiveDateTime,
) -> NaiveDateTime {
    // use chrono::Local;
    use chrono::{DateTime, Local, TimeZone};

    let dt_since = Local.from_local_datetime(&since).unwrap();
    let ndt_max_cron = ndt_add_seconds(now.clone(), 3600 * 24); /* within 24h we will surely have a poll */
    let dt_max_cron = Local.from_local_datetime(&ndt_max_cron).unwrap();
    let mut dt_now = Local.from_local_datetime(&now).unwrap();
    let mut dt_next_cron = Local.from_local_datetime(&ndt_max_cron).unwrap();

    for sched in &rtdt.cron_trigger_schedules {
        let mut skip = 0;
        let next_0 = sched.schedule.after(&dt_since).nth(0);
        println!("NEXT {} cron: {:?}", &sched.name, &next_0);
        let next_0 = next_0.unwrap_or(dt_max_cron.clone());
        if (next_0 < dt_now) {
            // run cron command
            debug!("CRON: attempting to run {}", &sched.name);
            if let Some(triggers) = &config.cron_triggers {
                if let Some(ctrig) = triggers.get(&sched.name) {
                    if let s5TriggerAction::command(cmd) = &ctrig.action {
                        let job_id = spawn_command(config, rtdt, &cmd);
                    }
                }
            }
            skip = 1;
        } else {
            debug!(
                "CRON not running {} as {} is in the future",
                &sched.name, &next_0
            );
        }
        for d in sched.schedule.after(&dt_since).skip(skip) {
            if d < dt_now {
                /* in the past, no need to deal with this one */
                continue;
            }
            if d > dt_next_cron {
                /* later than next cron, stop looking */
                break;
            }
            dt_next_cron = d;
        }
    }
    let ndt_next_cron = dt_next_cron.naive_local();
    debug!("CRON: Next cron occurence: {}", &ndt_next_cron);
    return ndt_add_seconds(ndt_next_cron, -1); /* one second earlier to catch the next occurence */
}

fn do_loop(config: &s5ciConfig, rtdt: &s5ciRuntimeData) {
    use std::env;
    use std::fs;
    println!("Starting loop at {}", now_naive_date_time());
    regenerate_all_html(&config, &rtdt);

    let sync_horizon_sec: u32 = config
        .server
        .sync_horizon_sec
        .unwrap_or(config.default_sync_horizon_sec.unwrap_or(86400));

    let mut before: Option<NaiveDateTime> = None;
    let mut after: Option<NaiveDateTime> = Some(NaiveDateTime::from_timestamp(
        (now_naive_date_time().timestamp() - sync_horizon_sec as i64),
        0,
    ));

    let mut cron_timestamp = now_naive_date_time();
    let mut poll_timestamp = now_naive_date_time();
    let config_mtime = get_mtime(&rtdt.config_path);
    let exe_mtime = get_mtime(&rtdt.real_s5ci_exe);

    if let Some(trigger_delay_sec) = config.default_regex_trigger_delay_sec {
        println!("default_regex_trigger_delay_sec = {}, all regex trigger reactions will be delayed by that", trigger_delay_sec)
    }

    loop {
        if let Some(trigger_delay_sec) = config.default_regex_trigger_delay_sec {
            if let Some(after_ts) = after {
                after = Some(ndt_add_seconds(after_ts, -(trigger_delay_sec as i32)));
            }
        }
        if config.autorestart.on_config_change
            && file_changed_since(&rtdt.config_path, config_mtime)
        {
            println!(
                "Config changed, attempt restart at {}...",
                now_naive_date_time()
            );
            restart_ourselves();
        }
        if config.autorestart.on_exe_change && file_changed_since(&rtdt.real_s5ci_exe, exe_mtime)
        {
            println!(
                "Executable changed, attempt restart at {}... ",
                now_naive_date_time()
            );
            restart_ourselves();
        }
        let ndt_now = now_naive_date_time();
        if ndt_now > poll_timestamp {
            // println!("{:?}", ndt);
            let res_res = poll_gerrit_over_ssh(&config, &rtdt, before, after);
            if let Ok(res) = res_res {
                for cs in res.changes {
                    process_change(&config, &rtdt, &cs, before, after);
                }
                before = res.before_when;
                after = res.after_when;
                if let Some(before_time) = before.clone() {
                    if before_time.timestamp()
                        < now_naive_date_time().timestamp() - sync_horizon_sec as i64
                    {
                        eprintln!(
                            "Time {} is beyond the horizon of {} seconds from now, finish sync",
                            &before_time, sync_horizon_sec
                        );
                        before = None;
                    }
                }
            } else {
                eprintln!("Error doing ssh: {:?}", &res_res);
            }
            let mut wait_time_ms = config.server.poll_wait_ms.unwrap_or(300000);
            if before.is_some() {
                wait_time_ms = config.server.syncing_poll_wait_ms.unwrap_or(wait_time_ms);
            }
            poll_timestamp = ndt_add_ms(poll_timestamp, wait_time_ms as i64 - 10);
        } else {
            debug!(
                "Poll timestamp {} is in the future, not polling",
                &poll_timestamp
            );
        }

        if ndt_now > ndt_add_seconds(cron_timestamp, 1) {
            cron_timestamp = process_cron_triggers(config, rtdt, &cron_timestamp, &ndt_now);
        } else {
            debug!(
                "Cron timestamp {} is in the future, no cron processing this time",
                &cron_timestamp
            );
        }

        let mut next_timestamp = ndt_add_seconds(cron_timestamp, 2);
        if poll_timestamp < next_timestamp {
            next_timestamp = poll_timestamp;
        }

        let wait_time_ms = next_timestamp
            .signed_duration_since(now_naive_date_time())
            .num_milliseconds()
            + 1;

        collect_zombies();
        // ps();
        // eprintln!("Sleeping for {} msec ({})", wait_time_ms, wait_name);
        debug!("Sleeping for {} ms", wait_time_ms);
        if wait_time_ms > 0 {
            s5ci::thread_sleep_ms(wait_time_ms as u64);
        }
    }
}

fn main() {
    env_logger::init();
    let (config, rtdt) = get_configs();
    use s5ciAction;
    maybe_compile_template(&config, "job_page").unwrap();
    maybe_compile_template(&config, "root_job_page").unwrap();
    maybe_compile_template(&config, "active_job_page").unwrap();
    maybe_compile_template(&config, "group_job_page").unwrap();

    match &rtdt.action {
        s5ciAction::Loop => do_loop(&config, &rtdt),
        s5ciAction::ListJobs => do_list_jobs(&config, &rtdt),
        s5ciAction::KillJob(job_id) => do_kill_job(&config, &rtdt, &job_id, "S5CI CLI"),
        s5ciAction::RunJob(cmd) => do_run_job(&config, &rtdt, &cmd),
        s5ciAction::SetStatus(job_id, msg) => do_set_job_status(&config, &rtdt, &job_id, &msg),
        s5ciAction::GerritCommand(cmd) => do_gerrit_command(&config, &rtdt, &cmd),
        s5ciAction::MakeReview(maybe_vote, msg) => do_review(&config, &rtdt, maybe_vote, &msg),
    }
}
