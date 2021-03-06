package main

import (
	"fmt"
	"github.com/erikdubbelboer/gspt"
	"github.com/jessevdk/go-flags"
	"github.com/vito/twentythousandtonnesofcrudeoil"
	"log"
	"os"
	"io"
	"path/filepath"
	"runtime"
	"runtime/pprof"
	"strings"
)

var opts struct {
	Verbose []bool `short:"v" long:"verbose" description:"Show verbose debug information"`
}

type S5ciCommand struct {
	Config        S5ciConfig
	Version       func()         `short:"v" long:"version" description:"Print the version of S5ci and exit"`
	ConfigFile    func(string)   `short:"c" long:"config" description:"configuration file" required:"true"`
	SandboxLevel  int            `short:"s" long:"sandbox-level" description:"sandbox level"`
	CpuProfile    *string        `long:"memory-profile" description:"Write memory profile to a file"`
	MemoryProfile *string        `long:"cpu-profile" description:"Write CPU profile to a file"`
	ProfileUsePid bool           `long:"profile-file-pid" description:"append the current pid to the profile files"`
	DevTest       DevTestCommand `command:"dev-test"     description:"tests."`
	DbMaint       DbMaintCommand `command:"database" description:"database maintenance"`

	CheckConfig        CheckConfigCommand        `command:"check-config" description:"check config, return 0 if ok"`
	GerritCommand      GerritCommandCommand      `command:"gerrit-command" description:"run arbitrary command"`
	KillJob            KillJobCommand            `command:"kill-job" description:"kill a running job"`
	ListJobs           ListJobsCommand           `command:"list-jobs" description:"list jobs"`
	MarkActiveAsFailed MarkActiveAsFailedCommand `command:"mark-active-as-failed" description:"mark all active jobs as failed"`
	RegenerateHtml     RegenerateHtmlCommand     `command:"regenerate-html" description:"regenerate all of the html"`
	ProcessGerritReply ProcessGerritReplyCommand `command:"process-gerrit-reply" description:"process saved JSON reply from gerrit"`
	RebuildDatabase    RebuildDatabaseCommand    `command:"rebuild-database" description:"rebuild database from per-job yaml"`
	Review             ReviewCommand             `command:"review" description:"review with comment and maybe vote"`
	RunJob             RunJobCommand             `command:"run-job" description:"run a job"`
	SetStatus          SetStatusCommand          `command:"set-status" description:"set the job status to message"`
}

var S5ciOptions S5ciCommand
var S5ciConfigPath string

func S5ciPrepareToRun() {
	c := &S5ciOptions.Config
	if c.Per_Project_Config != nil {
		for _, project_name := range c.Per_Project_Config.Projects {
			fname := filepath.Join(c.Per_Project_Config.Root_Dir, fmt.Sprintf("%s.yaml", project_name))
			// log.Printf("Reading per-project config from %s", fname)
			ppc, err := LoadS5ciPerProjectConfig(fname)
			if err != nil {
				log.Fatalf("error: %v", err)
			}
			if c.Comment_Triggers == nil {
				c.Comment_Triggers = make(map[string]S5CommentTrigger)
			}
			if c.Cron_Triggers == nil {
				c.Cron_Triggers = make(map[string]S5CronTrigger)
			}
			for trig_name, trig := range ppc.Comment_Triggers {
				global_trig_name := fmt.Sprintf("%s_%s", project_name, trig_name)
				if trig.Project == nil {
					trig.Project = &project_name
				}
				c.Comment_Triggers[global_trig_name] = trig
			}
			for trig_name, trig := range ppc.Cron_Triggers {
				global_trig_name := fmt.Sprintf("%s_%s", project_name, trig_name)
				c.Cron_Triggers[global_trig_name] = trig
			}
		}
	}

	if os.Getenv("DEBUG_S5CI_CONFIG") != "" {
		YamlDump(c)
	}
	InitRuntimeData()
}

func S5ciCommandHandler(command flags.Commander, args []string) error {
	S5ciPrepareToRun()
	os.Setenv("S5CI_JOB_ID", os.Getenv("X_S5CI_JOB_ID"))
	os.Setenv("S5CI_JOB_URL", os.Getenv("X_S5CI_JOB_URL"))
	os.Setenv("S5CI_JOB_NAME", os.Getenv("X_S5CI_JOB_NAME"))
	os.Setenv("S5CI_PARENT_JOB_ID", os.Getenv("X_S5CI_PARENT_JOB_ID"))
	os.Setenv("S5CI_PARENT_JOB_URL", os.Getenv("X_S5CI_PARENT_JOB_URL"))
	os.Setenv("S5CI_PARENT_JOB_NAME", os.Getenv("X_S5CI_PARENT_JOB_NAME"))
	reterr := command.Execute(args)
	return reterr
}

func main() {
	var Version = "0.1"
	gspt.SetProcTitle(strings.Join(os.Args, " "))

	// Setup the logger
	fd, err := s5LogCreateFile("./logs/s5ci.log")
	if fd != nil {
		defer fd.Close()
		fileAndStdout := io.MultiWriter(fd, os.Stdout)
		fileAndStderr := io.MultiWriter(fd, os.Stderr)

		// Trace will go only to the file, Info, Warning and Error
		// will go to the Stdout and Stderr
		s5LogInit(fd, fileAndStdout, fileAndStdout, fileAndStderr)
	} else {
		s5LogInit(os.Stdout, os.Stdout, os.Stdout, os.Stderr)
	}

	Trace.Printf("============ s5ci New Execution ============\n")

	S5ciOptions.Version = func() {
		fmt.Println(Version)
		os.Exit(0)
	}
	Info.Printf("s5ci Version: %s\n", Version)

	S5ciOptions.ConfigFile = func(fname string) {
		config_path, err := filepath.Abs(fname)
		if err != nil {
			log.Fatal(err)
		}
		S5ciConfigPath = config_path
		S5ciOptions.Config, _ = LoadS5ciConfig(fname)
	}

	parser := flags.NewParser(&S5ciOptions, flags.HelpFlag|flags.PassDoubleDash)
	parser.NamespaceDelimiter = "-"
	parser.CommandHandler = S5ciCommandHandler

	os.Setenv("X_S5CI_JOB_ID", os.Getenv("S5CI_JOB_ID"))
	os.Setenv("X_S5CI_JOB_URL", os.Getenv("S5CI_JOB_URL"))
	os.Setenv("X_S5CI_JOB_NAME", os.Getenv("S5CI_JOB_NAME"))
	os.Setenv("X_S5CI_PARENT_JOB_ID", os.Getenv("S5CI_PARENT_JOB_ID"))
	os.Setenv("X_S5CI_PARENT_JOB_URL", os.Getenv("S5CI_PARENT_JOB_URL"))
	os.Setenv("X_S5CI_PARENT_JOB_NAME", os.Getenv("S5CI_PARENT_JOB_NAME"))
	twentythousandtonnesofcrudeoil.TheEnvironmentIsPerfectlySafe(parser, "S5CI_")

	_, err = parser.Parse()

	if S5ciOptions.CpuProfile != nil {
		if S5ciOptions.ProfileUsePid {
			name := fmt.Sprintf("%s.%d", *S5ciOptions.CpuProfile, os.Getpid())
			S5ciOptions.CpuProfile = &name
		}

		f, err := os.Create(*S5ciOptions.CpuProfile)
		if err != nil {
			log.Fatal("could not create CPU profile: ", err)
		}
		defer f.Close()
		if err := pprof.StartCPUProfile(f); err != nil {
			log.Fatal("could not start CPU profile: ", err)
		}
		defer pprof.StopCPUProfile()
	}

	if err != nil {
		if e, ok := err.(*flags.Error); ok {
			if e.Type == flags.ErrCommandRequired {
				/* If no command, go to the poll loop */
				S5ciPrepareToRun()
				PollLoop()
			} else {
				fmt.Fprintf(os.Stderr, "Usage error: %v\n", err)
				os.Exit(1)
			}
		} else {
			fmt.Fprintf(os.Stderr, "Usage error: %v\n", err)
			os.Exit(1)
		}
	}
	if S5ciOptions.MemoryProfile != nil {
		if S5ciOptions.ProfileUsePid {
			name := fmt.Sprintf("%s.%d", *S5ciOptions.MemoryProfile, os.Getpid())
			S5ciOptions.MemoryProfile = &name
		}
		f, err := os.Create(*S5ciOptions.MemoryProfile)
		if err != nil {
			log.Fatal("could not create memory profile: ", err)
		}
		defer f.Close()
		runtime.GC() // get up-to-date statistics
		if err := pprof.WriteHeapProfile(f); err != nil {
			log.Fatal("could not write memory profile: ", err)
		}
	}

	if false {
		_, err := flags.Parse(&opts)

		if err != nil {
			panic(err)
		}
	}
	Trace.Printf("============ s5ci Exiting ============\n")
}
