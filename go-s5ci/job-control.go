package main

import (
	"fmt"
	"io/ioutil"
	"log"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
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

func wait_exec_proc(proc *exec.Cmd) error {
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
			fmt.Println("Signal:", s)
			// proc.Process.Signal(s)
			// syscall.Tgkill(-1, proc.Process.Pid, 3)
			pgid, err := syscall.Getpgid(proc.Process.Pid)
			if err == nil {
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
			fmt.Println("Return code: ", retCode)
			ret = retCode
			keep_waiting = false
		default:
			fmt.Println("Process still running, pid ", proc.Process.Pid)
		}
	}
	// outfile.Close()
	fmt.Println("done")
	return ret
}

func exec_fg_with_redir(cmdline string, out_fname string, new_cwd string, add_env []string) error {
	proc := start_exec_fg_with_redir(cmdline, out_fname, new_cwd, add_env)
	return wait_exec_proc(proc)
}

func getMinJobNumber(jobname string) int {
	c := S5ciOptions.Config
	files, _ := ioutil.ReadDir(filepath.Join(c.Jobs.Rootdir, jobname))
	return len(files) + 1
}

func jobGetNextJobNumber(jobname string) int {
	min_number := getMinJobNumber(jobname)
	log.Print("min job number for %s is %d", jobname, min_number)
	next_job_number := DbGetNextCounterWithMin(jobname, min_number)
	return next_job_number
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

func jobGetConsolePath(job_id string) string {
	c := S5ciOptions.Config
	return filepath.Join(c.Jobs.Rootdir, job_id, "console.txt")
}

func jobGetWorkspacePath(job_id string) string {
	c := S5ciOptions.Config
	return filepath.Join(c.Jobs.Rootdir, job_id, "workspace")
}

func jobCreateWorkspace(job_group string, job_nr int) {
	c := S5ciOptions.Config
	job_group_dir := filepath.Join(c.Jobs.Rootdir, job_group)
	_ = os.Mkdir(job_group_dir, 0755)

	job_dir := filepath.Join(c.Jobs.Rootdir, job_group, fmt.Sprintf("%d", job_nr))
	err := os.Mkdir(job_dir, 0755)
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

	exe_name, err := filepath.Abs(os.Args[0])
	if err != nil {
		panic(err)
	}
	proc := exec.Command(exe_name, "run-job", "-c", jobstr, "-p", patchset_id, "-s", changeset_id)
	log.Printf("Start job via our exe %s", exe_name)
	proc.SysProcAttr = &syscall.SysProcAttr{Setpgid: true, Pgid: 0}

	new_env := append([]string{}, os.Environ()...)
	new_env = append(new_env, fmt.Sprintf("S5CI_SANDBOX_LEVEL=%d", S5ciOptions.SandboxLevel))
	new_env = append(new_env, fmt.Sprintf("S5CI_CONFIG=%s", S5ciConfigPath))

	proc.Env = new_env

	err = proc.Start()
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
	job_nr := jobGetNextJobNumber(job_group)
	job_id := fmt.Sprintf("%s/%d", job_group, job_nr)
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
	new_env = append(new_env, fmt.Sprintf("S5CI_JOB_NAME=%s", job_group))
	new_env = append(new_env, fmt.Sprintf("S5CI_JOB_URL=%s", jobGetUrl(job_id)))
	new_env = append(new_env, fmt.Sprintf("S5CI_SANDBOX_LEVEL=%d", S5ciOptions.SandboxLevel))
	new_env = append(new_env, fmt.Sprintf("S5CI_CONFIG=%s", S5ciConfigPath))
	new_env = append(new_env, fmt.Sprintf("S5CI_PARENT_JOB_ID=%s", os.Getenv("S5CI_JOB_ID")))
	new_env = append(new_env, fmt.Sprintf("S5CI_PARENT_JOB_NAME=%s", os.Getenv("S5CI_JOB_NAME")))
	new_env = append(new_env, fmt.Sprintf("S5CI_PARENT_JOB_URL=%s", os.Getenv("S5CI_JOB_URL")))

	pj_id := os.Getenv("S5CI_PARENT_JOB_ID")
	pj_id_ptr := &pj_id
	if pj_id == "" {
		pj_id_ptr = nil
	}

	now := S5Now()

	new_job := Job{
		Record_UUID:    DbUUID(),
		Job_Group_Name: job_group,
		Instance_ID:    job_nr,
		Job_ID:         job_id,
		Job_Pid:        os.Getpid(),
		Parent_Job_ID:  pj_id_ptr,
		Changeset_ID:   changeset_id,
		Patchset_ID:    patchset_id,
		Command:        jobstr,
		Command_Pid:    nil,
		Status_Message: "",
		Started_At:     &now}

	db := DbOpen()
	DbInsertJob(&db, &new_job)
	StartingJob(new_job.Job_ID)

	syscall.Setsid()
	proc := start_exec_fg_with_redir(new_command, jobGetConsolePath(job_id), jobGetWorkspacePath(job_id), new_env)
	new_job.Command_Pid = &proc.Process.Pid
	DbSaveJob(&db, &new_job)
	DbClose(&db)

	retErr := wait_exec_proc(proc)

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
