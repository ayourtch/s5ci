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

mod autorestart;
mod comment_triggers;
mod database;
mod gerrit_interact;
mod gerrit_types;
mod html_gen;
mod job_mgmt;
mod run_ssh_command;
mod runtime_data;
mod s5ci_config;
mod unix_process;

use crate::autorestart::*;
use crate::comment_triggers::*;
use crate::database::*;
use crate::gerrit_interact::*;
use crate::gerrit_types::*;
use crate::html_gen::*;
use crate::job_mgmt::*;
use crate::run_ssh_command::*;
use crate::runtime_data::*;
use crate::s5ci_config::*;
use crate::unix_process::*;

use s5ci::*;

fn do_gerrit_command(config: &s5ciConfig, rtdt: &s5ciRuntimeData, cmd: &str) {
    run_ssh_command(config, cmd);
}

fn do_review(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    maybe_vote: &Option<GerritVoteAction>,
    msg: &str,
) {
    gerrit_add_review_comment(config, rtdt, maybe_vote, msg)
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
fn do_kill_job(config: &s5ciConfig, rtdt: &s5ciRuntimeData, jobid: &str, terminator: &str) {
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
    let group_name = job_group_name_from_cmd(cmd);
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

fn do_set_job_status(config: &s5ciConfig, rtdt: &s5ciRuntimeData, a_job_id: &str, a_msg: &str) {
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

    if let Some(trigger_delay_sec) = config.default_regex_trigger_delay_sec {
        println!("default_regex_trigger_delay_sec = {}, all regex trigger reactions will be delayed by that", trigger_delay_sec)
    }

    let ar_state = autorestart_init(config, rtdt);

    loop {
        autorestart_check(config, rtdt, &ar_state);
        if let Some(trigger_delay_sec) = config.default_regex_trigger_delay_sec {
            if let Some(after_ts) = after {
                after = Some(ndt_add_seconds(after_ts, -(trigger_delay_sec as i32)));
            }
        }

        let ndt_now = now_naive_date_time();
        if ndt_now > poll_timestamp {
            // println!("{:?}", ndt);
            let res_res = poll_gerrit_over_ssh(&config, &rtdt, before, after);
            if let Ok(res) = res_res {
                for cs in res.changes {
                    process_gerrit_change(&config, &rtdt, &cs, before, after);
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
