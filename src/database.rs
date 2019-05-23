use chrono::NaiveDateTime;
use diesel;
use s5ci::*;

pub fn db_get_changeset_last_comment_id(a_changeset_id: i32) -> i32 {
    let comment = db_get_comment_by_changeset_id(a_changeset_id);
    if comment.is_ok() {
        return comment.unwrap().comment_id;
    } else {
        use uuid::Uuid;
        let my_uuid = Uuid::new_v4().to_simple().to_string();
        let new_comment = models::comment {
            record_uuid: my_uuid,
            changeset_id: a_changeset_id,
            comment_id: -1,
        };
        let db = get_db();
        {
            use diesel::query_dsl::RunQueryDsl;
            use schema::comments;
            use schema::comments::dsl::*;

            diesel::insert_into(comments::table)
                .values(&new_comment)
                .execute(db.conn())
                .expect(&format!("Error inserting new comment {}", &a_changeset_id));
        }
        return -1;
    }
}

pub fn db_set_changeset_last_comment_id(a_changeset_id: i32, a_comment_id: i32) {
    let db = get_db();
    use diesel::expression_methods::*;
    use diesel::query_dsl::QueryDsl;
    use diesel::query_dsl::RunQueryDsl;
    use schema::comments;
    use schema::comments::dsl::*;

    let updated_rows = diesel::update(comments.filter(changeset_id.eq(a_changeset_id)))
        .set((comment_id.eq(a_comment_id),))
        .execute(db.conn())
        .unwrap();
}
fn db_get_next_counter_value(a_name: &str) -> Result<i32, String> {
    db_get_next_counter_value_with_min(a_name, 0)
}

fn get_lock_path(a_name: &str) -> String {
    let lock_path = format!("/tmp/{}.lock", &a_name);
    lock_path
}

fn lock_named(a_name: &str) -> Result<(), String> {
    let lock_path = get_lock_path(a_name);
    let max_retry_count = 20;
    let mut retry_count = max_retry_count;
    while std::fs::create_dir(&lock_path).is_err() {
        if retry_count == 0 {
            return Err(format!("Failed to lock {} after several tries", a_name));
        }
        // wait for sometime
        s5ci::thread_sleep_ms(300 * (1 + max_retry_count - retry_count));
        retry_count = retry_count - 1;
    }
    Ok(())
}

fn unlock_named(a_name: &str) -> Result<(), String> {
    let lock_path = get_lock_path(a_name);
    if std::fs::remove_dir(&lock_path).is_ok() {
        Ok(())
    } else {
        Err(format!("error unlocking {}", a_name))
    }
}

pub fn db_get_next_counter_value_with_min(a_name: &str, a_min: i32) -> Result<i32, String> {
    use diesel::connection::Connection;
    use diesel::expression_methods::*;
    use diesel::query_dsl::QueryDsl;
    use diesel::query_dsl::RunQueryDsl;
    use diesel::result::Error;
    use schema::counters;
    use schema::counters::dsl::*;

    let db = get_db();
    let conn = db.conn();
    let mut result: Result<i32, String> = Err(format!("result unset"));
    lock_named(&a_name).unwrap();

    conn.transaction::<_, Error, _>(|| {
        let res = counters
            .filter(name.eq(a_name))
            .limit(2)
            .load::<models::counter>(conn);

        let count_val: Result<i32, String> = match res {
            Ok(r) => match r.len() {
                0 => {
                    let curr_value = a_min;
                    let new_counter = models::counter {
                        name: format!("{}", a_name),
                        value: curr_value + 1,
                    };
                    diesel::insert_into(counters::table)
                        .values(&new_counter)
                        .execute(conn);
                    Ok(curr_value)
                }
                1 => {
                    let curr_val = if r[0].value < a_min {
                        a_min
                    } else {
                        r[0].value
                    };

                    diesel::update(counters.filter(name.eq(a_name)))
                        .set((value.eq(curr_val + 1)))
                        .execute(conn);
                    Ok(curr_val)
                }
                _ => Err(format!("More than one counter of type {}", a_name)),
            },
            Err(e) => Err(format!("counter select error: {:?}", &e)),
        };
        result = count_val;
        Ok(())
    })
    .unwrap();

    if result.is_ok() {
        let r = counters
            .filter(name.eq(a_name))
            .limit(1)
            .load::<models::counter>(conn)
            .unwrap();
        if Ok(r[0].value - 1) != result {
            /* there was another transaction in parallel, retry */
            s5ci::thread_sleep_ms(r[0].value as u64 % 100);
            result = Ok(db_get_next_counter_value_with_min(a_name, a_min).unwrap());
        }
    }
    unlock_named(&a_name).unwrap();
    result
}

pub fn db_get_timestamp(a_name: &str, a_default: NaiveDateTime) -> NaiveDateTime {
    use diesel::connection::Connection;
    use diesel::expression_methods::*;
    use diesel::query_dsl::QueryDsl;
    use diesel::query_dsl::RunQueryDsl;
    use diesel::result::Error;
    use schema::timestamps;
    use schema::timestamps::dsl::*;

    let db = get_db();
    let conn = db.conn();

    let res = timestamps
        .filter(name.eq(a_name))
        .limit(2)
        .load::<models::timestamp>(conn);
    if let Ok(r) = &res {
        match r.len() {
            0 => a_default,
            _ => r[0].value,
        }
    } else {
        a_default
    }
}

pub fn db_update_timestamp(a_name: &str, a_value: NaiveDateTime) {
    use diesel::connection::Connection;
    use diesel::expression_methods::*;
    use diesel::query_dsl::QueryDsl;
    use diesel::query_dsl::RunQueryDsl;
    use diesel::result::Error;
    use schema::timestamps;
    use schema::timestamps::dsl::*;

    let db = get_db();
    let conn = db.conn();

    conn.transaction::<_, Error, _>(|| {
        let res = timestamps
            .filter(name.eq(a_name))
            .limit(2)
            .load::<models::timestamp>(conn);

        if let Ok(r) = &res {
            match r.len() {
                0 => {
                    let new_timestamp = models::timestamp {
                        name: format!("{}", a_name),
                        value: a_value,
                    };
                    diesel::insert_into(timestamps::table)
                        .values(&new_timestamp)
                        .execute(conn);
                }
                _ => {
                    diesel::update(timestamps.filter(name.eq(a_name)))
                        .set((value.eq(a_value)))
                        .execute(conn);
                }
            }
        };
        res
    })
    .unwrap();
}
