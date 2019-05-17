use crate::gerrit_types::*;
use crate::s5ci_config::*;
use chrono::NaiveDateTime;
use mustache;
use serde_json;
use ssh2;
use ssh2::Session;
use std::io;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct s5SshResult {
    pub before_when: Option<NaiveDateTime>,
    pub after_when: Option<NaiveDateTime>,
    pub output: String,
    pub changes: Vec<GerritChangeSet>,
    pub stats: Option<GerritQueryStats>,
}

#[derive(Debug)]
pub enum s5SshError {
    Ssh2Error(ssh2::Error),
    IoError(io::Error),
    SerdeJsonError(serde_json::Error),
    RemoteError(i32, String),
    QueryBackendError(String),
    MustacheError(mustache::Error),
}

impl std::convert::From<ssh2::Error> for s5SshError {
    fn from(error: ssh2::Error) -> Self {
        s5SshError::Ssh2Error(error)
    }
}

impl std::convert::From<io::Error> for s5SshError {
    fn from(error: io::Error) -> Self {
        s5SshError::IoError(error)
    }
}

impl std::convert::From<serde_json::Error> for s5SshError {
    fn from(error: serde_json::Error) -> Self {
        s5SshError::SerdeJsonError(error)
    }
}
impl std::convert::From<mustache::Error> for s5SshError {
    fn from(error: mustache::Error) -> Self {
        s5SshError::MustacheError(error)
    }
}

pub fn run_ssh_command(config: &s5ciConfig, cmd: &str) -> Result<String, s5SshError> {
    match &config.server.poll_type {
        s5ciPollType::direct_ssh(x) => run_ssh_command_direct(config, cmd),
        s5ciPollType::shell(x) => run_ssh_command_shell(config, x, cmd),
    }
}

fn run_ssh_command_shell(
    config: &s5ciConfig,
    sc: &s5ciShellPoll,
    cmd: &str,
) -> Result<String, s5SshError> {
    use std::env;
    use std::process::{Command, Stdio};

    let child = Command::new("/usr/bin/ssh")
        .args(&sc.args)
        .arg(format!("{}", cmd))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to execute child");

    let output = child.wait_with_output().expect("failed to wait on child");

    //  assert!(output.status.success());
    let exit_status = output.status.code().unwrap();
    if exit_status != 0 {
        let txt = String::from_utf8_lossy(&output.stderr);
        Err(s5SshError::RemoteError(exit_status, txt.to_string()))
    } else {
        let txt = String::from_utf8_lossy(&output.stdout);
        Ok(txt.to_string())
    }
}

fn run_ssh_command_direct(config: &s5ciConfig, cmd: &str) -> Result<String, s5SshError> {
    // Connect to the local SSH server
    let tcp = TcpStream::connect(config.server.get_server_address_port())?; // .unwrap();
    let ssh_auth = &config.default_auth;
    // let tcp = TcpStream::connect("gerrit.fd.io:29418").unwrap();
    let mut sess = Session::new().unwrap();
    sess.handshake(&tcp)?;
    match ssh_auth {
        s5SshAuth::auth_pubkey_file(pk) => {
            sess.userauth_pubkey_file(
                &pk.username,
                None,
                Path::new(&pk.privatekey),
                pk.passphrase.as_ref().map_or(None, |x| Some(&**x)),
            )?;
        }
        s5SshAuth::auth_password(pw) => {
            sess.userauth_password(&pw.username, &pw.password)?;
        }
        s5SshAuth::auth_agent(agent) => {
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
        Err(s5SshError::RemoteError(exit_status, stderr_buffer))
    } else {
        Ok(s)
    }
}
