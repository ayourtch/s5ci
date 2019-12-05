package main

import (
	"fmt"
	"log"
)

func GerritQueryChanges(c *S5ciConfig, before_ts *int, after_ts *int) (string, error) {
	// log.Printf("Gerrit Query Changes")
	// RunSshCommand
	date_str := ""
	if before_ts != nil {
		before := S5TimeFromTimestamp(*before_ts)
		if after_ts != nil {
			after := S5TimeFromTimestamp(*after_ts)
			date_str = fmt.Sprintf(`(before: \"%s\" OR after: \"%s\")`, before, after)
		} else {
			date_str = fmt.Sprintf(`(before: \"%s\")`, before)
		}

	} else {
		if after_ts != nil {
			after := S5TimeFromTimestamp(*after_ts)
			date_str = fmt.Sprintf(`(after: \"%s\")`, after)
		}
	}

	// date_str = "" // XXXXX
	log.Printf("DATE query: %s", date_str)
	q := &c.Default_Query
	cmd := fmt.Sprintf("gerrit query %s %s --format JSON %s", q.Filter, date_str, q.Options)
	return RunSshCommand(c, cmd)
}

func PollGerritOverSsh(c *S5ciConfig, rtdt *S5ciRuntimeData, before_ts *int, after_ts *int) (*S5SshResult, error) {
	output, err := GerritQueryChanges(c, before_ts, after_ts)
	if err != nil {
		log.Printf("PollGerritOverSsh - error from GerritQueryChanges: %v", err)
		return nil, err
	}
	now_ts := UnixTimeNow()
	fname := fmt.Sprintf("/tmp/s5ci-gerrit-%d.json", now_ts)
	log.Printf("Saving the gerrit output to %s", fname)
	writeToFile(fname, output)
	res, err := ParseGerritPollCommandReply(c, rtdt, before_ts, after_ts, output)
	if err != nil {
		log.Printf("PollGerritOverSsh - error from ParseGerritPollCommandReply: %v", err)
		return nil, err
	}
	return res, err
}
