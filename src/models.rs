use crate::schema;
use chrono;
use chrono::NaiveDateTime;
use diesel;
use diesel::connection::TransactionManager;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel::types::*;
use schema::*;
use serde_derive;
use std;
#[serde(default)]
#[derive(Queryable, Insertable, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[table_name = "comments"]
pub struct comment {
    pub record_uuid: String,
    pub changeset_id: i32,
    pub comment_id: i32,
}

#[serde(default)]
#[derive(Queryable, Insertable, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[table_name = "counters"]
pub struct counter {
    pub name: String,
    pub value: i32,
}

#[serde(default)]
#[derive(Queryable, Insertable, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[table_name = "jobs"]
pub struct job {
    pub record_uuid: String,
    pub job_group_name: String,
    pub instance_id: String,
    pub job_id: String,
    pub job_pid: i32,
    pub parent_job_id: Option<String>,
    pub changeset_id: i32,
    pub patchset_id: i32,
    pub command: String,
    pub command_pid: Option<i32>,
    pub remote_host: Option<String>,
    pub status_message: String,
    pub status_updated_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
    pub started_at: Option<NaiveDateTime>,
    pub finished_at: Option<NaiveDateTime>,
    pub return_success: bool,
    pub return_code: Option<i32>,
    pub trigger_event_id: Option<String>,
}

#[serde(default)]
#[derive(Queryable, Insertable, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[table_name = "timestamps"]
pub struct timestamp {
    pub name: String,
    pub value: Option<NaiveDateTime>,
}
