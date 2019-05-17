use libc::pid_t;
pub fn mypid() -> pid_t {
    use libc::getpid;
    let pid = unsafe { getpid() };
    pid
}

pub fn kill_process(pid: i32) {
    unsafe {
        let pg_id = libc::getpgid(pid as pid_t);
        libc::kill(-pg_id, libc::SIGTERM);
    }
    // maybe call  s5ci::thread_sleep_ms() and then kill -9 ?
}

pub fn setsid() -> pid_t {
    use libc::setsid;
    let pid = unsafe { setsid() };
    pid
}

pub fn collect_zombies() -> i32 {
    mod c {
        use libc;
        extern "C" {
            pub fn waitpid(
                pid: libc::pid_t,
                status: *mut libc::c_int,
                flags: libc::c_int,
            ) -> libc::c_int;
        }
    }
    unsafe {
        let pid: i32 = -1;
        let flags: i32 = 1; // wnohang
        let mut status: libc::c_int = 0;
        let mut count = 0;
        loop {
            let ret_pid = c::waitpid(
                pid as libc::pid_t,
                &mut status as *mut libc::c_int,
                flags as libc::c_int,
            );
            if ret_pid <= 0 {
                break;
            }
            {
                eprintln!("Collected exit status from pid {}: {:?}", ret_pid, status);
                count += 1;
            }
        }
        count
    }
}
