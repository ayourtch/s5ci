package main

import (
	"fmt"
	mustache "github.com/hoisie/mustache"
	"github.com/robfig/cron"
	"log"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

type CommentTriggerRegex struct {
	R         *regexp.Regexp
	RSuppress *regexp.Regexp
	Name      string
}

type CronTriggerSchedule struct {
	Name     string
	Schedule cron.Schedule
	LastRun  int
}

/* a collection of global runtime state */

type S5ciRuntimeData struct {
	ConfigPath              string
	Hostname                string
	SandboxLevel            int
	PatchsetExtractRegex    *regexp.Regexp
	UnsafeCharRegex         *regexp.Regexp
	UnsafeStartRegex        *regexp.Regexp
	TriggerRegexes          []CommentTriggerRegex
	TriggerCommandTemplates map[string]*mustache.Template
	CronTriggerSchedules    []CronTriggerSchedule
	// action -- not needed
	ChangesetID    int
	PatchsetID     int
	TriggerEventID string
	CommentValue   string
	RealS5ciExe    string
}

func getTriggerRegexes(c *S5ciConfig) []CommentTriggerRegex {
	out := make([]CommentTriggerRegex, 0)
	for name, trig := range c.Comment_Triggers {
		r := regexp.MustCompile(trig.Regex)
		var r_suppress *regexp.Regexp = nil
		if trig.Suppress_Regex != nil {
			r_suppress = regexp.MustCompile(*trig.Suppress_Regex)
		}
		reg := CommentTriggerRegex{R: r, RSuppress: r_suppress, Name: name}
		out = append(out, reg)
	}
	return out
}

func getCronTriggerSchedules(c *S5ciConfig) []CronTriggerSchedule {
	out := []CronTriggerSchedule{}
	cron_parser := cron.NewParser(cron.Second | cron.Minute | cron.Hour | cron.Dom | cron.Month | cron.Dow)
	now_ts := UnixTimeNow()

	for cname, tr := range c.Cron_Triggers {
		sch, err := cron_parser.Parse(tr.Cron)
		if err != nil {
			s := regexp.MustCompile(`\s+`).Split(tr.Cron, -1)
			no_year_cron := strings.Join(s[0:6], " ")
			sch, err = cron_parser.Parse(no_year_cron)
			if err != nil {
				panic(err)
			}
		}
		// log.Println(i, sch)
		cs := CronTriggerSchedule{Schedule: sch, Name: cname, LastRun: now_ts}
		if os.Getenv("DEBUG_S5CI_CONFIG") != "" {
			YamlDump(cs)
		}
		out = append(out, cs)
	}
	return out
}

func getTriggerCommandTemplates(c *S5ciConfig) map[string]*mustache.Template {
	out := make(map[string]*mustache.Template)
	for name, trig := range c.Comment_Triggers {
		src_str := ""
		if trig.Action.Command != nil {
			src_str = *trig.Action.Command
		}
		template, err := mustache.ParseString(src_str)
		if err != nil {
			log.Fatalf("Error parsing '%s': %v", err)
		}
		out[name] = template
	}
	return out
}

var S5ciRuntime S5ciRuntimeData

func InitRuntimeData() {
	rtdt := &S5ciRuntime
	hostname, err := os.Hostname()
	if err != nil {
		fmt.Println("Can not get hostname")
		panic(err)
	}
	rtdt.Hostname = hostname
	c := &S5ciOptions.Config

	rtdt.ConfigPath = S5ciConfigPath
	rtdt.SandboxLevel = c.Sandbox_Level
	if rtdt.SandboxLevel == 0 {
		rtdt.SandboxLevel = S5ciOptions.SandboxLevel
	}
	rtdt.PatchsetExtractRegex = regexp.MustCompile(c.Patchset_Extract_Regex)
	rtdt.UnsafeCharRegex = regexp.MustCompile(`([^-/_A-Za-z0-9])`)
	rtdt.UnsafeStartRegex = regexp.MustCompile(`([^_A-Za-z0-9])`)
	rtdt.TriggerRegexes = getTriggerRegexes(c)
	rtdt.TriggerCommandTemplates = getTriggerCommandTemplates(c)
	rtdt.CronTriggerSchedules = getCronTriggerSchedules(c)
	rtdt.ChangesetID = -1
	rtdt.PatchsetID = -1
	rtdt.TriggerEventID = "no_trigger_event"
	rtdt.CommentValue = ""
	exe_name, err := filepath.Abs(os.Args[0])
	if err != nil {
		panic(err)
	}
	rtdt.RealS5ciExe = exe_name

}
