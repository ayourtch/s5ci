use crate::database::db_get_changeset_last_comment_id;
use crate::database::db_set_changeset_last_comment_id;
use crate::gerrit_types::*;
use crate::runtime_data::s5ciRuntimeData;
use crate::s5ci_config::s5ciConfig;
use chrono::NaiveDateTime;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CommentTrigger {
    pub comment_index: u32,
    pub trigger_name: String,
    pub patchset_id: u32,
    pub captures: HashMap<String, String>,
    pub is_suppress: bool,
    pub is_suppressed: bool,
}

// make anything other than -/_ or alphanum an underscore
fn safe_or_underscores(rtdt: &s5ciRuntimeData, val: &str) -> String {
    rtdt.unsafe_start_regex
        .replace_all(
            &rtdt.unsafe_char_regex.replace_all(val, "_").to_string(),
            "_",
        )
        .to_string()
}

pub fn get_comment_triggers_from_comments(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    changeset_id: i32,
    max_pset: u32,
    comments_vec: &Vec<GerritComment>,
    startline_ts: i64,
) -> Vec<CommentTrigger> {
    let trigger_regexes = &rtdt.trigger_regexes;
    let mut out = vec![];

    let last_seen_comment_id = db_get_changeset_last_comment_id(changeset_id);

    for (i, comment) in comments_vec.iter().enumerate() {
        debug!("Comment: {}: {:#?}", i, &comment);
        if comment.timestamp > startline_ts {
            if (i as i32) < last_seen_comment_id {
                /* already saw it */
                continue;
            }
            let mut safe_patchset_str = "".to_string();
            /*
            eprintln!(
                "    comment at {} by {}: {}",
                comment.timestamp,
                comment.reviewer.email.clone().unwrap_or("unknown".into()),
                comment.message
            );
            */
            if let Some(rem) = rtdt.patchset_extract_regex.captures(&comment.message) {
                if let Some(ps) = rem.name("patchset") {
                    safe_patchset_str = safe_or_underscores(rtdt, ps.as_str());
                }
            }
            for tr in trigger_regexes {
                if tr.r.is_match(&comment.message) {
                    let mut captures: HashMap<String, String> = HashMap::new();
                    captures.insert("patchset".into(), safe_patchset_str.clone());
                    // eprintln!("        Comment matched regex {}", &tr.name);
                    // try to extract the patchset from the start of comment
                    for m in tr.r.captures(&comment.message) {
                        for maybe_name in tr.r.capture_names() {
                            if let Some(name) = maybe_name {
                                if let Some(val) = m.name(&name) {
                                    let safe_val = safe_or_underscores(rtdt, val.as_str());
                                    captures.insert(name.to_string(), safe_val);
                                }
                            }
                        }
                    }

                    if !captures["patchset"].parse::<u32>().is_ok() {
                        if !comment
                            .message
                            .starts_with("Change has been successfully merged by ")
                        {
                            error!(
                                "unparseable patchset in {:#?}: {:#?}",
                                &comment, &safe_patchset_str
                            );
                        } else {
                            captures.insert("patchset".into(), format!("{}", &max_pset));
                        }
                    }
                    let patchset_id = captures["patchset"].parse::<u32>().unwrap();
                    let trigger_name = format!("{}", &tr.name);
                    let trig = CommentTrigger {
                        comment_index: i as u32,
                        trigger_name: trigger_name,
                        captures: captures,
                        patchset_id: patchset_id,
                        is_suppress: false,
                        is_suppressed: false,
                    };
                    out.push(trig);
                }
                if let Some(r_suppress) = &tr.r_suppress {
                    if r_suppress.is_match(&comment.message) {
                        let mut captures: HashMap<String, String> = HashMap::new();
                        captures.insert("patchset".into(), safe_patchset_str.clone());
                        // eprintln!("        Comment matched regex {}", &tr.name);
                        // try to extract the patchset from the start of comment
                        for m in r_suppress.captures(&comment.message) {
                            for maybe_name in tr.r.capture_names() {
                                if let Some(name) = maybe_name {
                                    if let Some(val) = m.name(&name) {
                                        let safe_val = safe_or_underscores(rtdt, val.as_str());
                                        captures.insert(name.to_string(), safe_val);
                                    }
                                }
                            }
                        }

                        if !captures["patchset"].parse::<u32>().is_ok() {
                            if !comment
                                .message
                                .starts_with("Change has been successfully merged by ")
                            {
                                error!(
                                    "unparseable patchset in {:#?}: {:#?}",
                                    &comment, &safe_patchset_str
                                );
                            } else {
                                captures.insert("patchset".into(), format!("{}", &max_pset));
                            }
                        }
                        let patchset_id = captures["patchset"].parse::<u32>().unwrap();
                        let trigger_name = format!("{}", &tr.name);
                        let trig = CommentTrigger {
                            comment_index: i as u32,
                            trigger_name: trigger_name,
                            captures: captures,
                            patchset_id: patchset_id,
                            is_suppress: true,
                            is_suppressed: false,
                        };
                        out.push(trig);
                    }
                }
            }
        }
    }
    db_set_changeset_last_comment_id(changeset_id, comments_vec.len() as i32);

    out
}