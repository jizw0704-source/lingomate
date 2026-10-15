//! Owned helper processes leave an MSIX parent's private view before any state/credential access.
use crate::remote::RemoteError;
use std::fs::File;
use std::io::{Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::Win32::System::JobObjects::*;
use windows::Win32::System::Pipes::*;
use windows::Win32::System::Threading::*;
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

pub fn unpacked() -> bool {
    let mut length = 0;
    unsafe { GetCurrentPackageFullName(&mut length, None) }.0 == 15700
}
pub fn require_unpacked() -> Result<(), RemoteError> {
    if unpacked() {
        Ok(())
    } else {
        Err(RemoteError::Packaged)
    }
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}
fn pipe() -> Result<(Handle, Handle), RemoteError> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        bInheritHandle: true.into(),
        ..Default::default()
    };
    let (mut read, mut write) = (HANDLE::default(), HANDLE::default());
    unsafe { CreatePipe(&mut read, &mut write, Some(&attributes), 16384) }
        .map_err(|_| RemoteError::Unavailable)?;
    Ok((Handle(read), Handle(write)))
}
pub struct Child {
    process: Handle,
    pub id: u32,
    output: Option<File>,
    input: Option<File>,
    managed: bool,
    job: Option<Handle>,
}
impl Drop for Child {
    fn drop(&mut self) {
        if self.managed && !self.terminated() {
            self.job.take();
            unsafe {
                let _ = TerminateProcess(self.process.0, 1);
                let _ = WaitForSingleObject(self.process.0, 1000);
            }
        }
    }
}
impl Child {
    fn terminated(&self) -> bool {
        (unsafe { WaitForSingleObject(self.process.0, 0) }) == WAIT_OBJECT_0
    }
    pub fn capture(
        mut self,
        body: &[u8],
        stop: &AtomicBool,
        timeout: Duration,
    ) -> Result<Vec<u8>, RemoteError> {
        if stop.load(Ordering::Acquire) {
            return Err(RemoteError::Changed);
        }
        if body.len() > 8192 {
            return Err(RemoteError::Configuration);
        }
        if let Some(mut input) = self.input.take() {
            input.write_all(body).map_err(|_| RemoteError::Network)?;
        }
        let deadline = Instant::now() + timeout;
        let mut bytes = Vec::new();
        loop {
            if stop.load(Ordering::Acquire) || Instant::now() > deadline {
                self.job.take();
                unsafe {
                    let _ = TerminateProcess(self.process.0, 1);
                    let _ = WaitForSingleObject(self.process.0, 5000);
                }
                return Err(if stop.load(Ordering::Acquire) {
                    RemoteError::Changed
                } else {
                    RemoteError::Network
                });
            }
            let mut available = 0;
            let file = self.output.as_mut().ok_or(RemoteError::Network)?;
            let peek = unsafe {
                PeekNamedPipe(
                    HANDLE(file.as_raw_handle()),
                    None,
                    0,
                    None,
                    Some(&mut available),
                    None,
                )
            };
            if available > 0 {
                let mut buffer = [0; 4096];
                let count = file
                    .read(&mut buffer[..available.min(4096) as usize])
                    .map_err(|_| RemoteError::Network)?;
                if bytes.len() + count > 32_000 {
                    unsafe {
                        let _ = TerminateProcess(self.process.0, 1);
                    }
                    return Err(RemoteError::InvalidResponse);
                }
                bytes.extend_from_slice(&buffer[..count]);
            } else if self.terminated() || peek.is_err() {
                let mut exit = 1;
                if unsafe { GetExitCodeProcess(self.process.0, &mut exit) }.is_err() || exit != 0 {
                    return Err(RemoteError::Unavailable);
                }
                return Ok(bytes);
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
pub fn spawn(executable: &Path, args: &[String], capture: bool) -> Result<Child, RemoteError> {
    if !unpacked() {
        let mut relay = vec![
            "--relay".into(),
            if capture { "--pipe" } else { "--ui" }.into(),
        ];
        relay.extend_from_slice(args);
        return spawn_direct(executable, &relay, capture);
    }
    spawn_direct(executable, args, capture)
}
pub fn relay(args: &[String]) -> Result<(), RemoteError> {
    let (kind, args) = args.split_first().ok_or(RemoteError::Configuration)?;
    if !matches!(
        args.first().map(String::as_str),
        Some(
            "--translate"
                | "--preferences"
                | "--identity"
                | "--record-study"
                | "--settings"
                | "--prepare-memory"
        )
    ) {
        return Err(RemoteError::Configuration);
    }
    let executable = std::env::current_exe().map_err(|_| RemoteError::Unavailable)?;
    match kind.as_str() {
        "--pipe" => {
            let mut body = Vec::new();
            std::io::stdin()
                .take(8193)
                .read_to_end(&mut body)
                .map_err(|_| RemoteError::InvalidResponse)?;
            if body.len() > 8192 {
                return Err(RemoteError::InvalidResponse);
            }
            let child = spawn_direct(&executable, args, true)?;
            let stop = AtomicBool::new(false);
            let output = child.capture(&body, &stop, Duration::from_secs(30))?;
            std::io::stdout()
                .write_all(&output)
                .map_err(|_| RemoteError::Network)
        }
        "--ui" => {
            spawn_direct(&executable, args, false)?;
            Ok(())
        }
        _ => Err(RemoteError::Configuration),
    }
}
fn spawn_direct(executable: &Path, args: &[String], capture: bool) -> Result<Child, RemoteError> {
    // Only fixed switches and enum/window metadata go in arguments. Translation text uses pipes.
    if args
        .iter()
        .any(|a| a.len() > 128 || a.contains(['"', '\r', '\n', '\0']))
    {
        return Err(RemoteError::Configuration);
    }
    if !executable.is_absolute() || !executable.is_file() {
        return Err(RemoteError::Unavailable);
    }
    let application = HSTRING::from(executable.as_os_str());
    let mut command: Vec<u16> = format!("\"{}\" {}", executable.display(), args.join(" "))
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut size = 0usize;
    let _ = unsafe {
        InitializeProcThreadAttributeList(None, if capture { 2 } else { 1 }, None, &mut size)
    };
    let mut storage = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
    let list = LPPROC_THREAD_ATTRIBUTE_LIST(storage.as_mut_ptr().cast());
    unsafe {
        InitializeProcThreadAttributeList(Some(list), if capture { 2 } else { 1 }, None, &mut size)
    }
    .map_err(|_| RemoteError::Unavailable)?;
    struct Attributes(LPPROC_THREAD_ATTRIBUTE_LIST);
    impl Drop for Attributes {
        fn drop(&mut self) {
            unsafe {
                DeleteProcThreadAttributeList(self.0);
            }
        }
    }
    let _attributes = Attributes(list);
    // ENABLE_PROCESS_TREE makes descendants leave the desktop app runtime.
    // A packaged parent first starts a data-free relay; its child is the final
    // helper. OVERRIDE (4) would force the immediate child back into the package.
    let policy = 1u32;
    unsafe {
        UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_DESKTOP_APP_POLICY as usize,
            Some((&policy as *const u32).cast()),
            4,
            None,
            None,
        )
    }
    .map_err(|_| RemoteError::Packaged)?;
    let mut startup = STARTUPINFOEXW {
        StartupInfo: STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOEXW>() as u32,
            ..Default::default()
        },
        lpAttributeList: list,
    };
    let mut pipes = None;
    let mut inherit_list = Vec::<HANDLE>::new();
    if capture {
        let (input_read, input_write) = pipe()?;
        let (output_read, output_write) = pipe()?;
        unsafe { SetHandleInformation(input_write.0, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) }
            .map_err(|_| RemoteError::Unavailable)?;
        unsafe { SetHandleInformation(output_read.0, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) }
            .map_err(|_| RemoteError::Unavailable)?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            bInheritHandle: true.into(),
            ..Default::default()
        };
        let null = Handle(
            unsafe {
                CreateFileW(
                    w!("NUL"),
                    FILE_GENERIC_WRITE.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    Some(&attributes),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            }
            .map_err(|_| RemoteError::Unavailable)?,
        );
        inherit_list.extend([input_read.0, output_write.0, null.0]);
        unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(inherit_list.as_ptr().cast()),
                inherit_list.len() * std::mem::size_of::<HANDLE>(),
                None,
                None,
            )
        }
        .map_err(|_| RemoteError::Unavailable)?;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = input_read.0;
        startup.StartupInfo.hStdOutput = output_write.0;
        startup.StartupInfo.hStdError = null.0;
        pipes = Some((input_read, input_write, output_read, output_write, null));
    }
    let mut info = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            &application,
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            capture,
            EXTENDED_STARTUPINFO_PRESENT
                | CREATE_NO_WINDOW
                | if capture {
                    CREATE_SUSPENDED
                } else {
                    PROCESS_CREATION_FLAGS(0)
                },
            None,
            PCWSTR::null(),
            &startup.StartupInfo,
            &mut info,
        )
    }
    .map_err(|_| RemoteError::Unavailable)?;
    let job = if capture {
        let process = Handle(info.hProcess);
        let setup = (|| {
            let job = Handle(
                unsafe { CreateJobObjectW(None, PCWSTR::null()) }
                    .map_err(|_| RemoteError::Unavailable)?,
            );
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of_val(&limits) as u32,
                )
            }
            .map_err(|_| RemoteError::Unavailable)?;
            unsafe { AssignProcessToJobObject(job.0, process.0) }
                .map_err(|_| RemoteError::Unavailable)?;
            if unsafe { ResumeThread(info.hThread) } == u32::MAX {
                return Err(RemoteError::Unavailable);
            }
            Ok::<_, RemoteError>(job)
        })();
        match setup {
            Ok(job) => {
                std::mem::forget(process);
                Some(job)
            }
            Err(error) => {
                unsafe {
                    let _ = TerminateProcess(process.0, 1);
                    let _ = CloseHandle(info.hThread);
                }
                return Err(error);
            }
        }
    } else {
        None
    };
    let _thread = Handle(info.hThread);
    let (input, output) = if let Some((_read, write, read, _write, _null)) = pipes {
        let input = unsafe { File::from_raw_handle(write.0.0) };
        std::mem::forget(write);
        let output = unsafe { File::from_raw_handle(read.0.0) };
        std::mem::forget(read);
        (Some(input), Some(output))
    } else {
        (None, None)
    };
    Ok(Child {
        process: Handle(info.hProcess),
        id: info.dwProcessId,
        input,
        output,
        managed: capture,
        job,
    })
}

pub fn desktop_path(dll: &Path) -> Result<std::path::PathBuf, RemoteError> {
    Ok(dll
        .parent()
        .ok_or(RemoteError::Unavailable)?
        .join("lingomate-desktop.exe"))
}
