package main

import (
	"fmt"
	"log"
	"os"
	"syscall"
)

func GetFileMtimestamp(filename string) int {
	file, err := os.Stat(filename)

	if err != nil {
		fmt.Println("error while doing stat on file ", filename, ":", err)
		return 0
	}

	modifiedtime := int(file.ModTime().Unix())
	return modifiedtime
}

func FileChangedSince(filename string, since int) bool {
	mtime := GetFileMtimestamp(filename)
	delta := 10
	if mtime > 0 && since > 0 {
		if mtime-since > delta {
			return true
		}
	}
	return false
}

type AutorestartState struct {
	ConfigMtime int
	ExeMtime    int
}

func AutorestartInit(c *S5ciConfig, rtdt *S5ciRuntimeData) AutorestartState {
	// log.Printf("exe: %s config: %s", rtdt.RealS5ciExe, rtdt.ConfigPath)
	config_mtime := GetFileMtimestamp(rtdt.ConfigPath)
	exe_mtime := GetFileMtimestamp(rtdt.RealS5ciExe)
	return AutorestartState{ConfigMtime: config_mtime, ExeMtime: exe_mtime}
}

func AutorestartCheck(c *S5ciConfig, rtdt *S5ciRuntimeData, ars *AutorestartState) {
	if c.Autorestart.On_Config_Change &&
		FileChangedSince(rtdt.ConfigPath, ars.ConfigMtime) {
		log.Printf("Config changed, attempt restart")
		S5ciEvent("autorestart-config")
		RestartOurselves(c, rtdt)
	}

	if c.Autorestart.On_Exe_Change && FileChangedSince(rtdt.RealS5ciExe, ars.ExeMtime) {
		log.Printf("Executable changed, attempt restart")
		S5ciEvent("autorestart-exe")
		RestartOurselves(c, rtdt)
	}
}

func RestartOurselves(c *S5ciConfig, rtdt *S5ciRuntimeData) {
	argv0 := rtdt.RealS5ciExe
	argv := append([]string{}, os.Args...)
	new_env := append([]string{}, os.Environ()...)
	syscall.Exec(argv0, argv, new_env)
}
