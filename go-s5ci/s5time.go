package main

import (
	// "database/sql"
	"errors"
	"fmt"
	//"github.com/google/uuid"
	// "github.com/jinzhu/gorm"
	_ "github.com/jinzhu/gorm/dialects/sqlite"
	_ "github.com/mattn/go-sqlite3"
	// "log"
	// "os"
	"strings"
	"time"
	// "io/ioutil"
	// "gopkg.in/yaml.v2"
	"database/sql/driver"
	"encoding/json"
	"strconv"
	// "github.com/ghodss/yaml"
)

type S5Time struct {
	Time time.Time
}

func S5Now() S5Time {
	return S5Time{Time: time.Now()}
}

func UnixTimeNow() int {
	return int(time.Now().Unix())
}

func S5TimeFromTimestamp(timestamp int) S5Time {
	// FIXME: printing to string and back is kinda stupid... but might work now
	i, err := strconv.ParseInt(fmt.Sprintf("%d", timestamp), 10, 64)
	if err != nil {
		panic(err)
	}
	tm := time.Unix(i, 0)
	return S5Time{Time: tm}
}

func (t *S5Time) UnixTimestamp() int {
	return int(t.Time.Unix())
}

func (t *S5Time) UnmarshalYAML(unmarshal func(interface{}) error) error {

	var buf string
	err := unmarshal(&buf)
	if err != nil {
		return nil
	}

	tt, err := time.Parse("2006-01-02T15:04:05.000000000", strings.TrimSpace(buf))
	if err != nil {
		return err
	}
	fmt.Println("Parsed:", buf, tt)
	t.Time = tt
	return nil
}

func (t S5Time) MarshalYAML() (interface{}, error) {
	return t.Time.Format("2006-01-02T15:04:05.000000000"), nil
}

func (t S5Time) MarshalJSON() ([]byte, error) {
	return json.Marshal(t.Time.Format("2006-01-02T15:04:05.000000000"))
}

func (s5time *S5Time) UnmarshalJSON(b []byte) error {

	var timeStr string
	// Basically removes the double quotes from b and converts
	// it to a string.
	json.Unmarshal(b, &timeStr)

	// Parse the string to produce a proper time.Time struct.
	pt, err := time.Parse("2006-01-02T15:04:05.000000000", timeStr)
	if err != nil {
		return err
	}
	s5time.Time = pt
	return nil
}

// sql.Scanner implementation to convert a time.Time column to a S5Time
func (ld *S5Time) Scan(value interface{}) error {
	if tm, ok := value.(time.Time); ok {
		ld.Time = tm
		return nil
	} else {
		if tt, err := time.Parse("2006-01-02T15:04:05.000000000", strings.TrimSpace(value.(string))); err == nil {
			ld.Time = tt
			return nil
		}
		if tt, err := time.Parse("2006-01-02 15:04:05.000000000", strings.TrimSpace(value.(string))); err == nil {
			ld.Time = tt
			return nil
		}
		return errors.New("failed to scan S5Time despite multiple attempts")
	}
	// ld.Time = value.(time.Time)
	return nil
}

// sql/driver.Valuer implementation to go from S5Time -> time.Time
func (ld *S5Time) Value() (driver.Value, error) {
	if ld != nil {
		// return ld.Time.Format("2006-01-02T15:04:05.000000000"), nil
		return ld.Time, nil
	} else {
		return nil, nil
	}
}

/*
func (s5t *S5Time) String() string {
	if s5t != nil {
		return s5t.Time.Format("2006-01-02 15:04:05.000")
	} else {
		return ""
	}
}
*/
func (s5t S5Time) String() string {
	return s5t.Time.Format("2006-01-02 15:04:05.000")
}
