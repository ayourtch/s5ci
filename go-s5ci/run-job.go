package main

import (
	"fmt"
)

type RunJobCommand struct {
	Command      string `short:"c" long:"command" description:"command to run" required:"true"`
	Changeset_ID int    `short:"s" long:"changeset-id" description:"changeset id" required:"true"`
	Patchset_ID  int    `short:"p" long:"patchset-id" description:"patchset id" required:"true"`
}

func (cmd *RunJobCommand) Execute(args []string) error {
	c := &S5ciOptions.Config
	rtdt := &S5ciRuntime
	fmt.Println("Command: ", cmd.Command)
	rtdt.ChangesetID = cmd.Changeset_ID
	rtdt.PatchsetID = cmd.Patchset_ID
	JobSpawnCommand(c, rtdt, cmd.Command)
	fmt.Println("done")
	return nil // ErrShowHelpMessage
}
