package main

import (
//"fmt"
)

type SetStatusCommand struct {
	JobID   string `short:"j" long:"job-id" description:"job ID (group_name/number)" required:"true"`
	Message string `short:"m" long:"message" description:"status message" required:"true"`
}

func (command *SetStatusCommand) Execute(args []string) error {
	job, err := DbGetJob(command.JobID)
	if err != nil {
		panic(err)
	}
	db := DbOpen()
	defer DbClose(&db)
	s5now := S5Now()
	job.Status_Message = command.Message
	job.Status_Updated_At = &s5now
	job.Updated_At = &s5now
	DbSaveJob(&db, job)
	RegenerateJobHtml(command.JobID)
	return nil
}
