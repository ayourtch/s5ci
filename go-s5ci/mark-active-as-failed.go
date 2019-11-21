package main

// package commands

import (
	"fmt"
)

type MarkActiveAsFailedCommand struct{}

func (command *MarkActiveAsFailedCommand) Execute(args []string) error {
	fmt.Println("Marking all currently active jobs as failed")
	jobs := DbGetActiveJobs()
	db := DbOpen()
	defer DbClose(&db)
	for i, job := range jobs {
		now := S5Now()
		fmt.Println(i, " = ", job)
		job.Updated_At = &now
		job.Finished_At = &now
		job.Return_Success = false
		retcode := 7777
		job.Return_Code = &retcode
		DbSaveJob(&db, &job)
		RegenerateJobHtml(job.Job_ID)
	}
	RegenerateAllHtml()
	return nil
}
