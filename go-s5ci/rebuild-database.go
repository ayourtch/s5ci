package main

import (
	"errors"
	"fmt"
	"io/ioutil"
	"log"
	"os"
	"path/filepath"
)

type RebuildDatabaseCommand struct {
	InitOnly bool `short:"i" long:"init-only" description:"only init the db/build schema, do not import"`
}

func db_restore_job_from(dir_path string) {
	job_path := filepath.Join(dir_path, "job.yaml")
	fi, err := os.Stat(job_path)
	if err == nil {
		if !fi.IsDir() {
			fmt.Println("Restoring ", job_path)
			job, _ := Import_Job_YAML(job_path)
			db := DbOpen()
			DbInsertJob(&db, &job)
			DbClose(&db)
		}
	} else {
		/* probably an intermediate dir, try to dive in */
		group_path := dir_path
		files, err := ioutil.ReadDir(group_path)
		if err != nil {
			log.Fatal(err)
		}
		for _, f := range files {
			if f.IsDir() {
				db_restore_job_from(filepath.Join(dir_path, f.Name()))
			}
			fmt.Println(f.Name())
		}
	}
}
func Db_restore_job_from(dir_path string) {
	db_restore_job_from(dir_path)
}
func db_restore_job_group_from(root string, group_name string) {
	group_path := filepath.Join(root, group_name)
	db_restore_job_group_from_path(group_path)
}

func db_restore_job_group_from_path(group_path string) {
	files, err := ioutil.ReadDir(group_path)
	if err != nil {
		log.Fatal(err)
	}
	for _, f := range files {
		if f.IsDir() {
			db_restore_job_from(filepath.Join(group_path, f.Name()))
		}
		fmt.Println(f.Name())
	}

}

func db_restore_jobs_from(root string) {
	files, err := ioutil.ReadDir(root)
	if err != nil {
		log.Fatal(err)
	}

	for _, f := range files {
		if f.IsDir() {
			db_restore_job_group_from(root, f.Name())
		}
		fmt.Println(f.Name())
	}
}

func (command *RebuildDatabaseCommand) Execute(args []string) error {
	var ErrShowHelpMessage = errors.New("restore database")
	fmt.Println("==== restoring database ====")

	DbInitDatabase()
	if command.InitOnly {
		return nil
	}

	db_restore_jobs_from(S5ciOptions.Config.Jobs.Rootdir)
	fmt.Println("Test")
	return ErrShowHelpMessage
}
