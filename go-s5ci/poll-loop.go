package main

// package commands

import (
	"fmt"
	"log"
	"time"
)

func PollLoop() {
	fmt.Println("Poll loop")
	c := &S5ciOptions.Config
	rtdt := &S5ciRuntime

	ts_now := int(time.Now().Unix())
	fmt.Println("Now: ", ts_now)
	s5time := S5TimeFromTimestamp(ts_now)
	fmt.Println(s5time)

	sync_horizon_sec := c.Default_Sync_Horizon_Sec

	default_after_ts := int(time.Now().Unix()) - sync_horizon_sec
	poll_ts, err := DbGetTimestamp("last-ssh-poll")
	if err == nil {
		default_after_ts = poll_ts
	}
	var before_ts *int = nil
	var after_ts *int = &default_after_ts
	trigger_delay_sec := c.Default_Regex_Trigger_Delay_Sec

	poll_timestamp := UnixTimeNow()

	for true {
		*after_ts = *after_ts + trigger_delay_sec

		now_ts := UnixTimeNow()
		if now_ts > poll_timestamp {
			res, err := PollGerritOverSsh(c, rtdt, before_ts, after_ts)
			if err != nil {
				log.Printf("Error in PollLoop: %v", err)
			} else {
				for _, cs := range res.Changes {
					GerritProcessChange(c, rtdt, cs, *after_ts)
				}
				before_ts = res.BeforeTS
				after_ts = res.AfterTS

				if before_ts != nil && *before_ts < now_ts-sync_horizon_sec {
					log.Printf("Time %d is beyond the horizon of %s seconds from now, finish sync", *before_ts, sync_horizon_sec)
					before_ts = nil
				}
			}
		}
		time.Sleep(5 * time.Second)

	}
}
