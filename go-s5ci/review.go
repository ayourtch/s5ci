package main

// package commands

import (
	"errors"
	"fmt"
)

type ReviewCommand struct{}

func (command *ReviewCommand) Execute(args []string) error {
	var ErrShowHelpMessage = errors.New("list jobs command invoked")
	fmt.Println("Test")
	return ErrShowHelpMessage
}
