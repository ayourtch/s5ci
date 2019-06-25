"s5" in s5CI stands for "s{imple}". Having had experience with many "Simple" protocols like SNMP, SMTP, SIP,
I deliberately chose this notation to distance from explicitly calling things "Simple".

Yes, this time it is different. Maybe. What gives me hope ? Because I am trying to do as little as possible,
with the following principles.

Avoid dynamic content
=====================

No dynamic content. The entirety of HTTP-accessible content is static and is generated during the changes.
Consequently, there is no specialized daemon to worry about (rather than nginx), and the overall vulnerable
footprint is reduced. There *may* be in the future some dynamic additions - but they will be implemented
reluctantly, only after concluding that there is no alternative.

Single upstream - gerrit
========================

s5ci has a single upstream: gerrit. Gerrit is a reasonable platform for code reviews, and has a SSH-based
interface that can spit out JSON for data. Not that this may not change - with slight tweaking s5CI can be
adapted to any review system, just that Gerrit was the one available/used/good enough for the job of doing
code-review based developer workflow.

Jobs are just shell-scripts
===========================

s5CI aims itself at relatively controlled environments, so it is deemed that using OS as a mechanism
for job control should be reasonable. Consequently, every job is just a shell script. If it returns 0,
the job succeeds. If it returns non-zero - the job fails. A job is executed by calling the s5ci executable
with parameter "run-job" - and it that calls /bin/sh in a child process with the script to execute.
This way this s5ci process can perform the necessary house-keeping - redraw the html, put a timestamp
into the database when the process finishes, etc.

A job can launch one or more other jobs using the same "run-job" mechanism - using environment variables
the parent-child relationship will be tracked and depicted accordigly in the UI.

This also means that once a job starts, it is not much dependent on the main loop of s5CI still being
present - if you want to put a node into maintenance mode, simply stop the main loop and wait for
the rest of the s5ci processes to finish!

This split of "job acquisition loop" and "distributed job management" allows for greater simplicity
and debuggability - the two parts are completely independent conceptually.

Triggers are regexes and crons
==============================

Even if a job can start a job, the first job needs to be started by something. There are two mechanisms:
Cron triggers and Comment triggers.


Cron triggers
=============

Cron triggers are defined pieces of configiration as follows:

```
cron_triggers:
  every_half_hour:
      #            sec  min   hour   day of month   month   day of week   year
      cron:        "0    0,30     *          *        *          *          * "
      action:
         command: run-cron twice-per-hour
```

"run-cron" is the shell script which will be launched every half an hour with "twice-per-hour" argument.

Comment triggers
================

Comment triggers fire on the comments made to the changesets. Every so often, s5CI polls the upstream and
retrieves the list of changesets which have been updated since the last poll. Then for each of the changesets
it walks the list of comments and attempts to match all of the configured regexes on them and builds
the vector of candidate jobs. If the regex for the job is matched but immediately in one of the following
comments the *suppress_regex* matches as well, the execution of that job is suppressed. This allows to arrange
for simple time-based server redundancy mechanisms.

The configuration for a regex trigger looks as follows:

```
triggers:
  echo_comment:
      regex: \secho (?P<testval>.+)
      # optional suppress_regex will delete the about-to-be-started jobs of the same name
      suppress_regex: Build http
      action:
        command: run-echo {{regex.testval}}
```

The script itself might look as follows:

```
#!/bin/sh
set -eux
${S5CI_EXE} review -m "Build ${S5CI_JOB_URL} has finished: $1"
```

In this case, the regex will match and will extract the "testval" variable, which can be referenced
in order to build the full command line to run the job. if in the same processing batch there is also
a match on "Build http" - then the job will not be started.

(optional) Per-project triggers
===============================

In addition to triggers defined in the main configuration file, s5ci supports having several
additional files specifying triggers - one (or in the future more) per project.
They can be located in a different location, specified in the main configuration file,
thus allowing to separate the high-level trigger configuration from the "main" and host-specific config.


(optional) Time-based redundancy for Comment triggers
=====================================================

Let's say we have two servers, S1 and S2, both running s5CI, polling gerrit.
The parameter *poll_wait_ms* determines how long to wait between the subsequent polls.

S1 server should have a shorter interval, say 60000ms, the other one should have a longer interval,
say 300000ms - thus, 1 and 5 minutes respectively. Assuming the trigger configuration above is
present in both of the configurations, if S2 performs a poll, and sees the comment
matching the trigger regex, which is more than just over a minute old, but no corresponding
"Build ..." comment on the changeset - this means the S1 is in trouble, and did not launch
the job, so it can do so. In order to add this reaction delay, S2 needs to have
*default_regex_trigger_delay_sec* set to something slightly above 60 - 120 would be a safe setting.

When S2 acts on trigger and puts the comment, even if S1 somehow managed to not poll for a longer
period of time, then it will see both the trigger comment and the reaction from S2 - so it will
not trigger the job.

(optional, draft) Running in a stateless container
==================================================

s5ci saves the per-job data exports of job records within each of the job directory, done
at the same time as the HTML updates. This provides a distributed backup of sorts, and enables
simple database schema migrations - delete the old database, create the new database, re-import
the existing jobs records from exported text files.

This allows for a largely stateless container-based setup in the following fashion:
create a remote rsync-available host, whose only purpose in life is to be accessible via HTTP for
the outside world and via rsync/ssh to the container infra, and upon container startup,
rsync the static data tree to it, then recreate the database. Run the jobs as usual,
and perform frequent periodic rsync back to the static storage. Upon the s5ci shutdown,
wait for the rsync to complete and just delete the container.

The UI with the results will continue to be available at the static location.

(optional, draft) Distributed jobs under hashicorp nomad
==========================================================

The distributed case is an extension of the above mentioned container setup, except rather
than starting the new "s5ci run-job" processes within the current container, it kicks them
off in the separate nomad jobs. 

The process hierarchy in case of the local job looks like:

```
mainhost:

s5ci # main loop
  |---> s5ci run-job foo
          |-----> bash -c /path/to/scripts/foo
```

For the remote job we need to abstract out the existence of the level inbetween, so the 
process/job hierarchy will look as follows:

```
mainhost:

s5ci # main loop
  |---> s5ci run-job foo
           . . . . . launch and monitor nomad job . . . 

subhost:

CALLBACK=http://mainhost/s5ci s5ci run-job foo 
  |---> bash -c /path/to/scripts/foo

```

the run-job instance of s5ci on the mainhost periodically polls the nomad control plane,
and pumps the stdout/stderr info that is being added there, to the console output on the mainhost
s5ci instance - this way it creates an abstraction that the job is running on the mainhost.

The s5ci instance on the subhost does the part of monitoring the local descendant processes,
communicating back to the mainhost: new job start requests (which turn into separate nomad jobs
with the same mainhost-subhost structure), job status updates, job termination and return code
values. The HTTP(s) callback is secured by a temporary random token that is valid for the duration of
that job, optionally (TBD) also by an IP address restriction that gets activated upon the very first
callback - the assumption is that the live-running host will not change the IP. The current
mechanism explicitly does *not* consider any MITM threats. For that one might use TLS with
certificate pinning (assuming that the subhost cert/private key distribution is secure
against MITM - else it is just complexity with not much value.

With this approach, none of the concepts behind running as a standalone node need to change,
and (assuming a good implementation), nomad will provide the required tolerance for restarting
and moving the failing jobs around in case of failing nodes.


