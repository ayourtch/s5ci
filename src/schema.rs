table! {
    comments (record_uuid) {
        record_uuid -> Text,
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
    jobs (record_uuid) {
        record_uuid -> Text,
        job_group_name -> Text,
        instance_id -> Integer,
        job_id -> Text,
        parent_job_id -> Nullable<Text>,
        changeset_id -> Integer,
        comment_id -> Integer,
        command -> Text,
        remote_host -> Nullable<Text>,
        started_at -> Nullable<Timestamp>,
        finished_at -> Nullable<Timestamp>,
        return_success -> Bool,
        return_code -> Nullable<Integer>,
    }
}

allow_tables_to_appear_in_same_query!(comments, counters, jobs,);
