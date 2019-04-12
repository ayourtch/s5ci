use ssh2::Session;
use std::collections::HashMap;
use std::io;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;

extern crate libc;
extern crate psutil;
extern crate regex;
extern crate serde_yaml;
extern crate yaml_rust;

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
struct LucyCiConfig {
    default_auth: LucySshAuth,
    server: LucyCiPollGerrit,
    default_query: LucyGerritQuery,
    default_batch_command: Option<String>,
    default_sync_horizon_sec: Option<u32>,
    triggers: Option<HashMap<String, LucyGerritTrigger>>,
    patchset_extract_regex: String,
    hostname: String,
    bid_regex: String,
    bid_template: String,
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

    eprintln!("SSH: running command '{}'", cmd);
    channel.exec(cmd)?;

    let mut stderr = channel.stderr();
    let mut stderr_buffer = String::new();
    stderr.read_to_string(&mut stderr_buffer)?;
    eprintln!("SSH: ERR: {}", &stderr_buffer);

    let mut s = String::new();
    eprintln!("SSH: collect output");
    while !channel.eof() {
        let mut s0 = String::new();
        channel.read_to_string(&mut s0)?;
        s.push_str(&s0);
    }
    eprintln!("SSH: end collecting");

    let exit_status = channel.exit_status().unwrap();
    println!("Exit status {}", &exit_status);
    if exit_status != 0 {
        Err(LucySshError::RemoteError(exit_status, stderr_buffer))
    } else {
        Ok(s)
    }
}

fn gerrit_bid_for_work(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
    change_id: u32,
    patch_set_id: u32,
    bid_id: &str,
) -> Result<String, LucySshError> {
    let data = mustache::MapBuilder::new()
        .insert("config", config)
        .unwrap()
        .insert_str("bid_id", format!("{}", bid_id))
        .build();
    let mut bytes = vec![];

    cconfig.bid_template.render_data(&mut bytes, &data)?;
    let message = String::from_utf8_lossy(&bytes);

    let cmd = &format!(
        "gerrit review {},{} --message '{}'",
        change_id, patch_set_id, message
    );
    run_ssh_command(config, &cmd)
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

    eprintln!("DATE query: {}", &date_str);
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
    eprintln!(
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
                    eprintln!(
                        "Change: {} number {}",
                        &cs.id.clone().unwrap_or("".into()),
                        &cs.number.unwrap_or(0)
                    );
                    if ts < last_timestamp {
                        last_timestamp = ts - 1;
                    }
                    ret_changes.push(cs);
                }
            } else {
                eprintln!("STATS Backend res: {:?}", backend_res);
                if let Ok(stats) = backend_res {
                    ret_stats = Some(stats.clone());
                    more_changes = stats.moreChanges;
                    if stats.rowCount > 0 {
                        use s5ci::*;
                        spawn_simple_command("scripts", "git-mirror");
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
                eprintln!("Command finished with error, code: {:?}", exit_code);
            }
        } else {
            println!("{}", output);
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
struct LucyCiCompiledConfig {
    patchset_extract_regex: Regex,
    bid_regex: Regex,
    trigger_regexes: Vec<CommentTriggerRegex>,
    bid_template: mustache::Template,
    trigger_command_templates: HashMap<String, mustache::Template>,
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
struct CommentBid {
    comment_index: u32,
    bid_id: String,
    hostname: String,
}

fn get_comment_bids(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
    comments_vec: &Vec<GerritComment>,
    startline_ts: i64,
) -> HashMap<String, CommentBid> {
    let mut out: HashMap<String, CommentBid> = HashMap::new();
    for (i, comment) in comments_vec.iter().enumerate() {
        if let Some(rem) = cconfig.bid_regex.captures(&comment.message) {
            if let (Some(bid), Some(host)) = (rem.name("bid_id"), rem.name("hostname")) {
                eprintln!(
                    "Found bid for id {} from host {}",
                    bid.as_str(),
                    host.as_str()
                );
                let bid_id = bid.as_str();
                if !out.contains_key(bid_id) {
                    let new_bid = CommentBid {
                        comment_index: i as u32,
                        bid_id: bid_id.to_string(),
                        hostname: host.as_str().to_string(),
                    };

                    out.insert(bid_id.to_string(), new_bid);
                }
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

pub fn spawn_command(cmd: &str) {
    use std::process::Command;
    use std::process::Stdio;

    let cmd_file = basename_from_cmd(cmd);
    let log_file = open_log_file(&cmd_file).unwrap();
    let stderr_cmd = format!("{}-stderr", cmd_file);
    let log_file_stderr = log_file.try_clone().unwrap(); // open_log_file(&stderr_cmd).unwrap();
                                                         // let errors = outputs.try_clone()?;
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("{}", cmd))
        .stdin(Stdio::null())
        .stdout(log_file)
        .stderr(log_file_stderr)
        .env("RUST_BACKTRACE", "1")
        .spawn()
        .expect("failed to execute child");
}

fn process_change(
    config: &LucyCiConfig,
    cconfig: &LucyCiCompiledConfig,
    cs: &GerritChangeSet,
    before_when: Option<NaiveDateTime>,
    after_when: Option<NaiveDateTime>,
) {
    /* Go through the change patchsets and comments,
    * with the following heuristics:

    1) accumulate:
       - triggers
       - our and others first-bids
       - our actions
    2) for triggers with "singleton" flag, filter out all but last trigger

    3) remove the triggers reacted upon
       - if not our first-bid exists -> sort them into "to check" triggers
       - if our action exists - remove

    */
    let mut triggers: Vec<CommentTrigger> = vec![];

    // eprintln!("Processing change: {:#?}", cs);
    if let Some(startline) = after_when {
        let startline_ts = startline.timestamp();
        let mut psmap: HashMap<String, GerritPatchSet> = HashMap::new();

        if let Some(psets) = &cs.patchSets {
            for pset in psets {
                if pset.createdOn > 0 {
                    // startline_ts {
                    // println!("{:?}", &pset);
                    eprintln!(
                        "  #{} revision: {} ref: {}",
                        &pset.number, &pset.revision, &pset.r#ref
                    );
                    // spawn_command_x("scripts", "git-test", &pset.r#ref);
                }
                psmap.insert(format!("{}", &pset.number), pset.clone());
                psmap.insert(format!("{}", &pset.revision), pset.clone());
            }

            eprintln!("Patchset map: {:#?}", &psmap);
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
            eprintln!("all triggers: {:#?}", &final_triggers);
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
                spawn_command(&expanded_command);
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

fn main() {
    use std::env;
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Supply the YAML config file name");
        return;
    }
    let yaml_fname = &args[1];
    let s = fs::read_to_string(yaml_fname).unwrap();
    let config: LucyCiConfig = serde_yaml::from_str(&s).unwrap();
    eprintln!("Config: {:#?}", &config);
    let trigger_regexes = get_trigger_regexes(&config);
    let patchset_regex = Regex::new(&config.patchset_extract_regex).unwrap();
    let bid_regex = Regex::new(&config.bid_regex).unwrap();
    let bid_template = mustache::compile_str(&config.bid_template).unwrap();
    let trigger_command_templates = get_trigger_command_templates(&config);
    let cconfig = LucyCiCompiledConfig {
        patchset_extract_regex: patchset_regex,
        bid_regex: bid_regex,
        trigger_regexes: trigger_regexes,
        bid_template: bid_template,
        trigger_command_templates: trigger_command_templates,
    };

    let sync_horizon_sec: u32 = config
        .server
        .sync_horizon_sec
        .unwrap_or(config.default_sync_horizon_sec.unwrap_or(86400));

    let mut before: Option<NaiveDateTime> = None;
    let mut after: Option<NaiveDateTime> = Some(NaiveDateTime::from_timestamp(
        (now_naive_date_time().timestamp() - sync_horizon_sec as i64),
        0,
    ));

    loop {
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
        ps();
        eprintln!("Sleeping for {} msec ({})", wait_time_ms, wait_name);
        s5ci::thread_sleep_ms(wait_time_ms);
    }
}
