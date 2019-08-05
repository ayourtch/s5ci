package main

// package commands

import (
	"errors"
	"fmt"
)

type SetStatusCommand struct{}

func (command *SetStatusCommand) Execute(args []string) error {
	var ErrShowHelpMessage = errors.New("list jobs command invoked")
	fmt.Println("Test")
	return ErrShowHelpMessage
}
