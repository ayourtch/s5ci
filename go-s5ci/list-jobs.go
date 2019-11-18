package main

// package commands

import (
	"bufio"
	"fmt"
	"os"
	"strings"
)

type ListJobsCommand struct {
	TimeHorizonSec    int     `short:"t" long:"time-horizon" description:"Time horizon for stopped jobs, seconds"`
	RsyncFileListName string  `short:"r" long:"rsync-file-list" description:"File list for rsync"`
	EqualsHostname    *string `short:"e" long:"equals-hostname" description:"Check hostname being equal to this"`
	NotEqualsHostname *string `short:"n" long:"not-equals-hostname" description:"Check hostname NOT being equal to this"`
	UpdateRootDir     *string `short:"u" long:"update-from-root" description:"Update the specified jobs from a job root"`
	DrainMode         bool    `short:"d" long:"drain-mode" description:"make list suitable for drain mode"`
}

func (command *ListJobsCommand) Execute(args []string) error {
	//	var ErrShowHelpMessage = errors.New("list jobs command invoked")
	jobs := DbGetAllJobs()
	w := bufio.NewWriter(os.Stdout)
	rsync_output := false
	if command.RsyncFileListName != "" {
		rsync_output = true
		f, err := os.Create(command.RsyncFileListName)
		if err != nil {
			panic(err)
		}
		w = bufio.NewWriter(f)
	}
	job_group_seen := make(map[string]bool)
	ts_now := UnixTimeNow()
	if rsync_output {
		fmt.Fprintf(w, "include jobs\n")
		fmt.Fprintf(w, "exclude workspace\n")
	}
	for i, job := range jobs {
		if job.Finished_At != nil {
			if job.Finished_At.UnixTimestamp() < ts_now-command.TimeHorizonSec {
				continue
			}
		}
		if command.EqualsHostname != nil {
			if job.Remote_Host == nil {
				continue
			}
			if *command.EqualsHostname != *job.Remote_Host {
				continue
			}
		}
		if command.NotEqualsHostname != nil {
			if job.Remote_Host == nil {
				continue
			}
			if *command.NotEqualsHostname == *job.Remote_Host {
				continue
			}
		}
		if rsync_output {
			if !job_group_seen[job.Job_Group_Name] {
				fmt.Fprintf(w, "include %s\n", job.Job_Group_Name)
				fmt.Fprintf(w, "include %s/*\n", job.Job_Group_Name)
				job_group_seen[job.Job_Group_Name] = true
			}
			split_nr := JobSplitJobNR(job.Instance_ID)
			path_parts := strings.SplitN(split_nr, "/", 5)
			accum := ""
			for _, part := range path_parts {
				accum = fmt.Sprintf("%s/%s", accum, part)
				full_name := fmt.Sprintf("%s%s", job.Job_Group_Name, accum)
				if !job_group_seen[full_name] {
					fmt.Fprintf(w, "include %s\n", full_name)
					fmt.Fprintf(w, "include %s/*\n", full_name)
					job_group_seen[full_name] = true
				}
			}
			fmt.Fprintf(w, "include %s/%s/**\n", job.Job_Group_Name, JobSplitJobNR(job.Instance_ID))
		} else if command.UpdateRootDir != nil {
			Db_restore_job_from(*command.UpdateRootDir, job.Job_Group_Name, fmt.Sprintf("%d", job.Instance_ID))
			RegenerateJobHtml(job.Job_ID)
		} else {
			fmt.Printf("job %d: %s\n", i, job)
			YamlDump(job)
		}
	}
	if rsync_output {
		if !command.DrainMode {
			fmt.Fprintf(w, "include /index*.html\n")
			fmt.Fprintf(w, "include jobs/index*.html\n")
			fmt.Fprintf(w, "include jobs/active*.html\n")
			fmt.Fprintf(w, "include heartbeat.json\n")
		}
		fmt.Fprintf(w, "exclude *\n")
	}
	w.Flush()
	return nil
	//return ErrShowHelpMessage
}
