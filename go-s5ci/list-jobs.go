package main

// package commands

import (
	"errors"
	"fmt"
)

type ListJobsCommand struct {
}

func (command *ListJobsCommand) Execute(args []string) error {
	var ErrShowHelpMessage = errors.New("list jobs command invoked")
	fmt.Println("Test")
	jobs := DbGetActiveJobs()
	for i, job := range jobs {
		fmt.Printf("job %d: %s\n", i, job)
	}
	return ErrShowHelpMessage
}
