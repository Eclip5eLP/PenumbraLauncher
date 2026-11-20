//! Starting games and knowing when they're closed. Every launched game goes into a Windows Job Object, so
//! "running" stays true while any process it spawned is alive (games that restart themselves or start a
//! helper process are tracked correctly).

use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::{Child, Command};
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JobObjectBasicAccountingInformation, QueryInformationJobObject,
};

pub struct Tracked {
    child: Child,
    job: HANDLE,
}

// SAFETY: the job handle is only used from the thread that owns `Tracked`.
unsafe impl Send for Tracked {}

impl Drop for Tracked {
    fn drop(&mut self) {
        // Closing our handle doesn't kill the game (no KILL_ON_JOB_CLOSE).
        // SAFETY: handle created by CreateJobObjectW and closed once.
        unsafe { CloseHandle(self.job) };
    }
}

pub fn launch(exe: &Path, args: &[String], cwd: &Path) -> std::io::Result<Tracked> {
    let child = Command::new(exe).args(args).current_dir(cwd).spawn()?;
    // SAFETY: anonymous job object; null security attributes and name are allowed.
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if !job.is_null() {
        // SAFETY: both handles are valid; failure (e.g. process already in a job) just means less tracking.
        let ok = unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) };
        if ok == 0 {
            tracing::warn!("could not attach game to a job object; tracking only the main process");
        }
    }
    Ok(Tracked { child, job })
}

impl Tracked {
    fn active_processes(&self) -> u32 {
        if self.job.is_null() {
            return 0;
        }
        let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: buffer is a correctly sized, writable struct for this information class.
        let ok = unsafe {
            QueryInformationJobObject(
                self.job,
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 { 0 } else { info.ActiveProcesses }
    }

    /// Blocks until the game and everything it started has exited.
    pub fn wait(mut self) {
        let _ = self.child.wait();
        while self.active_processes() > 0 {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
