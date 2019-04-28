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
extern crate serde_yaml;
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

extern crate mustache;
extern crate serde;
extern crate serde_json;

use chrono::NaiveDateTime;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritQueryError {
    r#type: String,
    message: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritQueryStats {
    r#type: String,
    rowCount: u32,
    runTimeMilliseconds: u32,
    moreChanges: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritOwner {
    name: Option<String>,
    email: Option<String>,
    username: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritComment {
    timestamp: i64,
    reviewer: GerritOwner,
    message: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritApproval {
    r#type: String,
    description: String,
    value: String,
    grantedOn: i64,
    by: GerritOwner,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritFileChange {
    file: String,
    r#type: String,
    insertions: i32,
    deletions: i32,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritPatchSet {
    number: u32,
    revision: String,
    parents: Vec<String>,
    r#ref: String,
    uploader: GerritOwner,
    createdOn: i64,
    author: GerritOwner,
    isDraft: bool,
    kind: String,
    approvals: Option<Vec<GerritApproval>>,
    files: Option<Vec<GerritFileChange>>,
    sizeInsertions: Option<i32>,
    sizeDeletions: Option<i32>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritDependentPatchSet {
    id: String,
    number: i32,
    revision: String,
    r#ref: String,
    isCurrentPatchSet: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritLabel {
    label: String,
    status: String,
    by: Option<GerritOwner>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritSubmitRecords {
    status: String,
    labels: Vec<GerritLabel>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GerritChangeSet {
    project: Option<String>,
    branch: Option<String>,
    id: Option<String>,
    number: Option<u32>,
    subject: Option<String>,
    owner: Option<GerritOwner>,
    url: Option<String>,
    commitMessage: Option<String>,
    createdOn: Option<i64>,
    lastUpdated: Option<i64>,
    open: Option<bool>,
    status: Option<String>,
    comments: Option<Vec<GerritComment>>,
    patchSets: Option<Vec<GerritPatchSet>>,
    submitRecords: Option<Vec<GerritSubmitRecords>>,
    allReviewers: Option<Vec<GerritOwner>>,
}

use s5ci::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
enum BeforeAfter {
    Before(NaiveDateTime),
    After(NaiveDateTime),
    Any,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum GerritVoteAction {
    success,
    failure,
    clear,
}

impl std::str::FromStr for GerritVoteAction {
    type Err = ();
    fn from_str(s: &str) -> Result<GerritVoteAction, ()> {
        match s {
            "success" => Ok(GerritVoteAction::success),
            "failure" => Ok(GerritVoteAction::failure),
            "clear" => Ok(GerritVoteAction::clear),
            _ => Err(()),
        }
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LucySshAuthPubkeyFile {
    username: String,
    pubkey: Option<String>,
    privatekey: String,
    passphrase: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LucySshAuthPassword {
    username: String,
    password: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LucySshAuthAgent {
    username: String,
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug, Clone)]
enum LucySshAuth {
    auth_pubkey_file(LucySshAuthPubkeyFile),
    auth_password(LucySshAuthPassword),
    auth_agent(LucySshAuthAgent),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LucyCiPollGerrit {
    address: std::net::IpAddr,
    port: u16,
    auth: Option<LucySshAuth>,
    poll_wait_ms: Option<u64>,
    syncing_poll_wait_ms: Option<u64>,
    sync_horizon_sec: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LucyGerritQuery {
    filter: String,
    options: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum LucyTriggerAction {
    event(String),
    command(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LucyGerritTrigger {
    regex: String,
    action: LucyTriggerAction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LucyCiJobs {
    rootdir: String,
    root_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LucyAutorestartConfig {
    on_config_change: bool,
    on_exe_change: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LucyCiConfig {
    default_auth: LucySshAuth,
    server: LucyCiPollGerrit,
    default_query: LucyGerritQuery,
    default_batch_command: Option<String>,
    default_sync_horizon_sec: Option<u32>,
    triggers: Option<HashMap<String, LucyGerritTrigger>>,
    patchset_extract_regex: String,
    hostname: String,
    autorestart: LucyAutorestartConfig,
    db_url: String,
    jobs: LucyCiJobs,
}

impl LucyCiPollGerrit {
    fn get_server_address_port(self: &Self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LucySshResult {
    before_when: Option<NaiveDateTime>,
    after_when: Option<NaiveDateTime>,
    output: String,
    changes: Vec<GerritChangeSet>,
    stats: Option<GerritQueryStats>,
}

#[derive(Debug)]
enum LucySshError {
    Ssh2Error(ssh2::Error),
    IoError(io::Error),
    SerdeJsonError(serde_json::Error),
    RemoteError(i32, String),
    QueryBackendError(String),
    MustacheError(mustache::Error),
}

impl From<ssh2::Error> for LucySshError {
    fn from(error: ssh2::Error) -> Self {
        LucySshError::Ssh2Error(error)
    }
}

impl From<io::Error> for LucySshError {
    fn from(error: io::Error) -> Self {
        LucySshError::IoError(error)
    }
}

impl From<serde_json::Error> for LucySshError {
    fn from(error: serde_json::Error) -> Self {
        LucySshError::SerdeJsonError(error)
    }
}
impl From<mustache::Error> for LucySshError {
    fn from(error: mustache::Error) -> Self {
        LucySshError::MustacheError(error)
    }
}

fn run_ssh_command(config: &LucyCiConfig, cmd: &str) -> Result<String, LucySshError> {
    // Connect to the local SSH server
    let tcp = TcpStream::connect(config.server.get_server_address_port())?; // .unwrap();
    let ssh_auth = &config.default_auth;
    // let tcp = TcpStream::connect("gerrit.fd.io:29418").unwrap();
    let mut sess = Session::new().unwrap();
    sess.handshake(&tcp)?;
    match ssh_auth {
        LucySshAuth::auth_pubkey_file(pk) => {
            sess.userauth_pubkey_file(
                &pk.username,
                None,
                Path::new(&pk.privatekey),
                pk.passphrase.as_ref().map_or(None, |x| Some(&**x)),
            )?;
        }
        LucySshAuth::auth_password(pw) => {
            sess.userauth_password(&pw.username, &pw.password)?;
        }
        LucySshAuth::auth_agent(agent) => {
            sess.userauth_agent(&agent.username)?;
        }
    }
    sess.set_blocking(true);
    // Safety timeout
    sess.set_timeout(120000);

    let mut channel = sess.channel_session().unwrap();

    debug!("SSH: running command '{}'", cmd);
    channel.exec(cmd)?;

    let mut stderr = channel.stderr();
    let mut stderr_buffer = String::new();
    stderr.read_to_string(&mut stderr_buffer)?;
    debug!("SSH: ERR: {}", &stderr_buffer);

    let mut s = String::new();
    debug!("SSH: collect output");
    while !channel.eof() {
        let mut s0 = String::new();
        channel.read_to_string(&mut s0)?;
        s.push_str(&s0);
    }
    debug!("SSH: end collecting");

    let exit_status = channel.exit_status().unwrap();
    if exit_status != 0 {
        Err(LucySshError::RemoteError(exit_status, stderr_buffer))
    } else {
        Ok(s)
    }
}

fn get_job_url(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig, job_id: &str) -> String {
    format!("{}/{}/", config.jobs.root_url, job_id)
}
fn get_job_name(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig, job_id: &str) -> String {
    let re = Regex::new(r"[^A-Za-z0-9_]").unwrap();

    let job_name = re.replace_all(&format!("{}", job_id), "_").to_string();
    job_name
}

fn gerrit_query_changes(
    config: &LucyCiConfig,
    before_when: Option<NaiveDateTime>,
    after_when: Option<NaiveDateTime>,
) -> Result<String, LucySshError> {
    let date_str = if before_when.is_some() {
        if after_when.is_some() {
            format!(
                "(before: \\\"{}\\\" OR after:\\\"{}\\\")",
                before_when.clone().unwrap(),
                after_when.clone().unwrap()
            )
        } else {
            format!("before:\\\"{}\\\"", before_when.clone().unwrap())
        }
    } else {
        if after_when.is_some() {
            format!("after:\\\"{}\\\"", after_when.clone().unwrap())
        } else {
            format!("")
        }
    };

    debug!("DATE query: {}", &date_str);
    // let cmd = format!("gerrit query status:open project:vpp limit:4 {} --format JSON --all-approvals --all-reviewers --comments --commit-message --dependencies --files --patch-sets --submit-records", &date_str);
    // let cmd = format!("gerrit query status:open project:vpp limit:4 {} --format JSON --all-approvals --all-reviewers --comments --commit-message --dependencies --patch-sets --submit-records", &date_str);
    let q = &config.default_query;
    let cmd = format!(
        "gerrit query {} {} --format JSON {}",
        &q.filter, &date_str, &q.options
    );
    run_ssh_command(config, &cmd)
}

fn do_ssh(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
    before_when: Option<NaiveDateTime>,
    after_when: Option<NaiveDateTime>,
) -> Result<LucySshResult, LucySshError> {
    debug!(
        "Retrieving changesets for time before {:?} or after {:?}",
        &before_when, &after_when
    );
    let mut ndt = now_naive_date_time();
    let ret_after_when = Some(ndt);
    let mut ret_before_when: Option<NaiveDateTime> = None;
    let mut ret_stats: Option<GerritQueryStats> = None;

    let mut last_timestamp: i64 = ndt.timestamp();
    let mut more_changes = false;
    let s = gerrit_query_changes(config, before_when, after_when)?;
    let mut ret_changes: Vec<GerritChangeSet> = vec![];
    if &s != "" {
        for line in s.lines() {
            // eprintln!("{}", &line);
            let backend_res: Result<GerritQueryError, serde_json::Error> =
                serde_json::from_str(&format!("{}", &line));
            if let Ok(error) = backend_res {
                if &error.r#type == "error" {
                    return Err(LucySshError::QueryBackendError(error.message));
                }
            }
            let backend_res: Result<GerritQueryStats, serde_json::Error> =
                serde_json::from_str(&format!("{}", &line));
            if backend_res.is_err() {
                let cs: GerritChangeSet = serde_json::from_str(&format!("{}", &line))?;
                // println!("Backend res: {:?}", &cs);
                if let Some(ts) = cs.lastUpdated {
                    debug!(
                        "Change: {} number {}",
                        &cs.id.clone().unwrap_or("".into()),
                        &cs.number.unwrap_or(0)
                    );
                    if ts < last_timestamp {
                        last_timestamp = ts;
                    }
                    ret_changes.push(cs);
                }
            } else {
                debug!("STATS Backend res: {:?}", backend_res);
                if let Ok(stats) = backend_res {
                    ret_stats = Some(stats.clone());
                    more_changes = stats.moreChanges;
                    if stats.rowCount > 0 {
                        use s5ci::*;
                        // spawn_simple_command("scripts", "git-mirror");
                    }
                }
            }
        }
    }
    if more_changes {
        ndt = NaiveDateTime::from_timestamp(last_timestamp, 0);
        ret_before_when = Some(ndt);
    }
    // println!("{}", channel.exit_status().unwrap());
    // ret_when
    Ok(LucySshResult {
        before_when: ret_before_when,
        after_when: ret_after_when,
        output: s,
        changes: ret_changes,
        stats: ret_stats,
    })
}

fn run_batch_command(
    config: &LucyCiConfig,
    before: &Option<NaiveDateTime>,
    after: &Option<NaiveDateTime>,
    stats: &GerritQueryStats,
    output: &str,
) -> bool {
    let mut abort_sync = false;
    if stats.rowCount > 0 {
        if let Some(cmd) = config.default_batch_command.clone() {
            use std::io::{BufRead, BufReader, BufWriter, Write};
            use std::process::{Command, Stdio};

            let mut p = Command::new("/bin/sh")
                .arg("-c")
                .arg(&format!("{}", &cmd,))
                .stdin(Stdio::piped())
                .env(
                    "BEFORE_TIMESTAMP",
                    &format!("{}", before.map_or(0, |x| x.timestamp())),
                )
                .env(
                    "AFTER_TIMESTAMP",
                    &format!("{}", after.map_or(0, |x| x.timestamp())),
                )
                .spawn()
                .unwrap();
            write!(p.stdin.as_mut().unwrap(), "{}", output);
            let exit_code = p.wait();
            if let Ok(status) = exit_code {
                match status.code() {
                    Some(code) => {
                        println!("Exited with status code: {}", code);
                        if code == 42 {
                            abort_sync = true;
                        }
                    }
                    None => println!("Process terminated by signal"),
                }
            } else {
                error!("Command finished with error, code: {:?}", exit_code);
            }
        } else {
            // println!("{}", output);
        }
    }
    abort_sync
}

use libc::pid_t;
fn mypid() -> pid_t {
    use libc::getpid;
    let pid = unsafe { getpid() };
    pid
}

fn setsid() -> pid_t {
    use libc::setsid;
    let pid = unsafe { setsid() };
    pid
}

pub fn collect_zombies() -> i32 {
    mod c {
        use libc;
        extern "C" {
            pub fn waitpid(
                pid: libc::pid_t,
                status: *mut libc::c_int,
                flags: libc::c_int,
            ) -> libc::c_int;
        }
    }
    unsafe {
        let pid: i32 = -1;
        let flags: i32 = 1; // wnohang
        let mut status: libc::c_int = 0;
        let mut count = 0;
        loop {
            let ret_pid = c::waitpid(
                pid as libc::pid_t,
                &mut status as *mut libc::c_int,
                flags as libc::c_int,
            );
            if ret_pid <= 0 {
                break;
            }
            {
                eprintln!("Collected exit status from pid {}: {:?}", ret_pid, status);
                count += 1;
            }
        }
        count
    }
}

#[derive(Debug, Clone)]
struct CommentTriggerRegex {
    r: Regex,
    name: String,
}

#[derive(Debug, Clone)]
enum LucyCiAction {
    Loop,
    ListJobs,
    SetStatus(String, String),
    RunJob(String),
    GerritCommand(String),
    MakeReview(Option<GerritVoteAction>, String),
}

#[derive(Debug, Clone)]
struct LucyCiCompiledConfig {
    config_path: String,
    patchset_extract_regex: Regex,
    trigger_regexes: Vec<CommentTriggerRegex>,
    trigger_command_templates: HashMap<String, mustache::Template>,
    action: LucyCiAction,
    changeset_id: Option<u32>,
    patchset_id: Option<u32>,
}

fn get_trigger_regexes(config: &LucyCiConfig) -> Vec<CommentTriggerRegex> {
    let mut out = vec![];
    if let Some(triggers) = &config.triggers {
        for (name, trig) in triggers {
            let r = Regex::new(&trig.regex).unwrap();
            out.push(CommentTriggerRegex {
                r: r,
                name: name.clone(),
            });
        }
    }

    out
}

fn get_trigger_command_templates(config: &LucyCiConfig) -> HashMap<String, mustache::Template> {
    let mut out = HashMap::new();
    if let Some(triggers) = &config.triggers {
        for (name, trig) in triggers {
            if let LucyTriggerAction::command(cmd) = &trig.action {
                let template = mustache::compile_str(cmd).unwrap();
                out.insert(name.clone(), template);
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
struct CommentTrigger {
    comment_index: u32,
    trigger_name: String,
    patchset_id: u32,
    captures: HashMap<String, String>,
}

fn get_comment_triggers(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
    comments_vec: &Vec<GerritComment>,
    startline_ts: i64,
) -> Vec<CommentTrigger> {
    let trigger_regexes = &cconfig.trigger_regexes;
    let mut out = vec![];

    for (i, comment) in comments_vec.iter().enumerate() {
        debug!("Comment: {}: {:#?}", i, &comment);
        if comment.timestamp > startline_ts {
            let mut patchset_str = format!("");
            /*
            eprintln!(
                "    comment at {} by {}: {}",
                comment.timestamp,
                comment.reviewer.email.clone().unwrap_or("unknown".into()),
                comment.message
            );
            */
            if let Some(rem) = cconfig.patchset_extract_regex.captures(&comment.message) {
                if let Some(ps) = rem.name("patchset") {
                    patchset_str = format!("{}", ps.as_str());
                }
            }
            for tr in trigger_regexes {
                let mut captures: HashMap<String, String> = HashMap::new();
                if tr.r.is_match(&comment.message) {
                    captures.insert("patchset".into(), format!("{}", &patchset_str));
                    // eprintln!("        Comment matched regex {}", &tr.name);
                    // try to extract the patchset from the start of comment
                    for m in tr.r.captures(&comment.message) {
                        for maybe_name in tr.r.capture_names() {
                            if let Some(name) = maybe_name {
                                if let Some(val) = m.name(&name) {
                                    captures.insert(name.to_string(), val.as_str().to_string());
                                }
                            }
                        }
                    }

                    if !captures["patchset"].parse::<u32>().is_ok() {
                        error!(
                            "unparseable patchset in {:#?}: {:#?}",
                            &comment, &patchset_str
                        );
                    }
                    let patchset_id = captures["patchset"].parse::<u32>().unwrap();
                    let trigger_name = format!("{}", &tr.name);
                    let trig = CommentTrigger {
                        comment_index: i as u32,
                        trigger_name: trigger_name,
                        captures: captures,
                        patchset_id: patchset_id,
                    };
                    out.push(trig);
                }
            }
        }
    }

    out
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

fn get_min_job_counter(config: &LucyCiConfig, jobname: &str) -> i32 {
    use std::fs;
    let jobpath = format!("{}/{}", &config.jobs.rootdir, jobname);
    let path = Path::new(&jobpath);
    if !path.is_dir() {
        fs::create_dir(&jobpath).unwrap();
    }
    let file_count = fs::read_dir(path).unwrap().count();
    file_count as i32
}

fn get_next_job_number(config: &LucyCiConfig, jobname: &str) -> i32 {
    use std::fs;
    let a_min = get_min_job_counter(config, jobname);
    let job_number = db_get_next_counter_value_with_min(jobname, a_min).unwrap();

    let jobpath = format!("{}/{}", &config.jobs.rootdir, jobname);
    let path = Path::new(&jobpath);
    if !path.is_dir() {
        fs::create_dir(&jobpath).unwrap();
    }
    let new_path = format!("{}/{}", jobpath, job_number);
    println!("CREATING DIR {}", &new_path);
    fs::create_dir(&new_path).unwrap();
    job_number
}

fn prepare_child_command<'a>(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
    child0: &'a mut std::process::Command,
    cmd: &str,
    suffix: &str,
) -> (String, i32, &'a mut std::process::Command) {
    use regex::Regex;
    use std::env;
    use std::process::Command;
    use std::process::Stdio;
    let args: Vec<String> = env::args().collect();

    let re = Regex::new(r"[^A-Za-z0-9_]").unwrap();

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
        .env("S5CI_EXE", &format!("{}", args[0]))
        .env("S5CI_JOB_ID", &job_id)
        .env("S5CI_JOB_NAME", &get_job_name(config, cconfig, &job_id))
        .env("S5CI_JOB_URL", &get_job_url(config, cconfig, &job_id))
        .env("S5CI_CONFIG", &cconfig.config_path);

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

fn spawn_command(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig, cmd: &str) {
    use std::env;
    use std::process::Command;
    let args: Vec<String> = env::args().collect();
    let env_changeset_id = format!("{}", cconfig.changeset_id.unwrap());
    let env_patchset_id = format!("{}", cconfig.patchset_id.unwrap());
    let mut child0 = Command::new(&args[0]);
    let mut child = child0
        .arg("run-job")
        .arg("-c")
        .arg(format!("{}", cmd))
        .env("S5CI_CONFIG", &cconfig.config_path)
        .env("S5CI_GERRIT_CHANGESET_ID", &env_changeset_id)
        .env("S5CI_GERRIT_PATCHSET_ID", &env_patchset_id);
    println!("Spawning {:#?}", child);
    let res = child.spawn().expect("failed to execute child");
    println!("Spawned pid {}", res.id());
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
    let max_retry_count = 5;
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

fn maybe_compile_template(config: &LucyCiConfig, name: &str) -> Result<Template, mustache::Error> {
    let res = mustache::compile_path(format!("./templates/{}.mustache", name));
    if res.is_err() {
        error!("Could not compile template {}: {:#?}", name, &res);
    }
    res
}

fn fill_and_write_template(
    template: Template,
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

fn regenerate_group_html(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig, group_name: &str) {
    let template = maybe_compile_template(config, "group_job_page").unwrap();
    let mut data = MapBuilder::new();
    let jobs = db_get_jobs_by_group_name(group_name);
    data = data.insert("job_group_name", &group_name).unwrap();
    data = data.insert("child_jobs", &jobs).unwrap();
    let fname = format!("{}/{}/index.html", &config.jobs.rootdir, group_name);
    fill_and_write_template(template, data, &fname).unwrap();
}

fn regenerate_root_html(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig) {
    let template = maybe_compile_template(config, "root_job_page").unwrap();
    let mut data = MapBuilder::new();
    let rjs = db_get_root_jobs();
    data = data.insert("child_jobs", &rjs).unwrap();
    let fname = format!("{}/index.html", &config.jobs.rootdir);
    fill_and_write_template(template, data, &fname).unwrap();
}

fn regenerate_active_html(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig) {
    let template = maybe_compile_template(config, "active_job_page").unwrap();
    let mut data = MapBuilder::new();
    let rjs = db_get_active_jobs();
    data = data.insert("child_jobs", &rjs).unwrap();
    let fname = format!("{}/active.html", &config.jobs.rootdir);
    fill_and_write_template(template, data, &fname).unwrap();
}

fn regenerate_html(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
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
    fill_and_write_template(template, data, &fname).unwrap();

    if update_children {
        for cj in cjs {
            regenerate_html(config, cconfig, &cj.job_id, false, false, groups);
        }
    }

    if update_parent {
        if let Some(pjob_id) = &j.parent_job_id {
            regenerate_html(config, cconfig, pjob_id, false, false, groups);
        } else {
            regenerate_root_html(config, cconfig);
        }
    }
    groups.insert(
        j.job_group_name.clone(),
        1 + groups.get(&j.job_group_name).unwrap_or(&0),
    );
}

fn regenerate_job_html(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig, job_id: &str) {
    let mut groups = HashMap::new();
    regenerate_html(config, cconfig, job_id, true, true, &mut groups);
    for (group_name, count) in groups {
        println!("Regenerating group {} with {} jobs", &group_name, count);
        regenerate_group_html(config, cconfig, &group_name);
    }
    regenerate_active_html(config, cconfig);
}

fn starting_job(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig, job_id: &str) {
    regenerate_job_html(config, cconfig, job_id);
}

fn finished_job(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig, job_id: &str) {
    regenerate_job_html(config, cconfig, job_id);
}

fn regenerate_all_html(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig) {
    let jobs = db_get_all_jobs();
    let mut groups = HashMap::new();
    for j in jobs {
        println!("Regenerate HTML for {}", &j.job_id);
        regenerate_html(config, cconfig, &j.job_id, false, false, &mut groups);
    }
    for (group_name, count) in groups {
        println!("Regenerating group {} with {} jobs", &group_name, count);
        regenerate_group_html(config, cconfig, &group_name);
    }
    regenerate_root_html(config, cconfig);
    regenerate_active_html(config, cconfig);
}

fn exec_command(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
    cmd: &str,
) -> (String, Option<i32>) {
    use std::env;
    use std::process::Command;
    use uuid::Uuid;

    let env_changeset_id = cconfig.changeset_id.unwrap() as i32;
    let env_patchset_id = cconfig.patchset_id.unwrap() as i32;
    let mut child0 = Command::new("/bin/sh");
    let mut child = child0.arg("-c");
    let (a_job_group_name, a_instance_id, mut child) =
        prepare_child_command(config, cconfig, child, cmd, "");
    let a_full_job_id = format!("{}/{}", &a_job_group_name, a_instance_id);

    let my_uuid = Uuid::new_v4().to_simple().to_string();
    /* in our environment the job ID, if set, is set by parent */
    let env_pj_id = env::var("S5CI_JOB_ID").ok();

    let mut new_job = models::job {
        record_uuid: my_uuid.clone(),
        job_group_name: a_job_group_name,
        instance_id: a_instance_id,
        job_id: a_full_job_id.clone(),
        parent_job_id: env_pj_id.clone(),
        changeset_id: env_changeset_id,
        patchset_id: env_patchset_id,
        command: format!("{}", cmd),
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
    starting_job(config, cconfig, &a_full_job_id);
    let status = child.status().expect("failed to execute process");
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
    finished_job(config, cconfig, &a_full_job_id);
    return (a_full_job_id, status.code());
}

fn process_change(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
    cs: &GerritChangeSet,
    before_when: Option<NaiveDateTime>,
    after_when: Option<NaiveDateTime>,
) {
    let mut triggers: Vec<CommentTrigger> = vec![];

    // eprintln!("Processing change: {:#?}", cs);
    if let Some(startline) = after_when {
        let startline_ts = startline.timestamp() - 1;
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
            }

            // eprintln!("Patchset map: {:#?}", &psmap);
        }
        if let Some(comments_vec) = &cs.comments {
            let all_triggers = get_comment_triggers(config, cconfig, comments_vec, startline_ts);
            let mut final_triggers = all_triggers.clone();
            if let Some(cfgt) = &config.triggers {
                final_triggers.retain(|x| {
                    let ctrig = &cfgt[&x.trigger_name];
                    if let LucyTriggerAction::command(cmd) = &ctrig.action {
                        true
                    } else {
                        false
                    }
                });
            }
            // eprintln!("all triggers: {:#?}", &final_triggers);
            eprintln!("final triggers: {:#?}", &final_triggers);
            for trig in &final_triggers {
                let template = cconfig
                    .trigger_command_templates
                    .get(&trig.trigger_name)
                    .unwrap();
                let patchset = psmap.get(&format!("{}", trig.patchset_id)).unwrap();
                let data = mustache::MapBuilder::new()
                    .insert("patchset", &patchset)
                    .unwrap()
                    .insert("regex", &trig.captures)
                    .unwrap()
                    .build();
                let mut bytes = vec![];

                template.render_data(&mut bytes, &data).unwrap();
                let expanded_command = String::from_utf8_lossy(&bytes);
                let change_id = cs.number.unwrap();
                let mut cconfig2 = cconfig.clone();
                cconfig2.changeset_id = Some(change_id);
                cconfig2.patchset_id = Some(trig.patchset_id);
                let job_id = spawn_command(config, &cconfig2, &expanded_command);
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

fn get_configs() -> (LucyCiConfig, LucyCiCompiledConfig) {
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
        .subcommand(SubCommand::with_name("list-jobs").about("list jobs"))
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

    let yaml_fname = &matches.value_of("config").unwrap();
    let s = fs::read_to_string(yaml_fname).unwrap();
    let config: LucyCiConfig = serde_yaml::from_str(&s).unwrap();
    debug!("Config: {:#?}", &config);
    set_db_url(&config.db_url);
    let trigger_regexes = get_trigger_regexes(&config);
    let patchset_regex = Regex::new(&config.patchset_extract_regex).unwrap();
    let trigger_command_templates = get_trigger_command_templates(&config);
    let mut changeset_id: Option<u32> = None;
    let mut patchset_id: Option<u32> = None;

    let mut action = LucyCiAction::Loop;

    if let Some(matches) = matches.subcommand_matches("gerrit-command") {
        let cmd = matches.value_of("command").unwrap().to_string();
        action = LucyCiAction::GerritCommand(cmd);
    }
    if let Some(matches) = matches.subcommand_matches("run-job") {
        let cmd = matches.value_of("command").unwrap().to_string();
        action = LucyCiAction::RunJob(cmd);
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
    if let Some(matches) = matches.subcommand_matches("list-jobs") {
        action = LucyCiAction::ListJobs;
    }
    if let Some(matches) = matches.subcommand_matches("set-status") {
        let msg = matches.value_of("message").unwrap().to_string();
        let job_id = matches.value_of("job-id").unwrap().to_string();
        action = LucyCiAction::SetStatus(job_id, msg);
    }
    if let Some(matches) = matches.subcommand_matches("review") {
        let msg = matches.value_of("message").unwrap().to_string();

        let vote_value = if matches.value_of("vote").is_some() {
            let val = value_t!(matches, "vote", GerritVoteAction).unwrap();
            Some(val)
        } else {
            None
        };
        action = LucyCiAction::MakeReview(vote_value, msg);
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

    let cconfig = LucyCiCompiledConfig {
        config_path: yaml_fname.to_string(),
        patchset_extract_regex: patchset_regex,
        trigger_regexes: trigger_regexes,
        trigger_command_templates: trigger_command_templates,
        action: action,
        changeset_id: changeset_id,
        patchset_id: patchset_id,
    };
    debug!("C-Config: {:#?}", &cconfig);
    (config, cconfig)
}

fn do_gerrit_command(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig, cmd: &str) {
    run_ssh_command(config, cmd);
}

fn do_review(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
    maybe_vote: &Option<GerritVoteAction>,
    msg: &str,
) {
    let vote = if let Some(act) = maybe_vote {
        match act {
            GerritVoteAction::success => format!(" --code-review +1"),
            GerritVoteAction::failure => format!(" --code-review -1"),
            GerritVoteAction::clear => format!(" --code-review 0"),
        }
    } else {
        format!("")
    };
    let cmd = format!(
        "gerrit review {},{} {} --message \"{}\"",
        cconfig.changeset_id.unwrap(),
        cconfig.patchset_id.unwrap(),
        vote,
        msg
    );
    run_ssh_command(config, &cmd);
}

fn do_list_jobs(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig) {
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
fn do_run_job(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig, cmd: &str) {
    use signal_hook::{iterator::Signals, SIGABRT, SIGHUP, SIGINT, SIGPIPE, SIGQUIT};
    use std::{error::Error, thread};

    let signals = Signals::new(&[SIGINT, SIGPIPE, SIGHUP, SIGQUIT, SIGABRT]).unwrap();

    thread::spawn(move || {
        for sig in signals.forever() {
            println!("Received signal {:?}", sig);
        }
    });
    println!("Requested to run job '{}'", cmd);
    let (job_id, status) = exec_command(config, cconfig, cmd);
    let mut ret_status = 4242;
    if let Some(st) = status {
        ret_status = st;
    }
    println!("Exiting job '{}' with status {}", cmd, &ret_status);
    std::process::exit(ret_status);
}

fn do_set_job_status(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
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
    regenerate_job_html(config, cconfig, &a_job_id);
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

fn do_loop(config: &LucyCiConfig, cconfig: &LucyCiCompiledConfig) {
    use std::env;
    use std::fs;
    let argv_real: Vec<String> = env::args().collect();
    println!("Starting loop at {}", now_naive_date_time());
    regenerate_all_html(&config, &cconfig);

    let sync_horizon_sec: u32 = config
        .server
        .sync_horizon_sec
        .unwrap_or(config.default_sync_horizon_sec.unwrap_or(86400));

    let mut before: Option<NaiveDateTime> = None;
    let mut after: Option<NaiveDateTime> = Some(NaiveDateTime::from_timestamp(
        (now_naive_date_time().timestamp() - sync_horizon_sec as i64),
        0,
    ));

    let config_mtime = get_mtime(&cconfig.config_path);
    let exe_mtime = get_mtime(&argv_real[0]);

    loop {
        if config.autorestart.on_config_change
            && file_changed_since(&cconfig.config_path, config_mtime)
        {
            println!(
                "Config changed, attempt restart at {}...",
                now_naive_date_time()
            );
            restart_ourselves();
        }
        if config.autorestart.on_exe_change && file_changed_since(&argv_real[0], exe_mtime) {
            println!(
                "Executable changed, attempt restart at {}... ",
                now_naive_date_time()
            );
            restart_ourselves();
        }

        // println!("{:?}", ndt);
        let res_res = do_ssh(&config, &cconfig, before, after);
        let mut abort_sync = false;
        if let Ok(res) = res_res {
            if let Some(stats) = res.stats {
                abort_sync = run_batch_command(&config, &before, &after, &stats, &res.output);
            }
            for cs in res.changes {
                process_change(&config, &cconfig, &cs, before, after);
            }
            before = res.before_when;
            after = res.after_when;
            if abort_sync {
                before = None;
                eprintln!("process terminated with status 42, aborting the back-sync");
            }
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
        let mut wait_name = "poll_wait_ms";
        if before.is_some() {
            wait_time_ms = config.server.syncing_poll_wait_ms.unwrap_or(wait_time_ms);
            wait_name = "syncing_poll_wait_ms";
        }

        collect_zombies();
        // ps();
        // eprintln!("Sleeping for {} msec ({})", wait_time_ms, wait_name);
        s5ci::thread_sleep_ms(wait_time_ms);
    }
}

fn main() {
    env_logger::init();
    let (config, cconfig) = get_configs();
    use LucyCiAction;
    maybe_compile_template(&config, "job_page").unwrap();
    maybe_compile_template(&config, "root_job_page").unwrap();
    maybe_compile_template(&config, "active_job_page").unwrap();
    maybe_compile_template(&config, "group_job_page").unwrap();

    match &cconfig.action {
        LucyCiAction::Loop => do_loop(&config, &cconfig),
        LucyCiAction::ListJobs => do_list_jobs(&config, &cconfig),
        LucyCiAction::RunJob(cmd) => do_run_job(&config, &cconfig, &cmd),
        LucyCiAction::SetStatus(job_id, msg) => do_set_job_status(&config, &cconfig, &job_id, &msg),
        LucyCiAction::GerritCommand(cmd) => do_gerrit_command(&config, &cconfig, &cmd),
        LucyCiAction::MakeReview(maybe_vote, msg) => do_review(&config, &cconfig, maybe_vote, &msg),
    }
}
