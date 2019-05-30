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
        job_pid -> Integer,
        parent_job_id -> Nullable<Text>,
        changeset_id -> Integer,
        patchset_id -> Integer,
        command -> Text,
        command_pid -> Nullable<Integer>,
        remote_host -> Nullable<Text>,
        status_message -> Text,
        status_updated_at -> Nullable<Timestamp>,
        started_at -> Nullable<Timestamp>,
        finished_at -> Nullable<Timestamp>,
        return_success -> Bool,
        return_code -> Nullable<Integer>,
    }
}

table! {
    timestamps (name) {
        name -> Text,
        value -> Nullable<Timestamp>,
    }
}

allow_tables_to_appear_in_same_query!(comments, counters, jobs, timestamps,);
