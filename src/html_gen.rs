use crate::runtime_data::s5ciRuntimeData;
use crate::s5ci_config::s5ciConfig;
use mustache::{MapBuilder, Template};
use s5ci::*;
use std::collections::HashMap;
use std::fs;

pub fn maybe_compile_template(
    config: &s5ciConfig,
    name: &str,
) -> Result<Template, mustache::Error> {
    let res = mustache::compile_path(format!(
        "{}/templates/{}.mustache",
        &config.install_rootdir, name
    ));
    if res.is_err() {
        error!("Could not compile template {}: {:#?}", name, &res);
    }
    res
}

fn fill_and_write_template(
    template: &Template,
    data: MapBuilder,
    fname: &str,
) -> std::io::Result<()> {
    let mut bytes = vec![];
    let data_built = data.build();
    template
        .render_data(&mut bytes, &data_built)
        .expect("Failed to render");
    let payload = std::str::from_utf8(&bytes).unwrap();
    let res = fs::write(&fname, payload);
    res
}

const jobs_per_page: usize = 20;

fn regenerate_group_html(config: &s5ciConfig, rtdt: &s5ciRuntimeData, group_name: &str) {
    let template = maybe_compile_template(config, "group_job_page").unwrap();
    let mut data = MapBuilder::new();
    let jobs = db_get_jobs_by_group_name(group_name);
    data = data.insert("job_group_name", &group_name).unwrap();
    data = data.insert("child_jobs", &jobs).unwrap();
    let fname = format!("{}/{}/index.html", &config.jobs.rootdir, group_name);
    fill_and_write_template(&template, data, &fname).unwrap();
}

fn regenerate_root_html(config: &s5ciConfig, rtdt: &s5ciRuntimeData) {
    let template = maybe_compile_template(config, "root_job_page").unwrap();
    let rjs = db_get_root_jobs();
    let total_jobs = rjs.len();
    let max_n_first_page_jobs = total_jobs % jobs_per_page + jobs_per_page;
    let n_nonfirst_pages = if total_jobs > max_n_first_page_jobs {
        (total_jobs - max_n_first_page_jobs) / jobs_per_page
    } else {
        0
    };
    let n_first_page_jobs = if total_jobs > max_n_first_page_jobs {
        max_n_first_page_jobs
    } else {
        total_jobs
    };

    let mut data = MapBuilder::new();
    let fname = format!("{}/index.html", &config.jobs.rootdir);
    if n_first_page_jobs < total_jobs {
        let first_prev_page_number = (total_jobs - n_first_page_jobs) / jobs_per_page;
        let mut prev_page_number = first_prev_page_number;
        let prev_page_name = format!("index_{}.html", prev_page_number);
        data = data.insert("prev_page_name", &prev_page_name).unwrap();

        loop {
            let mut data_n = MapBuilder::new();
            let curr_page_number = prev_page_number;
            if curr_page_number == 0 {
                break;
            }
            prev_page_number = prev_page_number - 1;
            let next_page_number = curr_page_number + 1;
            if prev_page_number > 0 {
                let prev_page_name = format!("index_{}.html", prev_page_number);
                data_n = data_n.insert("prev_page_name", &prev_page_name).unwrap();
            }
            let next_page_name = if next_page_number <= first_prev_page_number {
                format!("index_{}.html", next_page_number)
            } else {
                format!("index.html")
            };
            data_n = data_n.insert("next_page_name", &next_page_name).unwrap();
            let start_job =
                n_first_page_jobs + (n_nonfirst_pages - curr_page_number) * jobs_per_page;
            let end_job = start_job + jobs_per_page;

            let cjobs = rjs[start_job..end_job].to_vec();

            data_n = data_n.insert("child_jobs", &cjobs).unwrap();

            let fname = format!("{}/index_{}.html", &config.jobs.rootdir, curr_page_number);
            debug!("writing template {}", &fname);
            fill_and_write_template(&template, data_n, &fname).unwrap();
        }
    }
    let cjobs = rjs[0..n_first_page_jobs].to_vec();
    data = data.insert("child_jobs", &cjobs).unwrap();
    fill_and_write_template(&template, data, &fname).unwrap();
}

fn regenerate_active_html(config: &s5ciConfig, rtdt: &s5ciRuntimeData) {
    let template = maybe_compile_template(config, "active_job_page").unwrap();
    let mut data = MapBuilder::new();
    let rjs = db_get_active_jobs();
    data = data.insert("child_jobs", &rjs).unwrap();
    let fname = format!("{}/active.html", &config.jobs.rootdir);
    fill_and_write_template(&template, data, &fname).unwrap();
}

fn save_job_yaml(config: &s5ciConfig, job: &models::job) {
    let fname = format!("{}/{}/job.yaml", &config.jobs.rootdir, &job.job_id);
    let ys = serde_yaml::to_string(job).unwrap();
    std::fs::write(&fname, ys).unwrap();
}

fn save_job_json(config: &s5ciConfig, job: &models::job) {
    let fname = format!("{}/{}/job.json", &config.jobs.rootdir, &job.job_id);
    let js = serde_json::to_string(job).unwrap();
    std::fs::write(&fname, js).unwrap();
}

fn regenerate_html(
    config: &s5ciConfig,
    rtdt: &s5ciRuntimeData,
    job_id: &str,
    update_parent: bool,
    update_children: bool,
    groups: &mut HashMap<String, i32>,
) {
    use mustache::{Data, MapBuilder};
    let j = db_get_job(job_id).expect(&format!("Could not get job id {} from db", job_id));
    save_job_yaml(config, &j);
    save_job_json(config, &j);
    let template = maybe_compile_template(config, "job_page").unwrap();
    let mut data = MapBuilder::new();

    data = data.insert("job", &j).unwrap();
    if let Some(pjob_id) = &j.parent_job_id {
        let pj = db_get_job(pjob_id).expect(&format!("Could not get job id {} from db", pjob_id));
        data = data.insert("parent_job", &pj).unwrap();
    }
    let cjs = db_get_child_jobs(job_id);
    data = data.insert("child_jobs", &cjs).unwrap();

    let fname = format!("{}/{}/index.html", &config.jobs.rootdir, job_id);
    fill_and_write_template(&template, data, &fname).unwrap();

    if update_children {
        for cj in cjs {
            regenerate_html(config, rtdt, &cj.job_id, false, false, groups);
        }
    }

    if update_parent {
        if let Some(pjob_id) = &j.parent_job_id {
            regenerate_html(config, rtdt, pjob_id, false, false, groups);
        } else {
            regenerate_root_html(config, rtdt);
        }
    }
    groups.insert(
        j.job_group_name.clone(),
        1 + groups.get(&j.job_group_name).unwrap_or(&0),
    );
}

pub fn regenerate_job_html(config: &s5ciConfig, rtdt: &s5ciRuntimeData, job_id: &str) {
    let mut groups = HashMap::new();
    regenerate_html(config, rtdt, job_id, true, true, &mut groups);
    for (group_name, count) in groups {
        println!("Regenerating group {} with {} jobs", &group_name, count);
        regenerate_group_html(config, rtdt, &group_name);
    }
    regenerate_active_html(config, rtdt);
}

pub fn starting_job(config: &s5ciConfig, rtdt: &s5ciRuntimeData, job_id: &str) {
    regenerate_job_html(config, rtdt, job_id);
}

pub fn finished_job(config: &s5ciConfig, rtdt: &s5ciRuntimeData, job_id: &str) {
    regenerate_job_html(config, rtdt, job_id);
}

pub fn regenerate_all_html(config: &s5ciConfig, rtdt: &s5ciRuntimeData) {
    let jobs = db_get_all_jobs();
    let mut groups = HashMap::new();
    for j in jobs {
        // println!("Regenerate HTML for {}", &j.job_id);
        regenerate_html(config, rtdt, &j.job_id, false, false, &mut groups);
    }
    for (group_name, count) in groups {
        println!("Regenerating group {} with {} jobs", &group_name, count);
        regenerate_group_html(config, rtdt, &group_name);
    }
    regenerate_root_html(config, rtdt);
    regenerate_active_html(config, rtdt);
}
