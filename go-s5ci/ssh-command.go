package main

import (
	"log"
	"os/exec"
)

func RunSshCommand(c *S5ciConfig, command string) (string, error) {
	log.Printf("Running SSH command: %s", command)
	return RunSshCommandShell(c, command)

}

func RunSshCommandShell(c *S5ciConfig, command string) (string, error) {

	args := make([]string, 0)
	if len(c.Server.Poll_Type.Shell.Args) == 0 {
		panic("Check config - no ssh args ?")
	}
	for _, arg := range c.Server.Poll_Type.Shell.Args {
		args = append(args, arg)
	}
	args = append(args, command)

	proc := exec.Command("/usr/bin/ssh", args...)
	out, err := proc.CombinedOutput()
	if err != nil {
		log.Println(string(out))
		log.Printf("RunSshCommandShell error: %v", err)
		return string(out), err
	}

	return string(out), nil
}
