package main

// package commands

import (
	"errors"
	"fmt"
)

type ReviewCommand struct {
	ChangesetID int     `short:"s" long:"changeset-id" env:"S5CI_GERRIT_CHANGESET_ID" description:"changeset ID" required:"true"`
	Message     string  `short:"m" long:"message" description:"message to add in a review" required:"true"`
	PatchsetID  int     `short:"p" long:"patchset-id" env:"S5CI_GERRIT_PATCHSET_ID" description:"patchset ID"`
	Vote        *string `short:"v" long:"vote" description:"vote success, failure or clear"`
}

func (command *ReviewCommand) Execute(args []string) error {
	c := &S5ciOptions.Config
	rtdt := &S5ciRuntime

	vote_command_frag := ""

	if command.Vote != nil {
		switch *command.Vote {
		case "success":
			vote_command_frag = c.Default_Vote.Success
			break
		case "failure":
			vote_command_frag = c.Default_Vote.Failure
			break
		case "clear":
			vote_command_frag = c.Default_Vote.Clear
			break
		default:
			var ErrShowHelpMessage = errors.New("expecting 'success', 'failure' or 'clear'")
			return ErrShowHelpMessage
		}
		if vote_command_frag != "" && rtdt.SandboxLevel > 1 {
			fmt.Println("Sandbox level ", rtdt.SandboxLevel, " ignoring voting arg ", vote_command_frag)
			vote_command_frag = ""
		}
	}

	ssh_command := fmt.Sprintf(`gerrit review %d %s --message "%s"`, command.ChangesetID, vote_command_frag, command.Message)
	if command.PatchsetID > 0 {
		ssh_command = fmt.Sprintf(`gerrit review %d,%d %s --message "%s"`, command.ChangesetID, command.PatchsetID, vote_command_frag, command.Message)

	}
	if rtdt.SandboxLevel > 0 {
		fmt.Println("Sandbox level", rtdt.SandboxLevel, ", not running command ", ssh_command)
	} else {
		RunSshCommand(c, ssh_command)
	}

	return nil
}
