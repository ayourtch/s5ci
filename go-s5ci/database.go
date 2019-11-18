package main

import (
	"database/sql"
	"errors"
	"fmt"
	"github.com/google/uuid"
	"github.com/jinzhu/gorm"
	_ "github.com/jinzhu/gorm/dialects/sqlite"
	_ "github.com/mattn/go-sqlite3"
	"gopkg.in/yaml.v2"
	"io/ioutil"
	"log"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const databaseGlobalDebug = false

func execStatement(update_str string, exact_values ...interface{}) error {
	dbfile := S5ciOptions.Config.Db_URL

	db, err := sql.Open("sqlite3", dbfile)
	if err != nil {
		log.Fatal(err)
	}
	db.SetMaxOpenConns(1)
	defer db.Close()

	tx, err := db.Begin()
	if err != nil {
		log.Fatal(err)
	}

	stmt, err := tx.Prepare(update_str)
	if err != nil {
		log.Fatal(err)
	}
	defer stmt.Close()

	_, err = stmt.Exec(exact_values...)
	retry_count := 5
	for err != nil {
		fmt.Println("ERROR: ", err, "Retry count:", retry_count)
		time.Sleep(1 * time.Second)
		_, err = stmt.Exec(exact_values...)
		retry_count = retry_count - 1
		if retry_count < 0 {
			log.Fatal(err)
		}
	}
	// time.Sleep(1 * time.Second)
	tx.Commit()
	// fmt.Println("Finish update")
	return err
}

func InitCounter(name string, value int) error {
	return execStatement("insert into counters values(?, ?)", name, value)
}

type S5ciDb struct {
	db *gorm.DB
}

func DbInitDatabase() {
	db, err := gorm.Open("sqlite3", S5ciOptions.Config.Db_URL)
	if err != nil {
		panic(err)
	}
	defer db.Close()

	db.AutoMigrate(&Comment{})
	db.AutoMigrate(&Job{})
	db.AutoMigrate(&Counter{})
	db.AutoMigrate(&Timestamp{})
}

func dbGetLockPath(name string) string {
	return filepath.Join("/tmp", fmt.Sprintf("%s.lock", name))
}

func DbLockNamed(name string) {
	lpath := dbGetLockPath(name)
	retry_count := 5
	for os.Mkdir(lpath, 0700) != nil && retry_count > 0 {
		retry_count--
		log.Print("error locking ", name, " retries left ", retry_count)
		time.Sleep(1 * time.Second)
	}
	if retry_count <= 0 {
		log.Fatal("Could not lock %s", name)
	}

}

func DbUnlockNamed(name string) {
	lpath := dbGetLockPath(name)
	err := os.Remove(lpath)
	if err != nil {
		log.Fatal("Error unlocking %s: %s", name, err)
	}

}

func DbGetTimestamp(timestamp_name string) (int, error) {
	var timestamp Timestamp
	db := DbOpen()
	defer DbClose(&db)
	err := db.db.Where("Name = ?", timestamp_name).First(&timestamp).Error
	if err != nil {
		return -1, err
	}
	return timestamp.Value.UnixTimestamp(), nil
}

func DbSetTimestamp(timestamp_name string, timestamp_value int) {
	var timestamp Timestamp
	t := S5TimeFromTimestamp(timestamp_value)
	db := DbOpen()
	if err := db.db.Where("Name = ?", timestamp_name).First(&timestamp).Error; err != nil {
		timestamp.Name = timestamp_name
		timestamp.Value = &t
		db.db.Create(&timestamp)
	} else {
		timestamp.Value = &t
		db.db.Save(&timestamp)
	}
	DbClose(&db)
}

func DbGetChangesetLastComment(change_id int) int {
	var comment Comment
	db := DbOpen()
	if err := db.db.Where("Changeset_ID = ?", change_id).First(&comment).Error; err != nil {
		comment.Record_UUID = DbUUID()
		comment.Changeset_ID = change_id
		comment.Comment_ID = -1
		db.db.Create(&comment)
	}
	DbClose(&db)
	return comment.Comment_ID
}

func DbSetChangesetLastComment(change_id int, comment_id int) {
	var comment Comment
	db := DbOpen()
	if err := db.db.Where("Changeset_ID = ?", change_id).First(&comment).Error; err != nil {
		comment.Record_UUID = DbUUID()
		comment.Changeset_ID = change_id
		comment.Comment_ID = -1
		db.db.Create(&comment)
	}
	db.db.Model(comment).Where("record_uuid = ?", comment.Record_UUID).Update("comment_id", comment_id)
	DbClose(&db)
}

func DbGetNextCounterWithMin(name string, min_val int) int {
	var counter Counter
	DbLockNamed(name)
	db := DbOpen()
	log.Print("Min value for ", name, " is ", min_val)
	if err := db.db.Where("Name = ?", name).First(&counter).Error; err != nil {
		counter.Name = name
		counter.Value = min_val
		db.db.Create(&counter)
	}
	if counter.Value < min_val {
		counter.Value = min_val
	}
	retval := counter.Value
	counter.Value++
	db.db.Save(&counter)
	DbClose(&db)
	DbUnlockNamed(name)
	return retval
}

func DbOpen() S5ciDb {
	db, err := gorm.Open("sqlite3", S5ciOptions.Config.Db_URL)
	retry_count := 5
	for err != nil {
		fmt.Println("DB OPEN ERROR: ", err, "Retry count:", retry_count)
		time.Sleep(1 * time.Second)
		db, err = gorm.Open("sqlite3", S5ciOptions.Config.Db_URL)
		retry_count = retry_count - 1
		if retry_count < 0 {
			log.Fatal(err)
		}
	}
	if err != nil {
		panic("failed to connect database")
	}
	// defer db.Close()
	return S5ciDb{db: db}
}

func DbClose(db *S5ciDb) {
	db.db.Close()
}

func DbInsertJob(db *S5ciDb, job *Job) {
	db.db.Where("record_uuid = ?", job.Record_UUID).Delete(Job{})
	db.db.Create(job)
}
func DbSaveJob(db *S5ciDb, job *Job) {
	db.db.Model(job).Where("record_uuid = ?", job.Record_UUID).Updates(job)
}

func DbGetAllJobs() []Job {
	db := DbOpen()
	defer DbClose(&db)

	jobs := []Job{}

	db.db.Find(&jobs)
	return jobs
}

func DbGetJob(job_id string) (*Job, error) {
	db := DbOpen()
	defer DbClose(&db)

	var job Job
	if err := db.db.Where("job_id = ?", job_id).First(&job).Error; err != nil {
		return nil, err
	} else {
		return &job, nil
	}
}

func DbGetChildJobs(job_id string) []Job {
	db := DbOpen()
	defer DbClose(&db)
	jobs := []Job{}
	db.db.Where("parent_job_id = ?", job_id).Order("started_at desc").Find(&jobs)
	return jobs
}

func DbGetActiveJobs() []Job {
	db := DbOpen()
	defer DbClose(&db)
	jobs := []Job{}
	db.db.Where("finished_at is null").Order("started_at desc").Find(&jobs)
	return jobs
}

func DbGetRootJobs() []Job {
	db := DbOpen()
	defer DbClose(&db)
	jobs := []Job{}
	db.db.Where("parent_job_id is null").Order("started_at desc").Find(&jobs)
	return jobs
}

func DbGetJobsByGroupName(group_name string) []Job {
	db := DbOpen()
	defer DbClose(&db)
	jobs := []Job{}

	db.db.Where("job_group_name = ?", group_name).Order("started_at desc").Find(&jobs)
	return jobs
}

type DbMaintCommand struct {
	Init    bool `short:"i" long:"init" description:"init the db"`
	Shuffle bool `short:"s" long:"shuffle" description:"shuffle the db"`
}

type Comment struct {
	// gorm.Model
	Record_UUID  string `gorm:"size:32;unique_index;not null"`
	Changeset_ID int    `gorm:"unique_index;not null"`
	Comment_ID   int
}

type Job struct {
	// gorm.Model
	Record_UUID       string `gorm:"size:32;unique_index;not null"`
	Job_Group_Name    string
	Instance_ID       string
	Job_ID            string `gorm:"index;not null"`
	Job_Pid           int
	Parent_Job_ID     *string
	Changeset_ID      int
	Patchset_ID       int
	Command           string
	Command_Pid       *int
	Remote_Host       *string /* can be null */
	Status_Message    string
	Status_Updated_At *S5Time // `sql:"type:time"`
	Started_At        *S5Time // `sql:"type:time"`
	Finished_At       *S5Time // `sql:"type:time"`
	Return_Success    bool
	Return_Code       *int
	Trigger_Event_ID  string
}

type Counter struct {
	// gorm.Model
	Name  string `gorm:"PRIMARY_KEY"`
	Value int
}

type Timestamp struct {
	Name  string  `gorm:"PRIMARY_KEY"`
	Value *S5Time `sql:"type:time"`
}

func DbUUID() string {
	return strings.Replace(uuid.New().String(), "-", "", -1)
}

func Import_Job_YAML(fname string) (Job, error) {
	t := Job{}
	data, err := ioutil.ReadFile(fname)
	if err != nil {
		log.Fatalf("error: %v", err)
	}

	err = yaml.Unmarshal([]byte(data), &t)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	fmt.Printf("--- job:\n%v\n\n", t)

	d, err := yaml.Marshal(&t)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	fmt.Printf("--- job dump:\n%s\n\n", string(d))

	return t, nil
}

func (command *DbMaintCommand) Execute(args []string) error {
	var ErrShowHelpMessage = errors.New("run db stuff")

	fmt.Println("db Test:", S5ciOptions.Config.Db_URL)

	db, err := gorm.Open("sqlite3", S5ciOptions.Config.Db_URL)
	if err != nil {
		panic("failed to connect database")
	}
	defer db.Close()

	db.AutoMigrate(&Comment{})
	db.AutoMigrate(&Job{})
	db.AutoMigrate(&Counter{})
	db.AutoMigrate(&Timestamp{})
	if command.Init {
		InitCounter("testCounter", 0)
	}

	db.Create(&Comment{Record_UUID: strings.Replace(uuid.New().String(), "-", "", -1), Changeset_ID: 123, Comment_ID: 1234})
	Import_Job_YAML("misc/job.yaml")

	// fmt.Println("config: ", command)

	return ErrShowHelpMessage
}
