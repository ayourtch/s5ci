pub mod models;
pub mod schema;

#[macro_use]
extern crate diesel;
extern crate chrono;
extern crate dotenv;
extern crate libc;
extern crate num_cpus;
extern crate r2d2;
extern crate r2d2_diesel;
#[macro_use]
extern crate lazy_static;
extern crate procinfo;

#[macro_use]
extern crate serde_derive;

extern crate serde;
#[macro_use]
extern crate serde_json;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenv::dotenv;
use r2d2::{Pool, PooledConnection};
use r2d2_diesel::ConnectionManager;

use std::collections::HashMap;
use std::sync::Mutex;

/* a global mostly read-only string collection */

lazy_static! {
    static ref GLOBAL_HASH: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

pub fn set_db_url(url: &str) {
    GLOBAL_HASH
        .lock()
        .unwrap()
        .insert("DATABASE_URL".to_string(), url.to_string());
}

pub fn get_db_url() -> String {
    format!(
        "{}",
        GLOBAL_HASH.lock().unwrap().get("DATABASE_URL").unwrap()
    )
}

pub fn sqlite3_establish_connection() -> SqliteConnection {
    dotenv().ok();
    use diesel::connection::SimpleConnection;

    let database_url = get_db_url();
    let connection = SqliteConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url));
    connection
}

type DB_CONN_TYPE = SqliteConnection;
pub const DB_TYPE_NAME: &str = "Sqlite";
pub fn uncached_establish_connection() -> DB_CONN_TYPE {
    sqlite3_establish_connection()
}

pub fn get_db_info() -> String {
    dotenv().ok();

    let database_url = get_db_url();

    format!(
        "Database flavour: {}, DATABASE_URL: {}",
        DB_TYPE_NAME, database_url
    )
}

pub fn create_db_pool() -> Pool<ConnectionManager<DB_CONN_TYPE>> {
    dotenv().ok();

    let database_url = get_db_url();
    let manager = ConnectionManager::<DB_CONN_TYPE>::new(database_url);
    let pool = Pool::builder()
        .max_size(3)
        .build(manager)
        .expect("Failed to create pool.");
    pool
}

lazy_static! {
    pub static ref DB_POOL: Pool<ConnectionManager<DB_CONN_TYPE>> = create_db_pool();
}

pub struct DB(PooledConnection<ConnectionManager<DB_CONN_TYPE>>);

use diesel::connection::SimpleConnection;

impl DB {
    pub fn conn(&self) -> &DB_CONN_TYPE {
        let connection = &*self.0;
        if DB_TYPE_NAME == "Sqlite" {
            connection.batch_execute("PRAGMA busy_timeout=20000;");
        }
        &*self.0
    }
}

pub fn get_db() -> DB {
    use std::thread;
    use std::time::Duration;

    loop {
        match DB_POOL.get() {
            Ok(conn) => break DB(conn),
            Err(e) => {
                println!(
                    "Could not get a conn! increase the # in the pool (err: {:?})",
                    e
                );
                thread::sleep(Duration::from_millis(10));
                continue;
            }
        }
    }
}

pub fn flush_stdout() {
    use std::io::Write;
    std::io::stdout().flush().unwrap();
}

macro_rules! define_db_get_even_deleted {
    ( $fnname: ident, $typ: ty, $types: ident, $idfield: ident, $idtype: ty) => {
        pub fn $fnname(item_id: $idtype) -> Result<$typ, String> {
            use schema::$types::dsl::*;
            let db = get_db();
            let res = $types
                .filter($idfield.eq(item_id))
                .limit(2)
                .load::<$typ>(db.conn())
                .expect(&format!("Error loading {}", stringify!($types)));
            let thing = format!(
                "{} with {} == {}",
                stringify!($typ),
                stringify!($idfield),
                &item_id
            );
            match res.len() {
                1 => Ok(res[0].clone()),
                x => {
                    let thing = format!(
                        "{} with {} == {}",
                        stringify!($typ),
                        stringify!($idfield),
                        &item_id
                    );
                    let msg = if x == 0 {
                        format!("{} not found even deleted", &thing)
                    } else {
                        format!("more than one {} even deleted", &thing)
                    };
                    Err(msg)
                }
            }
        }
    };
}

define_db_get_even_deleted!(db_get_job, models::job, jobs, job_id, &str);

pub fn db_get_all_jobs() -> Vec<models::job> {
    use schema::jobs::dsl::*;
    let db = get_db();
    let results = jobs
        .load::<models::job>(db.conn())
        .expect("Error loading jobs");
    results
}

pub fn db_get_child_jobs(a_parent_job_id: &str) -> Vec<models::job> {
    use schema::jobs::dsl::*;
    let db = get_db();
    let results = jobs
        .filter(parent_job_id.eq(a_parent_job_id))
        .load::<models::job>(db.conn())
        .expect("Error loading jobs");
    results
}

pub fn db_get_root_jobs() -> Vec<models::job> {
    use schema::jobs::dsl::*;
    let db = get_db();
    let results = jobs
        .filter(parent_job_id.is_null())
        .load::<models::job>(db.conn())
        .expect("Error loading jobs");
    results
}

pub fn now_naive_date_time() -> chrono::NaiveDateTime {
    /* return the "now" value of naivedatetime */
    use chrono::*;
    let ndt = Local::now().naive_local();
    ndt
}

pub fn ndt_add_seconds(ndt: chrono::NaiveDateTime, sec: i32) -> chrono::NaiveDateTime {
    use chrono::prelude::*;
    use chrono::*;
    ndt.checked_add_signed(Duration::seconds(sec as i64))
        .unwrap()
}
pub fn ndt_add_seconds_safe(ndt: chrono::NaiveDateTime, sec: i32) -> Option<chrono::NaiveDateTime> {
    use chrono::prelude::*;
    use chrono::*;
    ndt.checked_add_signed(Duration::seconds(sec as i64))
}

pub fn thread_sleep_ms(ms: u64) {
    use std::thread;
    use std::time::Duration;
    thread::sleep(Duration::from_millis(ms));
}

use std::fs::File;

pub fn open_log_file(verb: &str) -> std::io::Result<File> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let logfname = format!("{}", verb);
    let file = OpenOptions::new().append(true).create(true).open(logfname);
    file
}

pub fn spawn_simple_command(command_dir: &str, cmd_file: &str) {
    use std::process::Command;
    use std::process::Stdio;

    let cmd = format!("{}/{}", command_dir, cmd_file);
    let log_file = open_log_file(cmd_file).unwrap();
    let stderr_cmd = format!("{}-stderr", cmd_file);
    let log_file_stderr = log_file.try_clone().unwrap(); // open_log_file(&stderr_cmd).unwrap();
                                                         // let errors = outputs.try_clone()?;
    let mut child = Command::new(cmd)
        // .arg("--debug")
        // .arg(verb)
        .stdin(Stdio::null())
        .stdout(log_file)
        .stderr(log_file_stderr)
        .env("RUST_BACKTRACE", "1")
        .spawn()
        .expect("failed to execute child");
}

pub fn spawn_command_x(command_dir: &str, cmd_file: &str, arg_ref: &str) {
    use std::process::Command;
    use std::process::Stdio;

    let cmd = format!("{}/{}", command_dir, cmd_file);
    let log_file = open_log_file(cmd_file).unwrap();
    let stderr_cmd = format!("{}-stderr", cmd_file);
    let log_file_stderr = log_file.try_clone().unwrap(); // open_log_file(&stderr_cmd).unwrap();
                                                         // let errors = outputs.try_clone()?;
    let mut child = Command::new(cmd)
        // .arg("--debug")
        // .arg(verb)
        .stdin(Stdio::null())
        .stdout(log_file)
        .stderr(log_file_stderr)
        .env("RUST_BACKTRACE", "1")
        .env("ARG_REF", arg_ref)
        .spawn()
        .expect("failed to execute child");
}
