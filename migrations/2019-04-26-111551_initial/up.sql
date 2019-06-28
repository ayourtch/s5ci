-- Your SQL goes here


CREATE TABLE comments (
	record_uuid VARCHAR(32) NOT NULL PRIMARY KEY,
	changeset_id INT NOT NULL,
	comment_id INT NOT NULL
);

CREATE TABLE jobs (
	record_uuid VARCHAR(32) NOT NULL PRIMARY KEY,
	job_group_name VARCHAR NOT NULL,
	instance_id INT NOT NULL,
	job_id VARCHAR NOT NULL,
	job_pid INT NOT NULL,
	parent_job_id VARCHAR,
	changeset_id INT NOT NULL,
	patchset_id INT NOT NULL,
	command VARCHAR NOT NULL,
	command_pid INT,
	remote_host VARCHAR,
	status_message VARCHAR NOT NULL,
	status_updated_at datetime,
	started_at datetime,
	finished_at datetime,
	return_success BOOLEAN NOT NULL,
	return_code INT,
	trigger_event_id VARCHAR
);

CREATE TABLE counters (
	name VARCHAR NOT NULL PRIMARY KEY,
	value INT NOT NULL
);

CREATE TABLE timestamps (
	name VARCHAR NOT NULL PRIMARY KEY,
	value datetime
);
