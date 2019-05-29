#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

extern crate regex;
extern crate yaml_rust;
#[macro_use]
extern crate lazy_static;
use regex::Regex;

use std::collections::HashMap;
use yaml_rust::yaml;
use yaml_rust::{Yaml, YamlEmitter, YamlLoader};

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate serde_yaml;

extern crate ant_style_matcher;
use ant_style_matcher::AntPathMatcher;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JenkinsParseConfig {
    jenkins_files: Vec<String>,
    s5ci_template: Option<String>,
    s5ci_output_file: String,
    node_filter: Option<String>,
    debug_add_node: bool,
    debug_add_job: bool,
    base_dir: Option<String>,
}

fn load_config(yaml_fname: Option<String>) -> JenkinsParseConfig {
    let yaml_fname = yaml_fname.unwrap_or("jenkins-parse.yaml".to_string());
    let yaml_fname = std::fs::canonicalize(yaml_fname)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let s = std::fs::read_to_string(&yaml_fname).unwrap();
    let mut config: JenkinsParseConfig = serde_yaml::from_str(&s).unwrap();
    if config.base_dir.is_none() {
        let mut base_path = std::fs::canonicalize(yaml_fname).unwrap();
        base_path.pop();
        config.base_dir = Some(base_path.to_str().unwrap().to_string());
    }
    config
}

fn process_terminal(
    template_str: &str,
    template: &Yaml,
    vars: &HashMap<String, Yaml>,
    all_templates: &HashMap<String, HashMap<String, Yaml>>,
) -> Vec<Yaml> {
    // println!("String {} has no substitutions", &template_str);
    // println!("VARS: {:#?}", &vars);
    let mut out_triggers: yaml::Array = yaml::Array::new();
    let mut new_template = yaml_subst(&template, vars);
    let mut out_yaml: Vec<Yaml> = vec![];

    if let yaml::Yaml::Array(ref arr) = template["triggers"] {
        for x in arr {
            // println!("X  trigger: {:#?}", &x);
            if let yaml::Yaml::Hash(ref h) = x {
                for (k, v) in h {
                    let mut trigger_vars: HashMap<String, Yaml> = vars.clone();
                    let key = k.as_str().unwrap();
                    // println!("Trigger key: {}", &key);
                    if key == "timed" {
                        out_triggers.push(yaml::Yaml::Hash(h.clone()));
                    } else if key == "reverse" {
                        // out_triggers.push(yaml::Yaml::Hash(h.clone()));
                        let xx = yaml_subst(&v, &trigger_vars);
                        out_triggers.push(xx);
                    } else {
                        fill_defaults(&mut trigger_vars, &all_templates);
                        fill_vars_with_subst(&mut trigger_vars, &v, &vars);
                        // println!("  trigger: {:#?}", &trigger_vars);
                        if all_templates["trigger"].contains_key(key) {
                            let vv = yaml_subst(&all_templates["trigger"][key], &trigger_vars);
                            // println!("TEMPLATE: {:#?}", &vv);
                            if let yaml::Yaml::Array(ref vv_arr) = vv["triggers"] {
                                for xx in vv_arr {
                                    // println!("XX: {:#?}", &xx);
                                    out_triggers.push(xx.clone());
                                }
                            }
                        } else if key == "gerrit" {
                            let xx = yaml_subst(&Yaml::Hash(h.clone()), &trigger_vars);
                            // println!("NO-TEMPLATE: for key '{}': {:#?}", &key, &xx);
                            out_triggers.push(xx);
                        } else {
                            panic!("Could not find key '{}' for a trigger");
                        }
                    }
                }
            }
        }
        // println!("OUT TRIGGERS: {:#?}", &out_triggers);
    }
    // replace the triggers with the expanded ones
    if let yaml::Yaml::Hash(ref mut h) = new_template {
        let mut count = 1;
        for trig in out_triggers {
            let mut h1 = h.clone();
            h1[&Yaml::String("triggers".to_string())] = Yaml::Array(vec![trig]);
            let unique_id = Yaml::String(format!(
                "uniq-{}-{}",
                h[&Yaml::String("name".to_string())].as_str().unwrap(),
                count
            ));
            h1.insert(Yaml::String("unique-id".to_string()), unique_id);
            out_yaml.push(Yaml::Hash(h1));
            count = count + 1;
        }
    }
    out_yaml
}

fn interpolate(
    template_str: &str,
    template_yaml: &Yaml,
    vars: &HashMap<String, Yaml>,
    all_templates: &HashMap<String, HashMap<String, Yaml>>,
) -> Vec<Yaml> {
    lazy_static! {
        // static ref re_subst: Regex = Regex::new(r"[{]([^-A-Za-z0-9]+)[}]").unwrap();
        static ref re_subst: Regex = Regex::new(r"[{]([-_.A-Za-z0-9]+)[}]").unwrap();
    }
    let mut out_yaml: Vec<Yaml> = vec![];
    // println!("interpolating {}, VARS: {:#?}", template_str, &vars);
    let maybe_mat = re_subst.find(template_str);
    if let Some(mat) = maybe_mat {
        let left_tmp_str = &template_str[0..mat.start()];
        let right_tmp_str = &template_str[mat.end()..];

        let var_name = template_str[mat.start() + 1..mat.end() - 1].to_string();
        // println!("var_name: {}", &var_name);
        match &vars[&var_name] {
            yaml::Yaml::Array(ref v) => {
                for x in v {
                    match x {
                        yaml::Yaml::Hash(ref h) => {
                            for (k, v) in h {
                                let key = k.as_str().unwrap();
                                let new_template_str =
                                    format!("{}{}{}", left_tmp_str, key, right_tmp_str);
                                // println!(" hash key {:?}, new template: {}", k, &new_template_str);
                                let mut new_vars = vars.clone();
                                // println!("Insert key itself {} => {:#?}", &var_name, &key);
                                new_vars
                                    .insert(var_name.to_string(), Yaml::String(key.to_string()));
                                if let yaml::Yaml::Hash(ref h) = v {
                                    for (k, v) in h {
                                        let key = k.as_str().unwrap().to_string();
                                        // println!("Adding vars: {} = {:#?}", &key, &v);
                                        new_vars.insert(key, v.clone());
                                    }
                                } else {
                                    println!(" hash value: {:#?}", &v);
                                }
                                new_vars.insert(var_name.clone(), k.clone());
                                // FIXME: trawl the values and update the vars
                                for x in interpolate(
                                    &new_template_str,
                                    template_yaml,
                                    &new_vars,
                                    all_templates,
                                ) {
                                    out_yaml.push(x);
                                }
                            }
                        }
                        x => {
                            let key = x.as_str().unwrap();
                            let new_template_str =
                                format!("{}{}{}", left_tmp_str, key, right_tmp_str);
                            // println!(" literal {:?}, new template: {}", x, &new_template_str);
                            let mut new_vars = vars.clone();
                            // new_vars.insert(key.to_string(), v.clone());
                            new_vars.insert(var_name.to_string(), Yaml::String(key.to_string()));
                            for x in interpolate(
                                &new_template_str,
                                template_yaml,
                                &new_vars,
                                all_templates,
                            ) {
                                out_yaml.push(x);
                            }
                        }
                    }
                    // println!("array elt: {:#?}", &x);
                }
            }
            yaml::Yaml::Hash(ref h) => {
                for (k, v) in h {
                    println!("{:?}:", k);
                }
                panic!("Hash detected rather than array");
            }
            x => {
                println!("{:?}", x);
                panic!("should be an array");
            }
        }
    } else {
        // no {}-variables to substitute
        for x in process_terminal(template_str, template_yaml, vars, all_templates) {
            out_yaml.push(x);
        }
    }
    out_yaml
}

fn subst_yaml_from_str(template_str: &str, subst_vars: &HashMap<String, Yaml>) -> Yaml {
    lazy_static! {
        // static ref re_subst: Regex = Regex::new(r"[{]([^-A-Za-z0-9]+)[}]").unwrap();
        static ref re_subst: Regex = Regex::new(r"[{]([-_.A-Za-z0-9]+)[}]").unwrap();
    }
    // println!("INTERPOLATE {}", template_str);
    // println!("subst vars: {:#?}", &subst_vars);
    let maybe_mat = re_subst.find(template_str);
    if let Some(mat) = maybe_mat {
        let left_tmp_str = &template_str[0..mat.start()];
        let right_tmp_str = &template_str[mat.end()..];

        let var_name = template_str[mat.start() + 1..mat.end() - 1].to_string();
        // println!("var_name: {}", &var_name);
        if left_tmp_str == "" && right_tmp_str == "" {
            yaml_subst(&subst_vars[&var_name], subst_vars)
        } else {
            // FIXME: combined string substitutions
            if subst_vars.contains_key(&var_name) {
                let new_yaml = yaml_subst(&subst_vars[&var_name], subst_vars);
                if let Yaml::String(s) = new_yaml {
                    subst_yaml_from_str(
                        &format!("{}{}{}", left_tmp_str, s, right_tmp_str),
                        subst_vars,
                    )
                } else {
                    panic!(
                        "unexpected yaml {:?} in string interp for {} in {}",
                        &new_yaml, &var_name, template_str
                    );
                }
            } else {
                println!(
                    "Can not find value of variable '{}' for substitution",
                    &var_name
                );
                panic!(
                    "Can not find value of variable '{}' for substitution",
                    &var_name
                );
            }
        }
    } else {
        Yaml::String(template_str.to_string())
    }
}

fn yaml_subst(v: &Yaml, subst_vars: &HashMap<String, Yaml>) -> Yaml {
    use yaml::Yaml;
    match v {
        Yaml::Array(ref v) => {
            let mut vv: yaml::Array = vec![];
            for iv in v {
                vv.push(yaml_subst(iv, subst_vars));
            }
            Yaml::Array(vv)
        }
        Yaml::Hash(ref h) => {
            let mut hh: yaml::Hash = yaml::Hash::new();
            for (k, v) in h {
                let kk = yaml_subst(k, subst_vars);
                let vv = yaml_subst(v, subst_vars);
                hh.insert(kk, vv);
            }
            Yaml::Hash(hh)
        }
        Yaml::String(s) => subst_yaml_from_str(s, subst_vars),
        x => x.clone(),
    }
}

fn fill_vars_with_subst(
    vars: &mut HashMap<String, Yaml>,
    fill_from: &Yaml,
    subst_vars: &HashMap<String, Yaml>,
) {
    // println!("FROM: {:#?}", &fill_from);

    if let yaml::Yaml::Hash(ref h) = fill_from {
        for (k, v) in h {
            let key = k.as_str().unwrap();
            let v_subst = yaml_subst(&v, subst_vars);
            // println!("   key: {:#?} = {:#?}", &key, &v_subst);
            vars.insert(key.to_string(), v_subst.clone());
        }
    }
}

fn fill_vars(vars: &mut HashMap<String, Yaml>, fill_from: &Yaml) {
    // println!("FROM: {:#?}", &fill_from);

    if let yaml::Yaml::Hash(ref h) = fill_from {
        for (k, v) in h {
            let key = k.as_str().unwrap();
            // println!("   key: {:#?} = {:#?}", &key, &v);
            vars.insert(key.to_string(), v.clone());
        }
    }
}

fn fill_defaults(
    vars: &mut HashMap<String, Yaml>,
    templates: &HashMap<String, HashMap<String, Yaml>>,
) {
    let global_defaults = &templates["defaults"]["global"];
    fill_vars(vars, global_defaults);
}

fn process_section(
    section: &Yaml,
    templates: &HashMap<String, HashMap<String, Yaml>>,
) -> Vec<Yaml> {
    let mut out_yaml: Vec<Yaml> = vec![];
    let mut vars: HashMap<String, Yaml> = HashMap::new();
    let job_templates = &templates["job-template"];
    fill_defaults(&mut vars, &templates);
    fill_vars(&mut vars, &section);
    // println!("Section: {:#?}", &section);
    if let yaml::Yaml::Hash(ref h) = &section {
        for (k, v) in h {
            let key = k.as_str().unwrap();
            if key == "jobs" {
                if let yaml::Yaml::Array(ref v) = v {
                    for x in v {
                        let jobname = x.as_str().unwrap().to_string();
                        // println!( "job: {:?}, has template: {:?}", &jobname, job_templates.contains_key(&jobname));
                        for x in interpolate(&jobname, &job_templates[&jobname], &vars, templates) {
                            out_yaml.push(x);
                        }
                    }
                }
            }
        }
    }
    out_yaml
}

fn load_yaml_doc(filename: &str) -> Yaml {
    let data = std::fs::read_to_string(filename).unwrap();
    let docs = YamlLoader::load_from_str(&data).unwrap();
    // Multi document support, doc is a yaml::Yaml
    docs[0].clone()
}

fn fill_templates(filename: &str, templates: &mut HashMap<String, HashMap<String, Yaml>>) {
    let data = std::fs::read_to_string(filename).unwrap();
    let docs = YamlLoader::load_from_str(&data).unwrap();
    // Multi document support, doc is a yaml::Yaml
    let doc = &docs[0];

    for root in doc.as_vec().unwrap() {
        if let yaml::Yaml::Hash(ref h) = &root {
            for (k, v) in h {
                let key = k.as_str().unwrap();
                // println!("root-key: {:#?}", &key);
                let template_key = v["name"].as_str().unwrap().to_string();
                let mut inner_hash = templates.entry(key.to_string()).or_insert(HashMap::new());
                inner_hash.insert(template_key, v.clone());
                if let yaml::Yaml::Hash(ref h) = &v {
                    for (k, v) in h {
                        // println!("   key: {:#?}", &k.as_str().unwrap());
                    }
                }
                // println!("------");
            }
        }
    }
}

fn hash_add_yaml_node(parent: &mut Yaml, key: &str, child: Yaml) {
    if let yaml::Yaml::Hash(ref mut h) = *parent {
        h.insert(Yaml::String(key.to_string()), child);
    } else {
        panic!("Yaml {:?} is not a hash", parent);
    }
}

fn vec_add_yaml_node(parent: &mut Yaml, child: Yaml) {
    if let yaml::Yaml::Array(ref mut v) = *parent {
        v.push(child);
    } else {
        panic!("Yaml {:?} is not an array", parent);
    }
}

fn yaml_str(parent: &Yaml) -> &str {
    parent.as_str().unwrap()
}

fn yaml_get_compare_pattern(parent: &Yaml, prefix: &str) -> Yaml {
    let idx_type = format!("{}-compare-type", prefix);
    let idx_pat = format!("{}-pattern", prefix);

    if let Yaml::String(compat) = &parent[idx_type.as_str()] {
        match compat.as_str() {
            "ANT" => parent[idx_pat.as_str()].clone(),
            x => {
                panic!("Unknown comparison type {}", x);
            }
        }
    } else {
        panic!("Not a string index");
    }
}

fn yaml_hash_first(parent: &Yaml) -> (&Yaml, &Yaml) {
    parent.as_hash().unwrap().iter().nth(0).unwrap()
}

fn yaml_get_gerrit_trigger_regex(triggers: &Yaml) -> Yaml {
    let mut acc: Vec<String> = vec![];
    if let Yaml::Array(v) = triggers {
        for trig in v {
            match trig {
                Yaml::String(s) => {
                    match s.as_str() {
                        "draft-published-event" => {
                            // acc.push(s.to_string());
                        }
                        x => {
                            // acc.push(x.to_string());
                        }
                    }
                }
                Yaml::Hash(h) => {
                    let t = yaml_hash_first(trig);
                    match t.0.as_str().unwrap() {
                        //Uploaded patch set
                        "patchset-created-event" => {
                            acc.push("Uploaded patch set ".to_string());
                            assert!(t.1["exclude-drafts"].as_str().unwrap() == "true");
                            assert!(t.1["exclude-trivial-rebase"].as_str().unwrap() == "false");
                            // assert!(t.1["exclude-no-code-change"].as_str().unwrap() == "false");
                        }
                        "comment-added-contains-event" => {
                            acc.push(t.1["comment-contains-value"].as_str().unwrap().to_string());
                        }
                        x => {
                            // acc.push(x.to_string());
                        }
                    }
                }
                x => {
                    acc.push("unknown trigger".to_string());
                }
            }
        }
    }
    Yaml::String(acc.join("|"))
}

pub fn main() {
    let args: Vec<String> = std::env::args().collect();
    let config_fname = if args.len() > 1 {
        Some(args[1].to_string())
    } else {
        None
    };
    jenkins_parse(config_fname);
}

pub fn jenkins_parse(config_fname: Option<String>) {
    let config = load_config(config_fname);
    if let Some(cwdpath) = config.base_dir {
         std::env::set_current_dir(cwdpath).unwrap();
    }

    let mut templates: HashMap<String, HashMap<String, Yaml>> = HashMap::new();

    // flil in the job template hash
    for fname in &config.jenkins_files {
        fill_templates(&fname, &mut templates);
    }

    // println!("TEMPLATE DATA: {:#?}", &templates);
    //
    let mut interim_yaml: Vec<Yaml> = vec![];
    let mut out_yaml = if let Some(fname) = config.s5ci_template {
        load_yaml_doc(&fname)
    } else {
        Yaml::Hash(yaml::Hash::new())
    };

    for (k, v) in &templates["project"] {
        for x in process_section(&v, &templates) {
            interim_yaml.push(x);
        }
    }

    println!("# Total from jenkins: {} jobs", interim_yaml.len());
    println!("# node filter: {:?}", &config.node_filter);

    // hash_add_yaml_node(&mut out_yaml, "Test", Yaml::String("testval".to_string()));
    let mut triggers = Yaml::Hash(yaml::Hash::new());
    let mut cron_triggers = Yaml::Hash(yaml::Hash::new());
    // repopulate the triggers with those from the template (we will overwrite)
    if let Some(h) = out_yaml["triggers"].as_hash() {
        for (k, v) in h {
            hash_add_yaml_node(&mut triggers, yaml_str(k), v.clone());
        }
    }
    if let Some(h) = out_yaml["cron_triggers"].as_hash() {
        for (k, v) in h {
            hash_add_yaml_node(&mut cron_triggers, yaml_str(k), v.clone());
        }
    }

    let ant_matcher = AntPathMatcher::new();

    for job in &interim_yaml {
        let mut j = Yaml::Hash(yaml::Hash::new());
        let key = job["unique-id"].as_str().unwrap();
        let node = job["node"].as_str().unwrap();

        if let Some(node_filt) = &config.node_filter {
            if !ant_matcher.is_match(node_filt, node) {
                continue;
            }
        }

        // hash_add_yaml_node(&mut j, "job-id", job["unique-id"].clone());
        assert!(job["triggers"].as_vec().unwrap().len() == 1);
        let trig = job["triggers"][0].as_hash().unwrap().iter().nth(0).unwrap();
        if config.debug_add_node {
            hash_add_yaml_node(&mut j, "node", job["node"].clone());
        }
        if config.debug_add_job {
            hash_add_yaml_node(&mut j, "job", job.clone());
        }

        // hash_add_yaml_node(&mut j, "job", job["triggers"][0].clone());
        if let Yaml::String(trig_type) = trig.0 {
            // println!("Trigger type: {}", &trig_type);
            match trig_type.as_str() {
                "gerrit" => {
                    if trig.1["projects"].is_array() {
                        assert!(trig.1["projects"].as_vec().unwrap().len() == 1);
                        if let Some(v) = trig.1["projects"][0]["branches"].as_vec() {
                            assert!(trig.1["projects"][0]["branches"].as_vec().unwrap().len() == 1);
                        } else {
                            panic!("can not get branches for project at job {:#?}", &job);
                        }
                        // let project = trig.1["projects"]
                        let proj = yaml_get_compare_pattern(&trig.1["projects"][0], "project");
                        let branch = yaml_get_compare_pattern(
                            &trig.1["projects"][0]["branches"][0],
                            "branch",
                        );
                        hash_add_yaml_node(&mut j, "project", proj);
                        hash_add_yaml_node(&mut j, "branch", branch);
                    }
                    let regex = yaml_get_gerrit_trigger_regex(&trig.1["trigger-on"]);

                    hash_add_yaml_node(&mut j, "regex", regex);

                    let mut action = Yaml::Hash(yaml::Hash::new());
                    let cmd = format!("{}", yaml_str(&job["name"]));
                    hash_add_yaml_node(&mut action, "command", Yaml::String(cmd));
                    hash_add_yaml_node(&mut j, "action", action);

                    // hash_add_yaml_node(&mut j, key, trig.1.clone());
                    hash_add_yaml_node(&mut triggers, key, j);
                }
                "timed" => {
                    let mut action = Yaml::Hash(yaml::Hash::new());
                    let cmd = format!("{}", yaml_str(&job["name"]));
                    hash_add_yaml_node(&mut action, "command", Yaml::String(cmd));
                    hash_add_yaml_node(&mut j, "action", action);
                    hash_add_yaml_node(
                        &mut j,
                        "cron",
                        Yaml::String(format!("0 {} *", yaml_str(trig.1))),
                    );
                    hash_add_yaml_node(&mut cron_triggers, key, j);
                }
                "jobs" => { /* unhandled currently */ }
                x => {
                    panic!("unknown trigger type '{}' on node {:#?}", &x, &job);
                }
            }
        }
    }
    println!(
        "# Total for s5ci: {} comment triggers",
        triggers.as_hash().unwrap().len()
    );
    println!(
        "# Total for s5ci: {} cron triggers",
        cron_triggers.as_hash().unwrap().len()
    );

    hash_add_yaml_node(&mut out_yaml, "triggers", triggers);
    hash_add_yaml_node(&mut out_yaml, "cron_triggers", cron_triggers);

    /*
    let mut total_triggers = 0;
    for j in out_yaml {
        println!("Triggers: {}", j["triggers"].as_vec().unwrap().len());

    }
    */
    println!("# DUMP START");
    // Dump the YAML object
    let mut out_str = String::new();
    {
        let mut emitter = YamlEmitter::new(&mut out_str);
        // emitter.dump(&Yaml::Array(interim_yaml)).unwrap(); // dump the YAML object to a String
        emitter.dump(&out_yaml).unwrap(); // dump the YAML object to a String
    }
    std::fs::write(config.s5ci_output_file, out_str);
    // println!("{}", out_str);
}
