#[cfg(windows)]
mod windows {
    use std::{io, mem::size_of};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            JobObjects::{AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE},
            Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE},
        },
    };

    pub struct ProcessTree(HANDLE);
    // A job handle has no thread affinity; all access is protected by Session's IO mutex.
    unsafe impl Send for ProcessTree {}

    impl ProcessTree {
        pub fn attach(pid: u32) -> Result<Self, String> {
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() { return Err(io::Error::last_os_error().to_string()); }
                let owner = Self(job);
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(job, JobObjectExtendedLimitInformation, &limits as *const _ as *const _, size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32) == 0 {
                    return Err(io::Error::last_os_error().to_string());
                }
                let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
                if process.is_null() { return Err(io::Error::last_os_error().to_string()); }
                let assigned = AssignProcessToJobObject(job, process);
                let error = io::Error::last_os_error();
                CloseHandle(process);
                if assigned == 0 { return Err(format!("Cannot own worker process tree: {error}")); }
                Ok(owner)
            }
        }

        pub fn kill(&self) -> Result<(), String> {
            if unsafe { TerminateJobObject(self.0, 1) } == 0 {
                Err(io::Error::last_os_error().to_string())
            } else { Ok(()) }
        }
    }

    impl Drop for ProcessTree {
        fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
    }
}

#[cfg(windows)]
pub use windows::ProcessTree;

#[cfg(not(windows))]
pub struct ProcessTree;

#[cfg(not(windows))]
impl ProcessTree {
    pub fn attach(_: u32) -> Result<Self, String> { Ok(Self) }
}
