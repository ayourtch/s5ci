package main

// package commands

import (
	"fmt"
	"github.com/jinzhu/copier"
	"log"
	"syscall"
	"time"
)

func CollectZombies() {
	pid := -1
	options := syscall.WNOHANG
	n_zombies := 0
	var wstatus syscall.WaitStatus
	for true {
		pid, err := syscall.Wait4(pid, &wstatus, options, nil)
		if pid == -1 {
			break
		}
		if pid == 0 {
			break
		}
		n_zombies = n_zombies + 1
		fmt.Println("Collected child process:", pid, err)
	}
	fmt.Println("Collected total zombies: ", n_zombies)
}

func PollLoop() {
	fmt.Println("Poll loop")
	c := &S5ciOptions.Config
	rtdt := &S5ciRuntime

	RegenerateAllHtml()

	ts_now := int(time.Now().Unix())
	fmt.Println("Now: ", ts_now)
	s5time := S5TimeFromTimestamp(ts_now)
	fmt.Println(s5time)

	sync_horizon_sec := c.Default_Sync_Horizon_Sec
	if c.Server.Sync_Horizon_Sec != nil {
		sync_horizon_sec = *c.Server.Sync_Horizon_Sec
	}

	default_after_ts := int(time.Now().Unix()) - sync_horizon_sec
	poll_ts, err := DbGetTimestamp("last-ssh-poll")
	if err == nil {
		default_after_ts = poll_ts
	}
	var before_ts *int = nil
	var after_ts *int = &default_after_ts
	var pending_comment_ts *int = nil
	trigger_delay_sec := c.Default_Regex_Trigger_Delay_Sec

	poll_timestamp := UnixTimeNow()
	autorestart_state := AutorestartInit(c, rtdt)
	for true {
		AutorestartCheck(c, rtdt, &autorestart_state)

		/*
		 We want to snoop further into the past by trigger_delay_sec,
		 because we don't react to any events before trigger_delay_sec elapses
		*/
		poll_after_ts := *after_ts - trigger_delay_sec - 10

		now_ts := UnixTimeNow()
		if now_ts > poll_timestamp {
			res, err := PollGerritOverSsh(c, rtdt, before_ts, &poll_after_ts)
			if err != nil {
				log.Printf("Error in PollLoop: %v", err)
			} else {
				for _, cs := range res.Changes {
					out_ts := GerritProcessChange(c, rtdt, cs, now_ts)
					if out_ts != nil && *out_ts < *res.AfterTS {
						*res.AfterTS = *out_ts
					}
					pending_comment_ts = out_ts
				}
				before_ts = res.BeforeTS
				after_ts = res.AfterTS
				DbSetTimestamp("last-ssh-poll", *after_ts)

				if before_ts != nil && *before_ts < now_ts-sync_horizon_sec {
					log.Printf("Time %d is beyond the horizon of %s seconds from now, finish sync", *before_ts, sync_horizon_sec)
					before_ts = nil
				}
			}
		}

		poll_delay_sec := (*c.Server.Poll_Wait_Ms / 1000)
		candidate_next_poll_ts := now_ts + poll_delay_sec
		if pending_comment_ts != nil {
			/* if there are comments pending to be processed due to trigger delay, ensure we don't miss them */
			if poll_delay_sec > c.Default_Regex_Trigger_Delay_Sec {
				candidate_next_poll_ts = now_ts + c.Default_Regex_Trigger_Delay_Sec
			}
		}

		// tm_now := time.Unix(int64(now_ts), 0)
		earliest_cron_ts := candidate_next_poll_ts
		for i, cs := range rtdt.CronTriggerSchedules {
			next_t := int(cs.Schedule.Next(time.Unix(int64(cs.LastRun), 0)).Unix())
			s5t := S5TimeFromTimestamp(next_t)
			if next_t < now_ts {
				log.Printf("Running cron %s", cs.Name)
				rtdt2 := S5ciRuntimeData{}
				copier.Copy(&rtdt2, rtdt)
				rtdt2.ChangesetID = 0
				rtdt2.PatchsetID = 0
				rtdt2.TriggerEventID = fmt.Sprintf("cron_%s", cs.Name)
				rtdt2.CommentValue = ""
				expanded_command := *c.Cron_Triggers[cs.Name].Action.Command
				log.Printf("running job: %s", expanded_command)
				JobSpawnCommand(c, &rtdt2, expanded_command)

				rtdt.CronTriggerSchedules[i].LastRun = next_t
				next_t = int(cs.Schedule.Next(time.Unix(int64(next_t), 0)).Unix())
			}
			log.Printf("NEXT(%s): %s", cs.Name, s5t)
			if next_t < earliest_cron_ts {
				earliest_cron_ts = next_t
			}

		}
		sleep_till_ts := candidate_next_poll_ts
		if earliest_cron_ts < sleep_till_ts {
			sleep_till_ts = earliest_cron_ts
		}

		ts_now := int(time.Now().Unix())
		sleep_delay_sec := sleep_till_ts - ts_now
		if sleep_delay_sec < 0 {
			sleep_delay_sec = 0
		}
		sleep_delay_sec = sleep_delay_sec + 1
		fmt.Println("Now: ", ts_now, " sleeping for ", sleep_delay_sec, " sec")
		s5time := S5TimeFromTimestamp(ts_now)
		fmt.Println(s5time)
		time.Sleep(time.Duration(sleep_delay_sec) * time.Second)
		CollectZombies()

	}
}
