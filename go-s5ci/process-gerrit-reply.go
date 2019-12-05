package main

import (
	"bufio"
	// "errors"
	"encoding/json"
	"fmt"
	// "github.com/jessevdk/go-flags"
	// "github.com/vito/twentythousandtonnesofcrudeoil"
	"github.com/jinzhu/copier"
	"io/ioutil"
	"log"
	"os"
	"regexp"
	"strconv"
	"strings"
	"time"
)

// readLines reads a whole file into memory
// and returns a slice of its lines.
func readLines(path string) ([]string, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	var lines []string
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		lines = append(lines, scanner.Text())
	}
	return lines, scanner.Err()
}

// writeLines writes the lines to the given file.
func writeLines(lines []string, path string) error {
	file, err := os.Create(path)
	if err != nil {
		return err
	}
	defer file.Close()

	w := bufio.NewWriter(file)
	for _, line := range lines {
		fmt.Fprintln(w, line)
	}
	return w.Flush()
}

type GerritQueryStats struct {
	Type                string
	RowCount            int
	RunTimeMilliseconds int
	MoreChanges         bool
}

type GerritPerson struct {
	Name     string
	Email    string
	Username string
}

type GerritComment struct {
	Timestamp int
	Reviewer  GerritPerson
	Message   string
}

type GerritPatchSet struct {
	Number    int
	Revision  string
	Parents   []string
	Ref       string
	Uploader  GerritPerson
	CreatedOn int
	Author    GerritPerson
	IsDraft   bool
	Kind      string
	// Approvals []GerritApproval
}

type GerritChangeSet struct {
	Project       *string
	Branch        *string
	Id            string
	Number        int
	Subject       string
	Owner         GerritPerson
	Url           string
	CommitMessage string
	CreatedOn     int
	LastUpdated   *int
	Open          bool
	Status        string
	Comments      []GerritComment
	PatchSets     []GerritPatchSet
}

type CommentTriggerMatch struct {
	CommentIndex int
	TriggerName  string
	PatchsetID   *int
	Captures     map[string]string
	IsSuppress   bool
	IsSuppressed bool
}

type ProcessGerritReplyCommand struct {
	InputFile       string `short:"i" long:"input-file" required:"true" description:"input text file as sent by gerrit"`
	BeforeTimestamp int    `short:"b" long:"before-ts" description:"timestamp for 'before' edge of the range"`
	AfterTimestamp  int    `short:"a" long:"after-ts" description:"timestamp for 'after' edge of the range"`
}

type GerritSshResult struct {
	changes []GerritChangeSet
	stats   GerritQueryStats
}

func GerritParsePollCommandReply(reply_data string) (GerritSshResult, error) {
	reply_lines := strings.Split(strings.TrimSpace(reply_data), "\n")
	// changes := make([]GerritChange{})
	ret := GerritSshResult{}

	for i, line := range reply_lines {
		if i+1 == len(reply_lines) {
			var stats GerritQueryStats
			var result map[string]interface{}

			json.Unmarshal([]byte(line), &result)
			json.Unmarshal([]byte(line), &stats)
			// fmt.Println("PARSED:", stats)
			// fmt.Println("UNPARSED:", result)
			ret.stats = stats
		} else {
			var change GerritChangeSet
			a1 := json.Unmarshal([]byte(line), &change)
			if change.Id != "" {
				fmt.Println(a1, " PARSED: ", change.Id, " ", change.Project, " ", change.Branch, " ", change.Number, " ")
				ret.changes = append(ret.changes, change)
			}
			// fmt.Println("PARSED:", change)
		}
		// fmt.Println(line)
	}
	return ret, nil

}

var RE_PATCHSET *regexp.Regexp = regexp.MustCompile(`(?s)^Patch Set \d+\s*:\s*`)

func SafeOrUnderscore(rtdt *S5ciRuntimeData, val string) string {
	str := rtdt.UnsafeCharRegex.ReplaceAllString(val, "_")
	return rtdt.UnsafeStartRegex.ReplaceAllString(str, "_")
}

func RegexpCaptures(r *regexp.Regexp, str string) map[string]string {
	ssm := r.FindStringSubmatch(str)
	if ssm == nil {
		return nil
	}
	out := make(map[string]string)
	for i, name := range r.SubexpNames() {
		out[name] = ssm[i]
	}
	return out
}

func getCommentTriggerMatchesFromComments(c *S5ciConfig, rtdt *S5ciRuntimeData, changeset_id int, max_pset int, comments_vec []GerritComment, last_seen_comment_id int, then_now_ts int) ([]CommentTriggerMatch, *int, int) {
	out := make([]CommentTriggerMatch, 0)

	trigger_regexes := &rtdt.TriggerRegexes

	var out_ts *int = nil

	log.Printf("Last seen comment %d", last_seen_comment_id)
	for i, comment := range comments_vec {
		safe_patchset_str := ""
		if i <= last_seen_comment_id {
			log.Printf("Comment %d already seen", i)
			continue
		}
		/*
		 * a 10-minute safety interval: if we rebuilt the database,
		 * we might pick up old comments otherwise
		 */
		if comment.Timestamp < then_now_ts-600 {
			log.Printf("Comment %d older than 10 minutes, ignore", i)
			continue
		}

		last_seen_comment_id = i
		log.Printf("Comment %d: ts %d: %s", i, comment.Timestamp, comment.Message)
		caps := RegexpCaptures(rtdt.PatchsetExtractRegex, comment.Message)
		if caps != nil {
			if val, ok := caps["patchset"]; ok {
				safe_patchset_str = SafeOrUnderscore(rtdt, val)
			}
		}
		for _, tr := range *trigger_regexes {
			rexpr := tr.R
			is_suppress := false
			if rexpr.MatchString(comment.Message) {
				captures := make(map[string]string)
				captures["_"] = RE_PATCHSET.ReplaceAllString(comment.Message, "")
				ssm := rexpr.FindStringSubmatch(comment.Message)
				for i, name := range rexpr.SubexpNames() {
					safe_val := SafeOrUnderscore(rtdt, ssm[i])
					captures[name] = safe_val
				}
				if captures["patchset"] != "" {
					safe_patchset_str = captures["patchset"]
				}
				patchset_id, err := strconv.Atoi(safe_patchset_str)
				if err != nil {
					if strings.HasPrefix(comment.Message, "Change has been successfully merged by ") {
						patchset_id = max_pset
						captures["patchset"] = fmt.Sprintf("%d", patchset_id)
					} else {
						log.Fatal("unparseable patchset in ", comment, " = ", safe_patchset_str)
					}
				}
				trigger_name := tr.Name
				trig := CommentTriggerMatch{
					CommentIndex: i,
					TriggerName:  trigger_name,
					Captures:     captures,
					PatchsetID:   &patchset_id,
					IsSuppress:   is_suppress,
					IsSuppressed: false,
				}
				out = append(out, trig)
			}
			if tr.RSuppress == nil {
				continue
			}
			rexpr = tr.RSuppress
			is_suppress = true
			if rexpr.MatchString(comment.Message) {
				captures := make(map[string]string)
				captures["_"] = RE_PATCHSET.ReplaceAllString(comment.Message, "")
				ssm := rexpr.FindStringSubmatch(comment.Message)
				for i, name := range rexpr.SubexpNames() {
					safe_val := SafeOrUnderscore(rtdt, ssm[i])
					captures[name] = safe_val
				}
				patchset_id, err := strconv.Atoi(safe_patchset_str)
				if err != nil {
					if strings.HasPrefix(comment.Message, "Change has been successfully merged by ") {
						patchset_id = max_pset
						captures["patchset"] = fmt.Sprintf("%d", patchset_id)
					} else {
						log.Fatal("unparseable patchset in ", comment, " = ", safe_patchset_str)
					}
				}
				trigger_name := tr.Name
				trig := CommentTriggerMatch{
					CommentIndex: i,
					TriggerName:  trigger_name,
					Captures:     captures,
					PatchsetID:   &patchset_id,
					IsSuppress:   is_suppress,
					IsSuppressed: false,
				}
				out = append(out, trig)
			}

		}

	}
	return out, out_ts, last_seen_comment_id

}

func GerritProcessChange(c *S5ciConfig, rtdt *S5ciRuntimeData, cs GerritChangeSet, then_now_ts int) *int {
	// triggers := make([]CommentTriggerMatch, 0)

	max_pset := 0
	var out_ts *int = nil

	log.Printf("process change now ts: %d", then_now_ts)

	psmap := make(map[string]GerritPatchSet)

	for _, pset := range cs.PatchSets {
		if pset.CreatedOn > 0 {
			log.Printf("%d revision: %s ref %s", pset.Number, pset.Revision, pset.Ref)
		}
		psmap[fmt.Sprintf("%d", pset.Number)] = pset
		psmap[fmt.Sprintf("%s", pset.Revision)] = pset
		if pset.Number > max_pset {
			max_pset = pset.Number
		}
	}

	if len(cs.Comments) > 0 {
		change_id := cs.Number
		if then_now_ts == -1 {
			log.Printf("Resetting the last seen comment id")
			DbSetChangesetLastComment(change_id, -1)
		}
		last_seen_comment_id := DbGetChangesetLastComment(change_id)
		all_triggers, trigger_out_ts, new_last_seen_comment_id := getCommentTriggerMatchesFromComments(c, rtdt, change_id, max_pset, cs.Comments, last_seen_comment_id, then_now_ts)
		log.Printf("change_id %d last_seen_comment_id: %d => %d", change_id, last_seen_comment_id, new_last_seen_comment_id)

		out_ts = trigger_out_ts
		final_triggers := all_triggers
		suppress_map := make(map[string]bool)
		for ai, _ := range final_triggers {
			i := len(final_triggers) - 1 - ai
			ctrig := &final_triggers[i]
			key := fmt.Sprintf("%s-%d", ctrig.TriggerName, *ctrig.PatchsetID)
			log.Printf("Key: %s", key)
			YamlDump(ctrig)
			if ctrig.IsSuppress {
				log.Printf("Adding suppress with key %s on trigger %d", key, i)
				suppress_map[key] = true
			} else if suppress_map[key] {
				(*ctrig).IsSuppressed = true
				log.Printf("Using suppress with key %s on trigger %d", key, i)
				delete(suppress_map, key)
			}
		}

		if len(c.Comment_Triggers) > 0 {
			final_triggers_out := final_triggers[:0]
			// YamlDump(cs)
			YamlDump(final_triggers)
			for _, x := range final_triggers {
				ctrig := c.Comment_Triggers[x.TriggerName]
				// YamlDump(ctrig)
				retain := !x.IsSuppressed
				if ctrig.Project != nil {
					if cs.Project != nil {
						if *ctrig.Project != *cs.Project {
							log.Printf("ctrig project '%s' != cs project '%s'", *ctrig.Project, *cs.Project)
							retain = false
						}
					} else {

						retain = false
					}
				}
				if ctrig.Branch != nil {
					if cs.Branch != nil {
						if *ctrig.Branch != *cs.Branch {
							log.Printf("ctrig branch '%s' != cs branch '%s'", *ctrig.Branch, *cs.Branch)
							retain = false
						}
					} else {
						retain = false
					}
				}

				if retain {
					final_triggers_out = append(final_triggers_out, x)
				}
			}
			final_triggers = final_triggers_out
			// now purge the suppressing triggers themselves
			final_triggers_out = final_triggers[:0]
			for _, x := range final_triggers {
				if !x.IsSuppress {
					final_triggers_out = append(final_triggers_out, x)
				}
			}
			final_triggers = final_triggers_out

			/* now retain only the triggers that are old enough, and rollback the last_seen_comment_id appropriately  */
			final_triggers_out = final_triggers[:0]
			startline_ts := then_now_ts - c.Default_Regex_Trigger_Delay_Sec
			for _, x := range final_triggers {
				comment := cs.Comments[x.CommentIndex]
				if comment.Timestamp > startline_ts {
					log.Printf("Trigger %s = Comment %d before the startline by %d sec", x.TriggerName, x.CommentIndex, comment.Timestamp-startline_ts)
					x.IsSuppress = true
				}
				if x.IsSuppress {
					if x.CommentIndex <= new_last_seen_comment_id {
						new_last_seen_comment_id = x.CommentIndex - 1
						log.Printf("Comment %d is a suppress comment, new comment id is %d", x.CommentIndex, new_last_seen_comment_id)
					}
					if trigger_out_ts != nil && comment.Timestamp < *trigger_out_ts {
						*trigger_out_ts = comment.Timestamp
					}
				} else {
					final_triggers_out = append(final_triggers_out, x)
				}
			}
			final_triggers = final_triggers_out
		}

		log.Println("setting new last seen comment id to ", new_last_seen_comment_id)
		DbSetChangesetLastComment(change_id, new_last_seen_comment_id)
		YamlDump(final_triggers)
		for _, trig := range final_triggers {
			template := rtdt.TriggerCommandTemplates[trig.TriggerName]
			log.Println("Trigger command template: ", template)
			data := make(map[string]interface{})
			if trig.PatchsetID != nil {
				ps := psmap[fmt.Sprintf("%d", *trig.PatchsetID)]
				psdata := make(map[string]interface{})
				psdata["number"] = ps.Number
				psdata["revision"] = ps.Revision
				psdata["parents"] = ps.Parents
				psdata["ref"] = ps.Ref
				uploader := make(map[string]interface{})
				uploader["name"] = ps.Uploader.Name
				uploader["email"] = ps.Uploader.Email
				uploader["username"] = ps.Uploader.Username
				psdata["uploader"] = uploader

				psdata["createdon"] = ps.CreatedOn
				author := make(map[string]interface{})
				author["name"] = ps.Author.Name
				author["email"] = ps.Author.Email
				author["username"] = ps.Author.Username
				psdata["author"] = author
				psdata["isdraft"] = ps.IsDraft
				psdata["kind"] = ps.Kind
				data["patchset"] = psdata
			}
			data["regex"] = trig.Captures
			expanded_command := template.Render(&data)
			rtdt2 := S5ciRuntimeData{}
			copier.Copy(&rtdt2, rtdt)
			rtdt2.ChangesetID = change_id
			rtdt2.PatchsetID = *trig.PatchsetID
			rtdt2.TriggerEventID = fmt.Sprintf("%s_ch%d_ps%d_cmt%d", trig.TriggerName, change_id, *trig.PatchsetID, trig.CommentIndex)
			rtdt2.CommentValue = string(trig.Captures["_"])
			if trig.IsSuppress || trig.IsSuppressed {
				log.Fatal("job not runnable", trig)
			}
			log.Printf("running job: %s", expanded_command)
			JobSpawnCommand(c, &rtdt2, expanded_command)
		}
	}
	return out_ts
}

type S5SshResult struct {
	BeforeTS *int
	AfterTS  *int
	Output   string
	Changes  []GerritChangeSet
	Stats    GerritQueryStats
}

func ParseGerritPollCommandReply(c *S5ciConfig, rtdt *S5ciRuntimeData, before_ts *int, after_ts *int, command_reply string) (*S5SshResult, error) {
	ts_now := UnixTimeNow()
	var ret_after_ts *int = &ts_now
	var ret_before_ts *int = nil

	last_ts := ts_now

	res, err := GerritParsePollCommandReply(command_reply)
	if err != nil {
		log.Printf("ParseGerritPollCommandReply error: %v", err)
		return nil, err
	}
	for _, cs := range res.changes {
		if cs.LastUpdated != nil {
			log.Printf("Change %s #%d", cs.Id, cs.Number)
			if *cs.LastUpdated < last_ts {
				log.Printf("   last updated: %d", *cs.LastUpdated)
				last_ts = *cs.LastUpdated
			}

		}
	}
	if res.stats.MoreChanges {
		ret_before_ts = &last_ts
	}
	out := S5SshResult{
		BeforeTS: ret_before_ts,
		AfterTS:  ret_after_ts,
		Output:   command_reply,
		Changes:  res.changes,
		Stats:    res.stats}
	return &out, nil
}

func (cmd *ProcessGerritReplyCommand) Execute(args []string) error {
	/*
			if cmd.Command == "" {
		           var ErrShowHelpMessage = errors.New("need a command to run")
		           return ErrShowHelpMessage
			}
			fmt.Println("Command: ", cmd.Command)
	*/

	c := S5ciOptions.Config

	buf, err := ioutil.ReadFile(cmd.InputFile)
	if err != nil {
		log.Fatalf("readLines: %s", err)
	}
	res, err := GerritParsePollCommandReply(string(buf))
	fmt.Println("RESULT n_changes:", len(res.changes))

	one_change := res.changes[0]
	ts_now := int(time.Now().Unix())
	fmt.Println("Now: ", ts_now)
	s5time := S5TimeFromTimestamp(ts_now)
	fmt.Println(s5time)
	GerritProcessChange(&c, &S5ciRuntime, one_change, cmd.AfterTimestamp)
	return nil // ErrShowHelpMessage
}
