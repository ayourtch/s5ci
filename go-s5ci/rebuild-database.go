package main

import (
	"errors"
	"fmt"
	"io/ioutil"
	"log"
	"os"
	"path/filepath"
)

type RebuildDatabaseCommand struct{}

func db_restore_job_from(root string, group_name string, instance_id string) {
	job_path := filepath.Join(root, group_name, instance_id, "job.yaml")
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
		group_path := filepath.Join(root, group_name, instance_id)
		files, err := ioutil.ReadDir(group_path)
		if err != nil {
			log.Fatal(err)
		}
		for _, f := range files {
			if f.IsDir() {
				db_restore_job_from(root, group_name, filepath.Join(instance_id, f.Name()))
			}
			fmt.Println(f.Name())
		}
	}
}
func Db_restore_job_from(root string, group_name string, instance_id string) {
	db_restore_job_from(root, group_name, instance_id)
}

func db_restore_job_group_from(root string, group_name string) {
	group_path := filepath.Join(root, group_name)
	files, err := ioutil.ReadDir(group_path)
	if err != nil {
		log.Fatal(err)
	}
	for _, f := range files {
		if f.IsDir() {
			db_restore_job_from(root, group_name, f.Name())
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

	db_restore_jobs_from(S5ciOptions.Config.Jobs.Rootdir)
	fmt.Println("Test")
	return ErrShowHelpMessage
}
