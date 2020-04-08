package main

import (
	"io"
	"os"
	"log"
	"path/filepath"
)

// s5LogInit Initializes the logger for the s5 module. several log types can
// be used. Trace, Info, Warning and Error. The io.Writers typically might
// be stdOut, Stderr, ioutil.Discard or a file descriptor. The writers for
// the log types are taken as the arguments to s5LogInit.
//
// The date, time and the short file name will be part of each log statment..
//
// Example of the s5Init:
//
//   s5LogInit(ioutil.Discard, os.Stdout, os.Stdout, os.Stderr, "./logs/s5ci.log")
//
// Example of log calls:
//
//   Trace.Println("This is a Trace")
//   Info.Println("This is Info")
//   Warning.Println("This is a Warning")
//   Error.Println("This is an Error")
//

var (
    Trace    *log.Logger
    Info     *log.Logger
    Warning  *log.Logger
    Error    *log.Logger
)

func s5LogInit(
	traceHandle io.Writer,
	infoHandle  io.Writer,
	warningHandle io.Writer,
	errorHandle io.Writer,
) {

	// Set the handles
	Trace = log.New(traceHandle,
		"TRACE: ",
		log.Ldate | log.Ltime | log.Lshortfile)
	Info = log.New(infoHandle,
		"INFO: ",
		log.Ldate | log.Ltime | log.Lshortfile)
	Warning = log.New(warningHandle,
		"WARNING: ",
		log.Ldate | log.Ltime | log.Lshortfile)
	Error = log.New(errorHandle,
		"ERROR: ",
		log.Ldate | log.Ltime | log.Lshortfile)
}

// s5CreateFile creates a file from the filename specified, the function
// returns the file descriptor and error

func s5LogCreateFile (filename string) (*os.File, error) {

	// Get the directory create one if it doesn't exist
	dir, _ := filepath.Split(filename)
	err := os.MkdirAll(dir, os.ModePerm)
	if err != nil {
		log.Fatalf("The log file directory could not be created %v \n", err)
	}

	// Open the file
	if filename != "" {
		fp, err := os.OpenFile(filename,
			os.O_RDWR | os.O_CREATE | os.O_APPEND, 0666)
		if err != nil {
			log.Fatalf("FATAL: error opening file %v for logging", err)
		}
		return fp, err
	}
	return nil, nil
}
