-- Your SQL goes here


CREATE TABLE comments (
	uuid VARCHAR(32) NOT NULL PRIMARY KEY,
	changeset_id INT NOT NULL,
	comment_id INT NOT NULL
);

CREATE TABLE jobs (
	uuid VARCHAR(32) NOT NULL PRIMARY KEY,
	job_name VARCHAR NOT NULL,
	id INT NOT NULL,
	changeset_id INT NOT NULL,
	comment_id INT NOT NULL,
	command VARCHAR NOT NULL,
	started_at datetime,
	finished_at datetime,
	return_code INT
);

CREATE TABLE counters (
	name VARCHAR NOT NULL PRIMARY KEY,
	value INT NOT NULL
)


