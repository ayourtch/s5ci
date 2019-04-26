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
#[derive(Queryable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[table_name = "comments"]
pub struct comment {
    pub uuid: String,
    pub changeset_id: i32,
    pub comment_id: i32,
}

#[derive(Queryable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[table_name = "counters"]
pub struct counter {
    pub name: String,
    pub value: i32,
}

#[derive(Queryable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[table_name = "jobs"]
pub struct job {
    pub uuid: String,
    pub job_name: String,
    pub id: i32,
    pub changeset_id: i32,
    pub comment_id: i32,
    pub command: String,
    pub started_at: Option<NaiveDateTime>,
    pub finished_at: Option<NaiveDateTime>,
    pub return_code: Option<i32>,
}
