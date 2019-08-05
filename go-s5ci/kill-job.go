package main

// package commands

import (
	"errors"
	"fmt"
)

type KillJobCommand struct{}

func (command *KillJobCommand) Execute(args []string) error {
	var ErrShowHelpMessage = errors.New("Kill job")
	fmt.Println("Test")
	return ErrShowHelpMessage
}
