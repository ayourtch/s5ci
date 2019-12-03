package main

import (
	"database/sql"
	"errors"
	"fmt"
	mustache "github.com/hoisie/mustache"
	"github.com/jinzhu/gorm"
	_ "github.com/jinzhu/gorm/dialects/sqlite"
	_ "github.com/mattn/go-sqlite3"
	"log"
	"os"
	"time"
)

func ReinitTestDatabase(dbfile string) error {
	os.Remove(dbfile)
	db, err := sql.Open("sqlite3", dbfile)
	if err != nil {
		log.Fatal(err)
	}
	defer db.Close()

	sqlStmt := `
	create table foo (id integer not null primary key, name text);
	delete from foo;
	`
	_, err = db.Exec(sqlStmt)
	if err != nil {
		log.Printf("%q: %s\n", err, sqlStmt)
		return errors.New("error creating table")
	}
	return nil
}

const GlobalDebug = false

func SetTestRowValue(dbfile string, row_id int, row_value string) error {
	if GlobalDebug {
		fmt.Println("setting row ", row_id, " to ", row_value)
	}
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

	stmt, err := tx.Prepare("update foo set name = ? where id = ?")
	if err != nil {
		log.Fatal(err)
	}
	defer stmt.Close()

	_, err = stmt.Exec(row_value, row_id)
	retry_count := 5
	for err != nil {
		fmt.Println("ERROR: ", err, "Retry count:", retry_count)
		time.Sleep(1 * time.Second)
		_, err = stmt.Exec(row_value, row_id)
		retry_count = retry_count - 1
		if retry_count < 0 {
			log.Fatal(err)
		}
	}
	time.Sleep(1 * time.Second)
	tx.Commit()
	// fmt.Println("Finish update")
	return err
}

func ShuffleTestDatabase(dbfile string) error {
	db, err := sql.Open("sqlite3", dbfile)
	if err != nil {
		log.Fatal(err)
	}
	defer db.Close()

	tx, err := db.Begin()
	if err != nil {
		log.Fatal(err)
	}
	stmt, err := tx.Prepare("insert into foo(id, name) values(?, ?)")
	if err != nil {
		log.Fatal(err)
	}
	defer stmt.Close()
	for i := 0; i < 100; i++ {
		_, err = stmt.Exec(i, fmt.Sprintf("こんにちわ世界%03d", i))
		if err != nil {
			log.Fatal(err)
		}
	}
	tx.Commit()
	rows, err := db.Query("select id, name from foo")
	if err != nil {
		log.Fatal(err)
	}
	defer rows.Close()
	/*
		for rows.Next() {
			var id int
			var name string
			err = rows.Scan(&id, &name)
			if err != nil {
				log.Fatal(err)
			}
			fmt.Println(id, name)
		}
		err = rows.Err()
		if err != nil {
			log.Fatal(err)
		}
	*/

	stmt, err = db.Prepare("select name from foo where id = ?")
	if err != nil {
		log.Fatal(err)
	}
	defer stmt.Close()
	var name string
	err = stmt.QueryRow("3").Scan(&name)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(name)

	_, err = db.Exec("delete from foo")
	if err != nil {
		log.Fatal(err)
	}

	_, err = db.Exec("insert into foo(id, name) values(1, 'foo'), (2, 'bar'), (3, 'baz')")
	if err != nil {
		log.Fatal(err)
	}

	rows, err = db.Query("select id, name from foo")
	if err != nil {
		log.Fatal(err)
	}
	defer rows.Close()
	for rows.Next() {
		var id int
		var name string
		err = rows.Scan(&id, &name)
		if err != nil {
			log.Fatal(err)
		}
		fmt.Println(id, name)
	}
	err = rows.Err()
	if err != nil {
		log.Fatal(err)
	}
	return nil
}

type DevTestCommand struct {
	Init           bool    `short:"i" long:"init" description:"init the db"`
	WebRpcStart    bool    `short:"w" long:"web-rpc-start" description:"start the RPC listener"`
	Shuffle        bool    `short:"s" long:"shuffle" description:"shuffle the db"`
	Stress         bool    `long:"stress" description:"stress the db"`
	TemplateFile   string  `short:"t" long:"template" description:"template file"`
	CommandToSpawn *string `short:"c" long:"command-to-spawn" description:"commmand to spawn"`
	RegenerateHtml bool    `short:"r" long:"regenerate-html" description:"regenerate all html"`
	JobId          string  `short:"j" long:"job-id" description:"job-id to shorten"`
}

type Foo struct {
	Foo_Id    int
	Foo_Value string
}

func (command *DevTestCommand) Execute(args []string) error {
	var ErrShowHelpMessage = errors.New("run dev test")
	fmt.Println("Dev Test")

	if command.JobId != "" {
		res := JobShortenJobId(command.JobId)
		fmt.Println("Short job id: ", res)
	}
	if command.WebRpcStart {
		StartWebRpcServer()
	}

	if command.CommandToSpawn != nil {
		c := &S5ciOptions.Config
		rtdt := &S5ciRuntime
		JobSpawnCommand(c, rtdt, *command.CommandToSpawn)
		CollectZombies()
		JobSpawnCommand(c, rtdt, *command.CommandToSpawn)
		CollectZombies()
		JobSpawnCommand(c, rtdt, *command.CommandToSpawn)
		CollectZombies()
		time.Sleep(10 * time.Second)
		CollectZombies()
		time.Sleep(10 * time.Second)
	}

	if command.Init {
		ReinitTestDatabase("./foo.db")
		db, err := gorm.Open("sqlite3", "./foo.db")
		if err != nil {
			panic("failed to connect database")
		}
		db.AutoMigrate(&Foo{})
		db.Create(&Foo{Foo_Id: 123, Foo_Value: "initial foo value"})
		db.Close()
	}
	if command.Shuffle {
		ShuffleTestDatabase("./foo.db")
	}

	if command.TemplateFile != "" {
		t, _ := mustache.ParseFile(command.TemplateFile)
		fmt.Println("template test:")
		err := t.Render(os.Stdout, &command)
		fmt.Println("Template error:", err)
		data := make(map[string]interface{})
		data["string"] = "string value"
		data["command"] = command
		err = t.Render(os.Stdout, &data)
		fmt.Println("Template data error:", err)

	}
	db, err := gorm.Open("sqlite3", "./foo.db")
	if err != nil {
		panic("failed to connect database")
	}
	defer db.Close()

	db.AutoMigrate(&Foo{})
	if command.RegenerateHtml {
		RegenerateAllHtml()
	}

	if command.Stress {
		for {
			var foo Foo
			t := time.Now()
			// i = i + 1
			// string_val := fmt.Sprintf("--this is a second row, count: %d, time: %d--", i, t.Unix())
			string_val := fmt.Sprintf("--this is a second row, time: %d--", t.Unix())
			// time.Sleep(1 * time.Millisecond)
			foo.Foo_Id = 123
			foo.Foo_Value = string_val
			db.Model(&foo).Where("foo_id = ?", 123).Updates(&foo)

			// SetTestRowValue("./foo.db", 2, string_val)
		}
	}

	return ErrShowHelpMessage
}
