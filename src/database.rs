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
