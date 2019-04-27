var console_data = "";
var console_last_start = 0;
var console_url = "";
var console_element_id = "console";
var reload_timeout = false;

function reqListener () {
	var d = document.getElementById(console_element_id);
	var resp = this.responseText;
	if (this.status < 400) {
	        if (reload_timeout && console_data != "") {
	    	   window.clearTimeout(reload_timeout);
		   reload_timeout = window.setTimeout(function(){ location.reload(true); }, 5000);
	        }
		console_data = console_data + resp;
		// console.log("response len: " + resp.length);
		// console.log("console_data len: " + console_data.length);
		var output_text = this.responseText.replace(/</g,"&lt;").replace(/>/g,"&gt;");
		ch = document.createElement("p")
		ch.style.fontsize = "10px";
		ch2 = document.createElement("div")
		ch.innerHTML = output_text;
		ch2.innerHTML = "";
		d.appendChild(ch);
		d.appendChild(ch2);
	        ch2.scrollIntoView();
	}
	setTimeout(function(){ load_some(); }, 1000);

}

function load_some() {
	var oReq = new XMLHttpRequest();
	oReq.addEventListener("load", reqListener);
	var get_url = console_url;
	oReq.open("GET", get_url);
	var start_len = console_data.length;
	if (start_len > 0) {
	  var range = "" + start_len + "-";
	  oReq.setRequestHeader("Range", "bytes=" + range);
	  if (console_last_start != start_len) {
		  console_last_start = start_len;
	          // console.log(range);
	  }
	}
	oReq.send();
}

function start_console(url, elementname) {
	console_data = "";
	console_last_start = 0;
	console_url = url;
	console_element_id = elementname;
	load_some();
	reload_timeout = window.setTimeout(function(){ location.reload(true); }, 20000);
}

