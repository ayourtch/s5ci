package main

import (
	"fmt"
	"io/ioutil"
	"log"

	"gopkg.in/yaml.v2"
)

type s5ciJobsConfig struct {
	Rootdir  string
	Root_Url string
}

type S5ActionType struct {
	Command *string
}

type S5CommentTrigger struct {
	Project        *string
	Branch         *string
	Regex          string
	Suppress_Regex *string
	// Action         map[string]string
	Action S5ActionType
}

type S5CronTrigger struct {
	Cron   string
	Action S5ActionType
}

type S5ciPerProjectConfig struct {
	Comment_Triggers map[string]S5CommentTrigger
	Cron_Triggers    map[string]S5CronTrigger
}

type S5ciPerProjectConfigInfo struct {
	Root_Dir string
	Projects []string
}

type S5ciGerritQuery struct {
	Filter  string
	Options string
}

type S5ciGerritVote struct {
	Success string
	Failure string
	Clear   string
}

type S5ciShellPoll struct {
	Command string
	Args    []string
}

type S5ciPollType struct {
	Shell S5ciShellPoll
}

type S5ciServer struct {
	Poll_Type            S5ciPollType
	Sync_Horizon_Sec     *int
	Poll_Wait_Ms         *int
	Syncing_Poll_Wait_Ms *int
}

type S5ciAutorestart struct {
	On_Config_Change bool
	On_Exe_Change    bool
}

type S5ciConfig struct {
	Server                          S5ciServer
	Default_Query                   S5ciGerritQuery
	Default_Vote                    S5ciGerritVote
	Db_URL                          string
	Autorestart                     S5ciAutorestart
	Debug_Verbosity                 int
	Command_Rootdir                 string
	Install_Rootdir                 string
	Comment_Triggers                map[string]S5CommentTrigger
	Cron_Triggers                   map[string]S5CronTrigger
	Default_Regex_Trigger_Delay_Sec int
	Default_Sync_Horizon_Sec        int
	Sandbox_Level                   int
	Patchset_Extract_Regex          string
	Per_Project_Config              *S5ciPerProjectConfigInfo
	Jobs                            s5ciJobsConfig
}

func YamlDump(x interface{}) {
	d, err := yaml.Marshal(&x)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	fmt.Printf("--- t dump:\n%s\n\n", string(d))

}

func LoadS5ciPerProjectConfig(fname string) (S5ciPerProjectConfig, error) {
	t := S5ciPerProjectConfig{}
	data, err := ioutil.ReadFile(fname)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	err = yaml.Unmarshal([]byte(data), &t)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	return t, nil
}

func LoadS5ciConfig(fname string) (S5ciConfig, error) {
	t := S5ciConfig{}
	data, err := ioutil.ReadFile(fname)
	if err != nil {
		log.Fatalf("error: %v", err)
	}

	err = yaml.Unmarshal([]byte(data), &t)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	// fmt.Printf("--- t:\n%v\n\n", t)

	_, err = yaml.Marshal(&t)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	// fmt.Printf("--- t dump:\n%s\n\n", string(d))

	m := make(map[interface{}]interface{})

	err = yaml.Unmarshal([]byte(data), &m)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	// fmt.Printf("--- m:\n%v\n\n", m)

	_, err = yaml.Marshal(&m)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	// fmt.Printf("--- m dump:\n%s\n\n", string(d))
	return t, nil
}
