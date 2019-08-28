package main

// package commands

import (
	"errors"
	"fmt"
	"gopkg.in/yaml.v2"
	"log"
)

type CheckConfigCommand struct{}

func (command *CheckConfigCommand) Execute(args []string) error {
	c := &S5ciOptions.Config
	fmt.Println("Checking config")
	d, err := yaml.Marshal(c)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	fmt.Println(string(d))

	var ErrShowHelpMessage = errors.New("check the configuration")
	if false {
		return ErrShowHelpMessage
	} else {
		return nil
	}
}
