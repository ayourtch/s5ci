package main

import (
	"crypto/sha1"
	"encoding/json"
	"fmt"
	mustache "github.com/hoisie/mustache"
	"gopkg.in/yaml.v2"
	"io"
	"io/ioutil"
	"log"
	"os"
	"path/filepath"
	"reflect"
	"regexp"
	"strings"
)

func ensureDbPath(job *Job) {
	job_data_dir := JobGetDataPathFromJobId(job.Job_ID)
	err := os.MkdirAll(job_data_dir, 0755)
	if err != nil {
		log.Fatal(err)
	}
}

func SaveJobYaml(job *Job) {
	c := S5ciOptions.Config
	d, err := yaml.Marshal(job)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	// write to both places for now
	writeToFileIfDifferent(filepath.Join(c.Jobs.Rootdir, job.Job_ID, "job.yaml"), string(d))
	ensureDbPath(job)
	writeToFileIfDifferent(filepath.Join(JobGetDataPathFromJobId(job.Job_ID), "job.yaml"), string(d))
}
func SaveJobJson(job *Job) {
	c := S5ciOptions.Config
	job_lowercase := structToLowerMap(*job)
	d, err := json.Marshal(job_lowercase)
	if err != nil {
		log.Fatalf("error: %v", err)
	}
	// write to both places for now
	writeToFileIfDifferent(filepath.Join(c.Jobs.Rootdir, job.Job_ID, "job.json"), string(d))
	ensureDbPath(job)
	writeToFileIfDifferent(filepath.Join(JobGetDataPathFromJobId(job.Job_ID), "job.json"), string(d))
}

func compileTemplate(template_name string) (*mustache.Template, error) {
	c := S5ciOptions.Config
	fname := fmt.Sprintf("%s.mustache", template_name)
	full_name := filepath.Join(c.Install_Rootdir, "templates", fname)
	return mustache.ParseFile(full_name)
}

// WriteToFile will print any string of text to a file safely by
// checking for errors and syncing at the end.
func writeToFile(filename string, data string) error {
	file_dir := filepath.Dir(filename)
	os.MkdirAll(file_dir, 0755)
	file, err := os.Create(filename)
	if err != nil {
		return err
	}
	defer file.Close()

	_, err = io.WriteString(file, data)
	if err != nil {
		return err
	}
	return file.Sync()
}

var sum_exclude_regex = regexp.MustCompile(`(?s)START EXCLUDE FROM CSUM.*?END EXCLUDE FROM CSUM`)

func writeToFileIfDifferent(fname string, data string) {
	// calculate sha1 of the file
	sum_of_file := "0"
	f_data_b, err := ioutil.ReadFile(fname)
	if err == nil {
		f_data := string(f_data_b)
		h := sha1.New()
		f_data_comp := sum_exclude_regex.ReplaceAllString(f_data, "")
		io.WriteString(h, f_data_comp)
		sum_of_file = fmt.Sprintf("%x", h.Sum(nil))
	}

	// calculate the sum of bytes
	h := sha1.New()
	data_comp := sum_exclude_regex.ReplaceAllString(data, "")
	io.WriteString(h, data_comp)
	sum_of_string := fmt.Sprintf("%x", h.Sum(nil))

	if sum_of_string != sum_of_file {
		writeToFile(fname, data)
	} else {
		fmt.Println(fname, " already has the same contents. not touching")
	}

}

const jobs_per_page = 20

func RegenerateRootHtml() {
	c := S5ciOptions.Config
	template, err := compileTemplate("root_job_page")
	if err != nil {
		log.Fatal(err)
	}
	rjs := DbGetRootJobs()
	total_jobs := len(rjs)
	max_n_first_page_jobs := total_jobs%jobs_per_page + jobs_per_page
	n_first_page_jobs := total_jobs
	if total_jobs > max_n_first_page_jobs {
		n_first_page_jobs = max_n_first_page_jobs
	}

	data := make(map[string]interface{})
	if n_first_page_jobs < total_jobs {
		data_n := make(map[string]interface{})
		out_cjs := make([]map[string]interface{}, len(rjs))
		data_n["next_page_name"] = "index.html"
		for i, elem := range rjs {
			out_cjs[i] = structToLowerMap(elem)
		}
		data_n["now"] = S5Now()
		rtdt := &S5ciRuntime
		data_n["hostname"] = rtdt.Hostname
		data_n["child_jobs"] = out_cjs
		fname := filepath.Join(c.Jobs.Rootdir, "index_full.html")
		writeToFileIfDifferent(fname, template.Render(&data_n))
	}

	cjobs := rjs[0:n_first_page_jobs]

	out_cjs := make([]map[string]interface{}, len(cjobs))
	for i, elem := range cjobs {
		out_cjs[i] = structToLowerMap(elem)
	}
	if n_first_page_jobs < total_jobs {
		data["prev_page_name"] = "index_full.html"
	}
	data["child_jobs"] = out_cjs
	data["now"] = S5Now()
	rtdt := &S5ciRuntime
	data["hostname"] = rtdt.Hostname
	fname := filepath.Join(c.Jobs.Rootdir, "index.html")
	writeToFileIfDifferent(fname, template.Render(&data))
}

func RegenerateActiveHtml() {
	c := S5ciOptions.Config
	template, err := compileTemplate("active_job_page")
	if err != nil {
		log.Fatal(err)
	}
	data := make(map[string]interface{})
	cjs := DbGetActiveJobs()
	out_cjs := make([]map[string]interface{}, len(cjs))
	for i, elem := range cjs {
		out_cjs[i] = structToLowerMap(elem)
	}
	data["child_jobs"] = out_cjs
	data["now"] = S5Now()
	rtdt := &S5ciRuntime
	data["hostname"] = rtdt.Hostname
	writeToFileIfDifferent(filepath.Join(c.Jobs.Rootdir, "active.html"), template.Render(&data))
}

func RegenerateGroupHtml(group_name string) {
	c := S5ciOptions.Config
	template, err := compileTemplate("group_job_page")
	if err != nil {
		log.Fatal(err)
	}
	data := make(map[string]interface{})

	cjs := DbGetJobsByGroupName(group_name)
	out_cjs := make([]map[string]interface{}, len(cjs))
	for i, elem := range cjs {
		out_cjs[i] = structToLowerMap(elem)
	}
	data["child_jobs"] = out_cjs
	data["job_group_name"] = group_name
	data["now"] = S5Now()
	rtdt := &S5ciRuntime
	data["hostname"] = rtdt.Hostname
	writeToFileIfDifferent(filepath.Join(c.Jobs.Rootdir, group_name, "index.html"), template.Render(&data))
}

func structToLowerMap(in interface{}) map[string]interface{} {
	v := reflect.ValueOf(in)
	vType := v.Type()

	result := make(map[string]interface{}, v.NumField())

	for i := 0; i < v.NumField(); i++ {
		name := vType.Field(i).Name
		// fmt.Printf("%d: %s : %s\n", i, name, reflect.ValueOf(v.Field(i).Interface()).Kind())
		if reflect.ValueOf(v.Field(i).Interface()).Kind() == reflect.Ptr {
			if v.Field(i).IsNil() {
				result[strings.ToLower(name)] = nil
			} else {
				result[strings.ToLower(name)] = reflect.Indirect(v.Field(i)).Interface()
			}
		} else {
			result[strings.ToLower(name)] = v.Field(i).Interface()
		}
	}
	result["short_job_id"] = JobShortenJobId(result["job_id"].(string))
	result["short_instance_id"] = JobShortenInstanceId(result["instance_id"].(string))
	return result
}

func regenerateHtml(job_id string, update_parent bool, update_children bool, groups *map[string]int) {
	c := S5ciOptions.Config
	j, err := DbGetJob(job_id)
	if err != nil {
		log.Fatal(err)
	}
	template, err := compileTemplate("job_page")
	if err != nil {
		log.Fatal(err)
	}
	SaveJobYaml(j)
	SaveJobJson(j)

	data := make(map[string]interface{})

	data["job"] = structToLowerMap(*j)

	var pj *Job = nil
	if j.Parent_Job_ID != nil {
		pj, _ = DbGetJob(*j.Parent_Job_ID)
		if pj != nil {
			data["parent_job"] = structToLowerMap(*pj)
		}
	}
	cjs := DbGetChildJobs(job_id)

	out_cjs := make([]map[string]interface{}, len(cjs))
	for i, elem := range cjs {
		out_cjs[i] = structToLowerMap(elem)
	}
	data["child_jobs"] = out_cjs
	data["now"] = S5Now()
	rtdt := &S5ciRuntime
	data["hostname"] = rtdt.Hostname

	archive_dir_name := filepath.Join(c.Jobs.Rootdir, job_id, "archive")
	if s, err := os.Stat(archive_dir_name); err == nil && s.IsDir() {
		data["archive_dir"] = "archive"
	}
	writeToFileIfDifferent(filepath.Join(c.Jobs.Rootdir, job_id, "index.html"), template.Render(&data))

	if update_children {
		for _, cj := range cjs {
			regenerateHtml(cj.Job_ID, false, false, groups)
		}
	}

	if update_parent {
		if pj != nil {
			regenerateHtml(pj.Job_ID, false, false, groups)
		} else {
			RegenerateRootHtml()
		}

	}
	g := *groups
	if g[j.Job_Group_Name] > 0 {
		g[j.Job_Group_Name]++
	} else {
		g[j.Job_Group_Name] = 1
	}
}

func RegenerateJobHtml(job_id string) {
	groups := make(map[string]int)
	regenerateHtml(job_id, true, true, &groups)
	for group_name, count := range groups {
		fmt.Printf("Regenerating group %s with %d jobs\n", group_name, count)
		RegenerateGroupHtml(group_name)
	}
	RegenerateActiveHtml()
	RegenerateRootHtml()
}

func StartingJob(job_id string) {
	RegenerateJobHtml(job_id)
}

func FinishedJob(job_id string) {
	RegenerateJobHtml(job_id)
}

func RegenerateAllHtml() {
	fmt.Printf("Regenerating all jobs HTML...\n")
	jobs := DbGetAllJobs()
	groups := make(map[string]int)
	for _, j := range jobs {
		regenerateHtml(j.Job_ID, false, false, &groups)
	}
	for group_name, count := range groups {
		fmt.Printf("Regenerating group %s with %d jobs\n", group_name, count)
		RegenerateGroupHtml(group_name)
	}
	RegenerateActiveHtml()
	RegenerateRootHtml()
}
