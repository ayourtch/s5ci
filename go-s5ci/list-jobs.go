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
	TimeHorizonSec       int     `short:"t" long:"time-horizon" description:"Time horizon for stopped jobs, seconds"`
	RsyncFileListName    string  `short:"r" long:"rsync-file-list" description:"File list for rsync"`
	IdxRsyncFileListName string  `short:"i" long:"index-rsync-file-list" description:"Index file list for rsync"`
	JsonJobListName      string  `short:"j" long:"json-job-list" description:"list of job IDs in json"`
	YamlJobListName      string  `short:"y" long:"yaml-job-list" description:"list of job IDs in yaml"`
	EqualsHostname       *string `short:"e" long:"equals-hostname" description:"Check hostname being equal to this"`
	NotEqualsHostname    *string `short:"n" long:"not-equals-hostname" description:"Check hostname NOT being equal to this"`
	UpdateRootDir        *string `short:"u" long:"update-from-root" description:"Update the specified jobs from a job root"`
	DbRsyncFileListName  string  `short:"d" long:"db-rsync-file-list" description:"database-only rsync filelist"`
}

func (command *ListJobsCommand) Execute(args []string) error {
	//	var ErrShowHelpMessage = errors.New("list jobs command invoked")
	rtdt := &S5ciRuntime
	jobs := DbGetAllJobs()
	rw := bufio.NewWriter(os.Stdout)
	dw := bufio.NewWriter(os.Stdout)
	iw := bufio.NewWriter(os.Stdout)
	rsync_output := false
	db_rsync_output := false
	idx_rsync_output := false
	json_output := false
	yaml_output := false
	precise_job_update_list := false
	BatchHtmlRegenerateStart()
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
		rw = bufio.NewWriter(f)
	}
	if command.DbRsyncFileListName != "" {
		db_rsync_output = true
		f, err := os.Create(command.DbRsyncFileListName)
		if err != nil {
			panic(err)
		}
		dw = bufio.NewWriter(f)
	}
	if command.IdxRsyncFileListName != "" {
		idx_rsync_output = true
		f, err := os.Create(command.IdxRsyncFileListName)
		if err != nil {
			panic(err)
		}
		iw = bufio.NewWriter(f)
	}
	job_group_seen := make(map[string]bool)
	db_job_group_seen := make(map[string]bool)
	idx_job_group_seen := make(map[string]bool)
	ts_now := UnixTimeNow()
	if rsync_output {
		fmt.Fprintf(rw, "include jobs\n")
		fmt.Fprintf(rw, "include jobs/db\n")
		fmt.Fprintf(rw, "include jobs/updatedb\n")
		fmt.Fprintf(rw, "include jobs/updatedb/%s\n", rtdt.Hostname)
		fmt.Fprintf(rw, "include jobs/updatedb/%s/heartbeat.json\n", rtdt.Hostname)
		fmt.Fprintf(rw, "include jobs/updatedb/%s/rsync-filter.txt\n", rtdt.Hostname)
		fmt.Fprintf(rw, "include jobs/updatedb/%s/rsync-db-filter.txt\n", rtdt.Hostname)
		fmt.Fprintf(rw, "include jobs/updatedb/%s/updated-jobs.json\n", rtdt.Hostname)
		fmt.Fprintf(rw, "include jobs/updatedb/%s/updated-jobs.yaml\n", rtdt.Hostname)
		fmt.Fprintf(rw, "exclude workspace\n")
	}
	if idx_rsync_output {
		fmt.Fprintf(iw, "include jobs\n")
		fmt.Fprintf(iw, "include /index*.html\n")
		fmt.Fprintf(iw, "include jobs/index*.html\n")
		fmt.Fprintf(iw, "include jobs/active*.html\n")
		fmt.Fprintf(iw, "include heartbeat.json\n")
	}
	for i, job := range jobs {
		if job.Finished_At != nil {
			if job.Finished_At.UnixTimestamp() < ts_now-command.TimeHorizonSec {
				continue
			}
		}
		/* index generation does not depend on equal/unequal hostnames - do it always */
		if idx_rsync_output {
			if !job_group_seen[job.Job_Group_Name] {
				fmt.Fprintf(iw, "include %s\n", job.Job_Group_Name)
				fmt.Fprintf(iw, "include %s/index*\n", job.Job_Group_Name)
				idx_job_group_seen[job.Job_Group_Name] = true
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
		if db_rsync_output {
			if !job_group_seen[job.Job_Group_Name] {
				fmt.Fprintf(dw, "include %s\n", job.Job_Group_Name)
				fmt.Fprintf(dw, "exclude %s/index*\n", job.Job_Group_Name)
				fmt.Fprintf(dw, "include %s/*\n", job.Job_Group_Name)
				db_job_group_seen[job.Job_Group_Name] = true
			}
			split_nr := JobSplitJobNR(job.Instance_ID)
			path_parts := strings.SplitN(split_nr, "/", 5)
			accum := ""
			for _, part := range path_parts {
				accum = fmt.Sprintf("%s/%s", accum, part)
				full_name := fmt.Sprintf("%s%s", job.Job_Group_Name, accum)
				if !db_job_group_seen[full_name] {
					fmt.Fprintf(dw, "include %s\n", full_name)
					fmt.Fprintf(dw, "include %s/*\n", full_name)
					db_job_group_seen[full_name] = true
				}
			}
			fmt.Fprintf(dw, "include %s/%s/**\n", job.Job_Group_Name, JobSplitJobNR(job.Instance_ID))
		}
		if rsync_output {
			if !job_group_seen[job.Job_Group_Name] {
				fmt.Fprintf(rw, "include %s\n", job.Job_Group_Name)
				fmt.Fprintf(rw, "exclude %s/index*\n", job.Job_Group_Name)
				fmt.Fprintf(rw, "include %s/*\n", job.Job_Group_Name)
				// sync the db files as well
				fmt.Fprintf(rw, "include db/%s\n", job.Job_Group_Name)
				fmt.Fprintf(rw, "include db/%s/*\n", job.Job_Group_Name)
				job_group_seen[job.Job_Group_Name] = true
			}
			split_nr := JobSplitJobNR(job.Instance_ID)
			path_parts := strings.SplitN(split_nr, "/", 5)
			accum := ""
			for _, part := range path_parts {
				accum = fmt.Sprintf("%s/%s", accum, part)
				full_name := fmt.Sprintf("%s%s", job.Job_Group_Name, accum)
				if !job_group_seen[full_name] {
					fmt.Fprintf(rw, "include %s\n", full_name)
					fmt.Fprintf(rw, "include %s/*\n", full_name)
					// sync the db files as well
					fmt.Fprintf(rw, "include db/%s\n", full_name)
					fmt.Fprintf(rw, "include db/%s/*\n", full_name)
					job_group_seen[full_name] = true
				}
			}
			fmt.Fprintf(rw, "include %s/%s/**\n", job.Job_Group_Name, JobSplitJobNR(job.Instance_ID))
			// sync the db files as well
			fmt.Fprintf(rw, "include db/%s/%s/**\n", job.Job_Group_Name, JobSplitJobNR(job.Instance_ID))
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
		fmt.Fprintf(rw, "include heartbeat.json\n")
		fmt.Fprintf(rw, "exclude *\n")
	}
	if db_rsync_output {
		fmt.Fprintf(dw, "exclude *\n")
	}
	if idx_rsync_output {
		fmt.Fprintf(iw, "exclude *\n")
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
	rw.Flush()
	dw.Flush()
	iw.Flush()
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
	BatchHtmlRegenerateFinish()
	return nil
	//return ErrShowHelpMessage
}
