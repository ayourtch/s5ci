package main

// package commands

import (
// "errors"
// "fmt"
)

type GerritCommandCommand struct {
	Command string `short:"c" long:"command" required:"true" description:"command to run"`
}

func (command *GerritCommandCommand) Execute(args []string) error {
	c := &S5ciOptions.Config
	// rtdt := &S5ciRuntime

	// var ErrShowHelpMessage = errors.New("run gerrit command")
	RunSshCommand(c, command.Command)
	return nil // ErrShowHelpMessage
}
