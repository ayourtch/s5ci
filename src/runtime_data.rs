use crate::gerrit_types::GerritVoteAction;
use crate::s5ci_config::*;
use chrono::NaiveDateTime;
use clap::{App, Arg, SubCommand};
use regex::Regex;
use s5ci::set_db_url;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone)]
pub struct CommentTriggerRegex {
    pub r: Regex,
    pub r_suppress: Option<Regex>,
    pub name: String,
}

pub struct CronTriggerSchedule {
    pub schedule: cron::Schedule,
    _cron: String,
    pub name: String,
}

impl std::fmt::Debug for CronTriggerSchedule {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "CronTriggerSchedule [name: {}]", &self.name)
    }
}

impl CronTriggerSchedule {
    pub fn from_str(cron_str: &str, name: &str) -> Self {
        use cron::Schedule;
        use std::str::FromStr;

        let schedule = cron::Schedule::from_str(cron_str).unwrap();
        CronTriggerSchedule {
            schedule: schedule,
            _cron: cron_str.to_string(),
            name: name.to_string(),
        }
    }
}

impl std::clone::Clone for CronTriggerSchedule {
    fn clone(&self) -> Self {
        use std::str::FromStr;
        CronTriggerSchedule {
            schedule: cron::Schedule::from_str(&self._cron).unwrap(),
            _cron: self._cron.clone(),
            name: self.name.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct s5ciRunJobArgs {
    pub cmd: String,
    pub omit_if_ok: bool,
    pub kill_previous: bool,
}

#[derive(Debug, Clone)]
pub struct s5ciProcessGerritReplyArgs {
    pub input_file: String,
    pub before: Option<NaiveDateTime>,
    pub after: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub enum s5ciAction {
    Loop,
    ListJobs,
    SetStatus(String, String),
    RunJob(s5ciRunJobArgs),
    ProcessGerritReply(s5ciProcessGerritReplyArgs),
    KillJob(String),
    GerritCommand(String),
    MakeReview(Option<GerritVoteAction>, String),
}

#[derive(Debug, Clone)]
pub struct s5ciRuntimeData {
    pub config_path: String,
    pub sandbox_level: u32,
    pub patchset_extract_regex: Regex,
    pub unsafe_char_regex: Regex,
    pub unsafe_start_regex: Regex,
    pub trigger_regexes: Vec<CommentTriggerRegex>,
    pub trigger_command_templates: HashMap<String, mustache::Template>,
    pub cron_trigger_schedules: Vec<CronTriggerSchedule>,
    pub action: s5ciAction,
    pub changeset_id: Option<u32>,
    pub patchset_id: Option<u32>,
    pub real_s5ci_exe: String,
}

pub fn get_cron_trigger_schedules(config: &s5ciConfig) -> Vec<CronTriggerSchedule> {
    let mut out = vec![];
    if let Some(cron_triggers) = &config.cron_triggers {
        for (name, trig) in cron_triggers {
            out.push(CronTriggerSchedule::from_str(&trig.cron, &name));
        }
    }

    out
}

pub fn get_trigger_regexes(config: &s5ciConfig) -> Vec<CommentTriggerRegex> {
    let mut out = vec![];
    if let Some(triggers) = &config.triggers {
        for (name, trig) in triggers {
            let r = Regex::new(&trig.regex).unwrap();
            let r_suppress = trig.suppress_regex.clone().map(|x| Regex::new(&x).unwrap());
            out.push(CommentTriggerRegex {
                r: r,
                r_suppress: r_suppress.clone(),
                name: name.clone(),
            });
        }
    }

    out
}

pub fn get_trigger_command_templates(config: &s5ciConfig) -> HashMap<String, mustache::Template> {
    let mut out = HashMap::new();
    if let Some(triggers) = &config.triggers {
        for (name, trig) in triggers {
            if let s5TriggerAction::command(cmd) = &trig.action {
                let full_cmd = format!("{}/{}", &config.command_rootdir, &cmd);
                let template = mustache::compile_str(cmd).unwrap();
                out.insert(name.clone(), template);
            }
        }
    }
    out
}

pub fn get_configs() -> (s5ciConfig, s5ciRuntimeData) {
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
            SubCommand::with_name("process-gerrit-reply")
                .about("process saved JSON reply from gerrit")
                .arg(
                    Arg::with_name("input-file")
                        .short("i")
                        .help("input text file as sent by grrit")
                        .required(true)
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("before-ts")
                        .short("b")
                        .help("timestamp for 'before' edge of the range")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("after-ts")
                        .short("a")
                        .help("timestamp for 'after' edge of the range")
                        .takes_value(true),
                )
                )
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
    if let Some(matches) = matches.subcommand_matches("process-gerrit-reply") {
        let input_file = matches.value_of("input-file").unwrap().to_string();
        let before = matches
            .value_of("before-ts")
            .map(|x| x.parse::<i64>().unwrap_or(0))
            .map(|x| NaiveDateTime::from_timestamp(x, 0));
        let after = matches
            .value_of("after-ts")
            .map(|x| x.parse::<i64>().unwrap_or(0))
            .map(|x| NaiveDateTime::from_timestamp(x, 0));
        action = s5ciAction::ProcessGerritReply(s5ciProcessGerritReplyArgs {
            input_file,
            before,
            after,
        });
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
