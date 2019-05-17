use crate::s5ci_config::GerritVoteAction;
use regex::Regex;
use std::collections::HashMap;

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
pub enum s5ciAction {
    Loop,
    ListJobs,
    SetStatus(String, String),
    RunJob(s5ciRunJobArgs),
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
