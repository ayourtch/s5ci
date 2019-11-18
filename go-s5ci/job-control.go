package main

import (
	"fmt"
	"crypto/sha1"
	"log"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"regexp"
	"strings"
	"syscall"
	"time"
)

func start_exec_fg_with_redir(cmdline string, out_fname string, new_cwd string, add_env []string) *exec.Cmd {
	proc := exec.Command("/bin/sh", "-c", cmdline)
	proc.SysProcAttr = &syscall.SysProcAttr{Setpgid: true, Pgid: 0}

	outfile, err := os.Create(out_fname)
	if err != nil {
		panic(err)
	}

	child_env := append([]string{}, os.Environ()...)
	child_env = append(child_env, add_env...)
	proc.Env = child_env
	if new_cwd != "" {
		proc.Dir = new_cwd
	}

	proc.Stdout = outfile
	proc.Stderr = outfile

	err = proc.Start()
	if err != nil {
		panic(err)
	}
	return proc
}

func wait_exec_proc(proc *exec.Cmd, job_id string) error {
	child_pid := proc.Process.Pid
	fmt.Printf("Waiting for pid %d to finish running...", child_pid)
	result := make(chan error, 1)
	go func() {
		result <- proc.Wait()
	}()

	sigc := make(chan os.Signal)
	signal.Notify(sigc,
		syscall.SIGHUP,
		syscall.SIGINT,
		syscall.SIGTERM,
		syscall.SIGQUIT)
	go func() {
		for true {
			s := <-sigc
			fmt.Println("Process ", child_pid, " got Signal:", s)
			// proc.Process.Signal(s)
			// syscall.Tgkill(-1, proc.Process.Pid, 3)
			pgid, err := syscall.Getpgid(proc.Process.Pid)
			if err == nil {
				if job_id != "" {
					cjs := DbGetChildJobs(job_id)
					for _, cj := range cjs {
						DoKillJob(cj.Job_ID, fmt.Sprintf("terminating parent %s", job_id))
					}
				}
				syscall.Kill(-pgid, s.(syscall.Signal)) // note the minus sign
			}
		}
	}()

	var ret error
	keep_waiting := true
	for keep_waiting {
		time.Sleep(1 * time.Second)
		select {
		case retCode, _ := <-result:
			fmt.Println(child_pid, " Return code: ", retCode)
			ret = retCode
			keep_waiting = false
		default:
			// fmt.Println("Process still running, pid ", proc.Process.Pid)
		}
	}
	// outfile.Close()
	fmt.Println(child_pid, " done")
	return ret
}

func jobGetNextJobNumber(jobname string, hostname string) string {
	timestamp := time.Now().UnixNano()
	str_to_hash := fmt.Sprintf("%s %s %x", jobname, hostname, timestamp)
	fmt.Printf("JOB BEFORE HASH: '%s'", str_to_hash)
	sum := sha1.Sum([]byte(str_to_hash))
	return fmt.Sprintf("%x", sum)
}

func JobBaseNameFromCommand(jobstr string) string {
	pieces := strings.Fields(jobstr)
	if len(pieces) > 0 {
		return pieces[0]
	} else {
		return "empty-job"
	}
}

func jobGetUrl(job_id string) string {
	c := S5ciOptions.Config
	return fmt.Sprintf("%s/%s/", c.Jobs.Root_Url, job_id)
}

func jobGetName(job_id string) string {
	nameRegex := regexp.MustCompile(`([^A-Za-z0-9_])`)
	return nameRegex.ReplaceAllString(job_id, "_")
}

func jobGetConsolePath(job_id string) string {
	c := S5ciOptions.Config
	return filepath.Join(c.Jobs.Rootdir, job_id, "console.txt")
}

func jobGetWorkspacePath(job_id string) string {
	c := S5ciOptions.Config
	return filepath.Join(c.Jobs.Rootdir, job_id, "workspace")
}

func JobSplitJobNR(job_nr string) string {
	first_level := job_nr[0:3]
	second_level := job_nr[3:6]
	third_level := job_nr[6:9]
	rest_level := job_nr[9:]
	job_dir := filepath.Join(first_level, second_level, third_level, rest_level)
	return job_dir
}

func JobGetJobID(job_group string, job_nr string) string {
	return filepath.Join(job_group, JobSplitJobNR(job_nr))
}

func JobGetPath(job_group string, job_nr string) string {
	c := S5ciOptions.Config
	return filepath.Join(c.Jobs.Rootdir, job_group, JobSplitJobNR(job_nr))
}

func jobCreateWorkspace(job_group string, job_nr string) {
	c := S5ciOptions.Config
	job_group_dir := filepath.Join(c.Jobs.Rootdir, job_group)
	_ = os.Mkdir(job_group_dir, 0755)

	first_level := job_nr[0:3]
	second_level := job_nr[3:6]
	third_level := job_nr[6:9]

	a_dir := filepath.Join(c.Jobs.Rootdir, job_group, first_level)
	err := os.Mkdir(a_dir, 0755)
	if err != nil {
		log.Fatal(err)
	}

	b_dir := filepath.Join(c.Jobs.Rootdir, job_group, first_level, second_level)
	err = os.Mkdir(b_dir, 0755)
	if err != nil {
		log.Fatal(err)
	}

	c_dir := filepath.Join(c.Jobs.Rootdir, job_group, first_level, second_level, third_level)
	err = os.Mkdir(c_dir, 0755)
	if err != nil {
		log.Fatal(err)
	}

	job_dir := JobGetPath(job_group, job_nr)
	err = os.Mkdir(job_dir, 0755)
	if err != nil {
		log.Fatal(err)
	}

	job_workspace_dir := filepath.Join(job_dir, "workspace")
	err = os.Mkdir(job_workspace_dir, 0755)
	if err != nil {
		log.Fatal(err)
	}
}

func jobFindBestCommand(job_group string) string {
	c := S5ciOptions.Config
	best_command := job_group
	for best_command != "" {
		log.Print("best command - trying ", best_command)
		full_cmd := filepath.Join(c.Command_Rootdir, best_command)
		if fi, err := os.Stat(full_cmd); err == nil {
			if !fi.IsDir() {
				return best_command
			}
		}
		dash_index := strings.LastIndex(best_command, "-")
		if dash_index != -1 {
			best_command = best_command[0:dash_index]
		} else {
			best_command = ""

		}
	}
	return job_group

}

func JobSpawnCommand(c *S5ciConfig, rtdt *S5ciRuntimeData, jobstr string) {
	changeset_id := fmt.Sprintf("%d", rtdt.ChangesetID)
	patchset_id := fmt.Sprintf("%d", rtdt.PatchsetID)

	exe_name := rtdt.RealS5ciExe
	proc := exec.Command(exe_name, "run-job", "-c", jobstr, "-p", patchset_id, "-s", changeset_id)
	log.Printf("Start job via our exe %s", exe_name)
	proc.SysProcAttr = &syscall.SysProcAttr{Setpgid: true, Pgid: 0}
	if rtdt.SandboxLevel >= 2 {
		log.Printf("Sandbox level %d, not actually launching the job", rtdt.SandboxLevel)
		return
	}

	new_env := append([]string{}, os.Environ()...)
	new_env = append(new_env, fmt.Sprintf("S5CI_SANDBOX_LEVEL=%d", S5ciOptions.SandboxLevel))
	new_env = append(new_env, fmt.Sprintf("S5CI_CONFIG=%s", S5ciConfigPath))
	new_env = append(new_env, fmt.Sprintf("S5CI_TRIGGER_EVENT_ID=%s", rtdt.TriggerEventID))

	proc.Env = new_env

	err := proc.Start()
	if err != nil {
		panic(err)
	}
	log.Printf("Started job %s", jobstr)
}

func JobExecCommand(c *S5ciConfig, rtdt *S5ciRuntimeData, jobstr string) {
	changeset_id := rtdt.ChangesetID
	patchset_id := rtdt.PatchsetID
	exe_name, err := filepath.Abs(os.Args[0])
	if err != nil {
		log.Fatal(err)
	}
	job_group := JobBaseNameFromCommand(jobstr)
	job_nr := jobGetNextJobNumber(job_group, rtdt.Hostname)
	job_id := JobGetJobID(job_group, job_nr)
	jobCreateWorkspace(job_group, job_nr)
	fmt.Println("running job ", job_group, " job id ", job_nr)
	best_command := jobFindBestCommand(job_group)
	new_command := jobstr
	if best_command != job_group {
		new_command = best_command + " " + jobstr[len(best_command)+1:]
	}

	new_env := append([]string{}, fmt.Sprintf("PATH=%s:%s", c.Command_Rootdir, os.Getenv("PATH")))
	new_env = append(new_env, fmt.Sprintf("S5CI_EXE=%s", exe_name))
	new_env = append(new_env, fmt.Sprintf("S5CI_JOB_ID=%s", job_id))
	new_env = append(new_env, fmt.Sprintf("S5CI_WORKSPACE=%s", jobGetWorkspacePath(job_id)))
	new_env = append(new_env, fmt.Sprintf("S5CI_CONSOLE_LOG=%s", jobGetConsolePath(job_id)))
	new_env = append(new_env, fmt.Sprintf("S5CI_JOB_NAME=%s", jobGetName(job_id)))
	new_env = append(new_env, fmt.Sprintf("S5CI_JOB_URL=%s", jobGetUrl(job_id)))
	new_env = append(new_env, fmt.Sprintf("S5CI_SANDBOX_LEVEL=%d", S5ciOptions.SandboxLevel))
	new_env = append(new_env, fmt.Sprintf("S5CI_CONFIG=%s", S5ciConfigPath))
	new_env = append(new_env, fmt.Sprintf("S5CI_GERRIT_CHANGESET_ID=%d", changeset_id))
	new_env = append(new_env, fmt.Sprintf("S5CI_GERRIT_PATCHSET_ID=%d", patchset_id))
	new_env = append(new_env, fmt.Sprintf("S5CI_PARENT_JOB_ID=%s", os.Getenv("S5CI_JOB_ID")))
	new_env = append(new_env, fmt.Sprintf("S5CI_PARENT_JOB_NAME=%s", os.Getenv("S5CI_JOB_NAME")))
	new_env = append(new_env, fmt.Sprintf("S5CI_PARENT_JOB_URL=%s", os.Getenv("S5CI_JOB_URL")))
	new_env = append(new_env, fmt.Sprintf("S5CI_TRIGGER_EVENT_ID=%s", rtdt.TriggerEventID))

	pj_id := os.Getenv("S5CI_JOB_ID")
	pj_id_ptr := &pj_id
	if pj_id == "" {
		pj_id_ptr = nil
	}

	now := S5Now()

	new_job := Job{
		Record_UUID:      DbUUID(),
		Job_Group_Name:   job_group,
		Instance_ID:      job_nr,
		Job_ID:           job_id,
		Job_Pid:          os.Getpid(),
		Parent_Job_ID:    pj_id_ptr,
		Changeset_ID:     changeset_id,
		Patchset_ID:      patchset_id,
		Command:          jobstr,
		Command_Pid:      nil,
		Remote_Host:      &rtdt.Hostname,
		Status_Message:   "",
		Trigger_Event_ID: rtdt.TriggerEventID,
		Started_At:       &now}

	db := DbOpen()
	DbInsertJob(&db, &new_job)
	StartingJob(new_job.Job_ID)

	syscall.Setsid()
	proc := start_exec_fg_with_redir(new_command, jobGetConsolePath(job_id), jobGetWorkspacePath(job_id), new_env)
	new_job.Command_Pid = &proc.Process.Pid
	DbSaveJob(&db, &new_job)
	DbClose(&db)

	retErr := wait_exec_proc(proc, job_id)

	exec_success := true
	exec_retcode := 0
	if retErr != nil {
		exec_success = proc.ProcessState.Success()
		exec_retcode = 4242 // proc.ProcessState.ExitStatus()
		if exitErr, ok := retErr.(*exec.ExitError); ok {
			if status, ok := exitErr.Sys().(syscall.WaitStatus); ok {
				exec_retcode = status.ExitStatus()
			}
		}
	}
	finished_now := S5Now()
	new_job.Finished_At = &finished_now
	new_job.Return_Success = exec_success
	new_job.Return_Code = &exec_retcode
	db = DbOpen()
	DbSaveJob(&db, &new_job)
	DbClose(&db)
	FinishedJob(new_job.Job_ID)
}
