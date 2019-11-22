package main

// package commands

import (
	"bufio"
	"encoding/json"
	"fmt"
	"gopkg.in/yaml.v2"
	"io/ioutil"
	"log"
	"os"
	"path/filepath"
	"strings"
)

type ListJobsCommand struct {
	TimeHorizonSec    int     `short:"t" long:"time-horizon" description:"Time horizon for stopped jobs, seconds"`
	RsyncFileListName string  `short:"r" long:"rsync-file-list" description:"File list for rsync"`
	JsonJobListName   string  `short:"j" long:"json-job-list" description:"list of job IDs in json"`
	YamlJobListName   string  `short:"y" long:"yaml-job-list" description:"list of job IDs in yaml"`
	EqualsHostname    *string `short:"e" long:"equals-hostname" description:"Check hostname being equal to this"`
	NotEqualsHostname *string `short:"n" long:"not-equals-hostname" description:"Check hostname NOT being equal to this"`
	UpdateRootDir     *string `short:"u" long:"update-from-root" description:"Update the specified jobs from a job root"`
	DrainMode         bool    `short:"d" long:"drain-mode" description:"make list suitable for drain mode"`
}

func (command *ListJobsCommand) Execute(args []string) error {
	//	var ErrShowHelpMessage = errors.New("list jobs command invoked")
	rtdt := &S5ciRuntime
	jobs := DbGetAllJobs()
	w := bufio.NewWriter(os.Stdout)
	rsync_output := false
	json_output := false
	yaml_output := false
	precise_job_update_list := false
	// if the update root directory was specified, the json list is the *input*
	if command.JsonJobListName != "" && command.UpdateRootDir == nil {
		json_output = true
	}
	if command.YamlJobListName != "" && command.UpdateRootDir == nil {
		yaml_output = true
	}
	if (command.JsonJobListName != "" || command.YamlJobListName != "") && command.UpdateRootDir != nil {
		precise_job_update_list = true
	}
	updated_jobs_list := make([]string, 0)
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
		fmt.Fprintf(w, "include jobs/db\n")
		fmt.Fprintf(w, "include jobs/updatedb\n")
		fmt.Fprintf(w, "include jobs/updatedb/%s\n", rtdt.Hostname)
		fmt.Fprintf(w, "include jobs/updatedb/%s/heartbeat.json\n", rtdt.Hostname)
		fmt.Fprintf(w, "include jobs/updatedb/%s/rsync-filter.txt\n", rtdt.Hostname)
		fmt.Fprintf(w, "include jobs/updatedb/%s/updated-jobs.json\n", rtdt.Hostname)
		fmt.Fprintf(w, "include jobs/updatedb/%s/updated-jobs.yaml\n", rtdt.Hostname)
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
		if json_output {
			updated_jobs_list = append(updated_jobs_list, job.Job_ID)
		}
		if rsync_output {
			if !job_group_seen[job.Job_Group_Name] {
				fmt.Fprintf(w, "include %s\n", job.Job_Group_Name)
				fmt.Fprintf(w, "include %s/*\n", job.Job_Group_Name)
				// sync the db files as well
				fmt.Fprintf(w, "include db/%s\n", job.Job_Group_Name)
				fmt.Fprintf(w, "include db/%s/*\n", job.Job_Group_Name)
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
					// sync the db files as well
					fmt.Fprintf(w, "include db/%s\n", full_name)
					fmt.Fprintf(w, "include db/%s/*\n", full_name)
					job_group_seen[full_name] = true
				}
			}
			fmt.Fprintf(w, "include %s/%s/**\n", job.Job_Group_Name, JobSplitJobNR(job.Instance_ID))
			// sync the db files as well
			fmt.Fprintf(w, "include db/%s/%s/**\n", job.Job_Group_Name, JobSplitJobNR(job.Instance_ID))
		} else if command.UpdateRootDir != nil {
			if !precise_job_update_list {
				Db_restore_job_from(filepath.Join(*command.UpdateRootDir, job.Job_ID))
				RegenerateJobHtml(job.Job_ID)
			}
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
	if json_output {
		d, err := json.MarshalIndent(updated_jobs_list, "", "  ")
		if err != nil {
			log.Fatalf("error: %v", err)
		}
		writeToFile(command.JsonJobListName, string(d))
	}
	if yaml_output {
		d, err := yaml.Marshal(updated_jobs_list)
		if err != nil {
			log.Fatalf("error: %v", err)
		}
		writeToFile(command.YamlJobListName, string(d))
	}
	w.Flush()
	if precise_job_update_list {
		jobs_list := make([]string, 0)
		if command.JsonJobListName != "" {
			data, err := ioutil.ReadFile(command.JsonJobListName)
			if err != nil {
				log.Fatalf("error: %v", err)
			}

			err = json.Unmarshal([]byte(data), &jobs_list)
			if err != nil {
				log.Fatalf("error: %v", err)
			}
		}
		if command.YamlJobListName != "" {
			data, err := ioutil.ReadFile(command.YamlJobListName)
			if err != nil {
				log.Fatalf("error: %v", err)
			}

			err = yaml.Unmarshal([]byte(data), &jobs_list)
			if err != nil {
				log.Fatalf("error: %v", err)
			}
		}
		for _, job_id := range jobs_list {
			fmt.Printf("Restoring/updating job %s\n", job_id)
			Db_restore_job_from(filepath.Join(*command.UpdateRootDir, job_id))
			RegenerateJobHtml(job_id)
		}

	}
	return nil
	//return ErrShowHelpMessage
}
