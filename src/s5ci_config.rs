use chrono::NaiveDateTime;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BeforeAfter {
    Before(NaiveDateTime),
    After(NaiveDateTime),
    Any,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5SshAuthPubkeyFile {
    pub username: String,
    pub pubkey: Option<String>,
    pub privatekey: String,
    pub passphrase: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5SshAuthPassword {
    pub username: String,
    pub password: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5SshAuthAgent {
    pub username: String,
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum s5SshAuth {
    auth_pubkey_file(s5SshAuthPubkeyFile),
    auth_password(s5SshAuthPassword),
    auth_agent(s5SshAuthAgent),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5ciDirectSshPoll {
    pub auth: Option<s5SshAuth>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5ciShellPoll {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum s5ciPollType {
    direct_ssh(s5ciDirectSshPoll),
    shell(s5ciShellPoll),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5ciPollGerrit {
    pub address: std::net::IpAddr,
    pub port: u16,
    pub poll_type: s5ciPollType,
    pub poll_wait_ms: Option<u64>,
    pub syncing_poll_wait_ms: Option<u64>,
    pub sync_horizon_sec: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5GerritQuery {
    pub filter: String,
    pub options: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5GerritVote {
    pub success: String,
    pub failure: String,
    pub clear: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum s5TriggerAction {
    event(String),
    command(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5GerritTrigger {
    pub project: Option<String>,
    pub branch: Option<String>,
    pub regex: String,
    pub suppress_regex: Option<String>,
    pub action: s5TriggerAction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5CronTrigger {
    pub cron: String,
    pub action: s5TriggerAction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5ciJobs {
    pub rootdir: String,
    pub root_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5AutorestartConfig {
    pub on_config_change: bool,
    pub on_exe_change: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5PerProjectConfigInfo {
    pub root_dir: String,
    pub projects: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5PerProjectConfig {
    pub commit_triggers: Option<HashMap<String, s5GerritTrigger>>,
    pub cron_triggers: Option<HashMap<String, s5CronTrigger>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5ciConfig {
    pub default_auth: s5SshAuth,
    pub server: s5ciPollGerrit,
    pub default_query: s5GerritQuery,
    pub default_vote: s5GerritVote,
    // unused at the moment.
    pub default_batch_command: Option<String>,
    pub default_sync_horizon_sec: Option<u32>,
    pub default_regex_trigger_delay_sec: Option<u32>,
    pub command_rootdir: String,
    pub triggers: Option<HashMap<String, s5GerritTrigger>>,
    pub cron_triggers: Option<HashMap<String, s5CronTrigger>>,
    pub patchset_extract_regex: String,
    pub hostname: String,
    pub install_rootdir: String,
    pub autorestart: s5AutorestartConfig,
    pub db_url: String,
    pub jobs: s5ciJobs,
    pub per_project_config: Option<s5PerProjectConfigInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5ciProjectConfig {
    pub commit_triggers: Option<HashMap<String, s5GerritTrigger>>,
    pub cron_triggers: Option<HashMap<String, s5CronTrigger>>,
}

impl s5ciPollGerrit {
    pub fn get_server_address_port(self: &Self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}
