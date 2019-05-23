use crate::gerrit_types::*;
use crate::run_ssh_command::run_ssh_command;
use crate::run_ssh_command::s5SshError;
use crate::run_ssh_command::s5SshResult;
use crate::runtime_data::s5ciRuntimeData;
use crate::s5ci_config::s5ciConfig;
use chrono::NaiveDateTime;
use s5ci::now_naive_date_time;

fn gerrit_query_changes(
    config: &s5ciConfig,
    before_when: Option<NaiveDateTime>,
    after_when: Option<NaiveDateTime>,
) -> Result<String, s5SshError> {
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

pub fn poll_gerrit_over_ssh(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    before_when: Option<NaiveDateTime>,
    after_when: Option<NaiveDateTime>,
) -> Result<s5SshResult, s5SshError> {
    let s = gerrit_query_changes(config, before_when, after_when)?;
    parse_gerrit_poll_command_reply(config, rtdt, before_when, after_when, &s)
}

pub fn parse_gerrit_poll_command_reply(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    before_when: Option<NaiveDateTime>,
    after_when: Option<NaiveDateTime>,
    command_reply: &str,
) -> Result<s5SshResult, s5SshError> {
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
    let mut ret_changes: Vec<GerritChangeSet> = vec![];
    if command_reply != "" {
        for line in command_reply.lines() {
            // eprintln!("{}", &line);
            let backend_res: Result<GerritQueryError, serde_json::Error> =
                serde_json::from_str(&format!("{}", &line));
            if let Ok(error) = backend_res {
                if &error.r#type == "error" {
                    return Err(s5SshError::QueryBackendError(error.message));
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
    Ok(s5SshResult {
        before_when: ret_before_when,
        after_when: ret_after_when,
        output: command_reply.to_string(),
        changes: ret_changes,
        stats: ret_stats,
    })
}

pub fn gerrit_add_review_comment(
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
