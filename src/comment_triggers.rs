use crate::database::db_get_changeset_last_comment_id;
use crate::database::db_set_changeset_last_comment_id;
use crate::gerrit_types::*;
use crate::runtime_data::s5ciRuntimeData;
use crate::s5ci_config::s5ciConfig;
use crate::s5ci_config::s5TriggerAction;
use chrono::NaiveDateTime;
use std::collections::HashMap;
use crate::job_mgmt::spawn_command;

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


pub fn process_gerrit_change(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    cs: &GerritChangeSet,
    before_when: Option<NaiveDateTime>,
    after_when: Option<NaiveDateTime>,
) {
    let mut triggers: Vec<CommentTrigger> = vec![];
    let mut max_pset = 0;

    // eprintln!("Processing change: {:#?}", cs);
    if let Some(startline) = after_when {
        let startline_ts =
            startline.timestamp() - 1 + config.default_regex_trigger_delay_sec.unwrap_or(0) as i64;

        debug!(
            "process change with startline timestamp: {}",
            startline.timestamp()
        );
        debug!("process change with startline_ts: {}", &startline_ts);

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
                if pset.number > max_pset {
                    max_pset = pset.number;
                }
            }

            // eprintln!("Patchset map: {:#?}", &psmap);
        }
        if let Some(comments_vec) = &cs.comments {
            let change_id = cs.number.unwrap() as i32;
            let all_triggers = get_comment_triggers_from_comments(
                config,
                rtdt,
                change_id,
                max_pset,
                comments_vec,
                startline_ts,
            );
            let mut final_triggers = all_triggers.clone();
            let mut suppress_map: HashMap<(String, u32), bool> = HashMap::new();
            for mut ctrig in final_triggers.iter_mut().rev() {
                let key = (ctrig.trigger_name.clone(), ctrig.patchset_id);
                if ctrig.is_suppress {
                    suppress_map.insert(key, true);
                } else if suppress_map.contains_key(&key) {
                    ctrig.is_suppressed = true;
                    suppress_map.remove(&key);
                }
            }
            if let Some(cfgt) = &config.triggers {
                final_triggers.retain(|x| {
                    let ctrig = &cfgt[&x.trigger_name];
                    let mut retain = !x.is_suppressed;
                    if let Some(proj) = &ctrig.project {
                        if let Some(cs_proj) = &cs.project {
                            if cs_proj != proj {
                                retain = false;
                            }
                        } else {
                            retain = false;
                        }
                    }
                    if let s5TriggerAction::command(cmd) = &ctrig.action {
                        retain
                    } else {
                        false
                    }
                });
                // now purge all the suppressing triggers themselves
                final_triggers.retain(|x| !x.is_suppress);
            }
            // eprintln!("all triggers: {:#?}", &final_triggers);
            eprintln!("final triggers: {:#?}", &final_triggers);
            for trig in &final_triggers {
                let template = rtdt
                    .trigger_command_templates
                    .get(&trig.trigger_name)
                    .unwrap();
                let mut data = mustache::MapBuilder::new();
                if let Some(patchset) = psmap.get(&format!("{}", trig.patchset_id)) {
                    data = data.insert("patchset", &patchset).unwrap();
                }
                data = data.insert("regex", &trig.captures).unwrap();
                let data = data.build();
                let mut bytes = vec![];

                template.render_data(&mut bytes, &data).unwrap();
                let expanded_command = String::from_utf8_lossy(&bytes);
                let change_id = cs.number.unwrap();
                let mut rtdt2 = rtdt.clone();
                rtdt2.changeset_id = Some(change_id);
                rtdt2.patchset_id = Some(trig.patchset_id);
                if (trig.is_suppress || trig.is_suppressed) {
                    panic!(format!("bug: job is not runnable: {:#?}", &trig));
                }
                let job_id = spawn_command(config, &rtdt2, &expanded_command);
            }
        }
    }
}
