package main

// package commands

import (
	"fmt"
)

type RegenerateHtmlCommand struct{}

func (command *RegenerateHtmlCommand) Execute(args []string) error {
	fmt.Println("Regenerating html for all jobs")
	RegenerateAllHtml()
	return nil
}
