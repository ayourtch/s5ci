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

func db_restore_job_from(db *S5ciDb, dir_path string) {
	job_path := filepath.Join(dir_path, "job.yaml")
	fi, err := os.Stat(job_path)
	if err == nil {
		if !fi.IsDir() {
			// fmt.Println("Restoring ", job_path)
			job, _ := Import_Job_YAML(job_path)
			DbInsertJob(db, &job)
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
				db_restore_job_from(db, filepath.Join(dir_path, f.Name()))
			}
			// fmt.Println(f.Name())
		}
	}
}
func Db_restore_job_from(dir_path string) {
	db := DbOpen()
	db_restore_job_from(&db, dir_path)
	DbClose(&db)
}
func db_restore_job_group_from(db *S5ciDb, root string, group_name string) {
	group_path := filepath.Join(root, group_name)
	db_restore_job_group_from_path(db, group_path)
}

func db_restore_job_group_from_path(db *S5ciDb, group_path string) {
	files, err := ioutil.ReadDir(group_path)
	if err != nil {
		log.Fatal(err)
	}
	for _, f := range files {
		if f.IsDir() {
			db_restore_job_from(db, filepath.Join(group_path, f.Name()))
		}
		// fmt.Println(f.Name())
	}

}

func db_restore_jobs_from(root string) {
	files, err := ioutil.ReadDir(root)
	if err != nil {
		log.Fatal(err)
	}
	db := DbOpen()
	db.db.Exec("PRAGMA journal_mode=WAL;")
	txdb := DbBeginTransaction(&db)

	for _, f := range files {
		if f.IsDir() {
			db_restore_job_group_from(&txdb, root, f.Name())
		}
		// fmt.Println(f.Name())
	}
	DbCommitTransaction(&txdb)
	db.db.Exec("PRAGMA journal_mode=DELETE;")
	DbClose(&db)
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
