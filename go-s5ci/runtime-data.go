package main

import (
	mustache "github.com/hoisie/mustache"
	"log"
	"regexp"
)

type CommentTriggerRegex struct {
	R         *regexp.Regexp
	RSuppress *regexp.Regexp
	Name      string
}

type CronTriggerSchedule struct {
}

/* a collection of global runtime state */

type S5ciRuntimeData struct {
	ConfigPath              string
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
	// rtdt.CronTriggerSchedules =
	rtdt.ChangesetID = -1
	rtdt.PatchsetID = -1
	rtdt.TriggerEventID = "no_trigger_event"
	rtdt.CommentValue = ""
	// RealS5ciExe = ""

}
