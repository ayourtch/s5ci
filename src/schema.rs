table! {
    comments (uuid) {
        uuid -> Text,
        changeset_id -> Integer,
        comment_id -> Integer,
    }
}

table! {
    counters (name) {
        name -> Text,
        value -> Integer,
    }
}

table! {
    jobs (uuid) {
        uuid -> Text,
        job_name -> Text,
        id -> Integer,
        changeset_id -> Integer,
        comment_id -> Integer,
        command -> Text,
        started_at -> Nullable<Timestamp>,
        finished_at -> Nullable<Timestamp>,
        return_code -> Nullable<Integer>,
    }
}

allow_tables_to_appear_in_same_query!(comments, counters, jobs,);
