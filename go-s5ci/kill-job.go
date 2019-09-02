package main

// package commands

import (
	"fmt"
	"syscall"
)

type KillJobCommand struct {
	JobID string `short:"j" long:"job-id" description:"job ID (group_name/number)" required:"true"`
}

func DoKillJob(job_id string, terminator string) {
	job, err := DbGetJob(job_id)
	if err != nil {
		panic(err)
	}
	db := DbOpen()
	defer DbClose(&db)
	if job.Command_Pid != nil {
		fmt.Printf("Requested to kill a job, sending signal to pid %d from job %s", job.Command_Pid, job_id)
		syscall.Kill(*job.Command_Pid, syscall.SIGTERM)
		if job.Finished_At == nil {
			s5now := S5Now()
			job.Status_Message = fmt.Sprintf("Terminated by the %s", terminator)
			job.Status_Updated_At = &s5now
			DbSaveJob(&db, job)
			RegenerateJobHtml(job_id)
		}
	}
}

func (command *KillJobCommand) Execute(args []string) error {
	DoKillJob(command.JobID, "S5CI CLI")
	return nil
}
