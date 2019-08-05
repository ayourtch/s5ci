package main

// package commands

import (
	"errors"
	"fmt"
)

type CheckConfigCommand struct{}

func (command *CheckConfigCommand) Execute(args []string) error {
	var ErrShowHelpMessage = errors.New("check the configuration")
	fmt.Println("Test")
	return ErrShowHelpMessage
}
