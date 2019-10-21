package main

import (
	"fmt"
)

type RunJobCommand struct {
	Command        string `short:"c" long:"command" description:"command to run" required:"true"`
	Changeset_ID   int    `short:"s" long:"changeset-id" env:"S5CI_GERRIT_CHANGESET_ID" description:"changeset id" required:"true"`
	Patchset_ID    int    `short:"p" long:"patchset-id" env:"S5CI_GERRIT_PATCHSET_ID" description:"patchset id" required:"true"`
	TriggerEventID string `short:"t" long:"trigger-event-id" description:"Trigger event ID" required:"false"`
}

func (cmd *RunJobCommand) Execute(args []string) error {
	c := &S5ciOptions.Config
	rtdt := &S5ciRuntime
	fmt.Println("Command: ", cmd.Command)
	rtdt.ChangesetID = cmd.Changeset_ID
	rtdt.PatchsetID = cmd.Patchset_ID
	rtdt.TriggerEventID = cmd.TriggerEventID
	JobExecCommand(c, rtdt, cmd.Command)
	fmt.Println("done")
	return nil // ErrShowHelpMessage
}
