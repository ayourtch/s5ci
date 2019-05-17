use crate::s5ci_config::*;
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
