//! Windows x86-64 register capture via RtlCaptureContext / GetThreadContext.

use crate::{RegisterArchitecture, RegisterError, RegisterSnapshot, RegisterValue};
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE};
use windows::Win32::System::Diagnostics::Debug::{
    GetThreadContext, RtlCaptureContext, CONTEXT, CONTEXT_FULL_AMD64,
};
use windows::Win32::System::Threading::{
    GetCurrentThreadId, OpenThread, ResumeThread, SuspendThread, THREAD_GET_CONTEXT,
    THREAD_QUERY_INFORMATION, THREAD_SUSPEND_RESUME,
};

fn reg(name: &str, bits: u64) -> RegisterValue {
    RegisterValue::new(name, bits, 64)
}

fn reg32(name: &str, bits: u32) -> RegisterValue {
    RegisterValue::new(name, u64::from(bits), 32)
}

fn reg16(name: &str, bits: u16) -> RegisterValue {
    RegisterValue::new(name, u64::from(bits), 16)
}

fn context_to_snapshot(context: &CONTEXT, thread_id: u64) -> RegisterSnapshot {
    RegisterSnapshot {
        architecture: RegisterArchitecture::X86_64,
        thread_id,
        registers: vec![
            reg("RAX", context.Rax),
            reg("RBX", context.Rbx),
            reg("RCX", context.Rcx),
            reg("RDX", context.Rdx),
            reg("RSI", context.Rsi),
            reg("RDI", context.Rdi),
            reg("RBP", context.Rbp),
            reg("RSP", context.Rsp),
            reg("R8", context.R8),
            reg("R9", context.R9),
            reg("R10", context.R10),
            reg("R11", context.R11),
            reg("R12", context.R12),
            reg("R13", context.R13),
            reg("R14", context.R14),
            reg("R15", context.R15),
            reg("RIP", context.Rip),
            reg32("RFLAGS", context.EFlags),
            reg16("CS", context.SegCs),
            reg16("DS", context.SegDs),
            reg16("ES", context.SegEs),
            reg16("FS", context.SegFs),
            reg16("GS", context.SegGs),
            reg16("SS", context.SegSs),
        ],
    }
}

pub(super) fn capture_current() -> Result<RegisterSnapshot, RegisterError> {
    let mut context = Box::new(CONTEXT {
        ContextFlags: CONTEXT_FULL_AMD64,
        ..Default::default()
    });
    // SAFETY: RtlCaptureContext writes a complete CONTEXT for the calling thread.
    unsafe {
        RtlCaptureContext(context.as_mut());
    }
    // SAFETY: GetCurrentThreadId is a pure kernel32 query with no pointer args.
    let thread_id = u64::from(unsafe { GetCurrentThreadId() });
    Ok(context_to_snapshot(context.as_ref(), thread_id))
}

pub(super) fn capture_thread(
    _process_id: u64,
    thread_id: u64,
) -> Result<RegisterSnapshot, RegisterError> {
    let tid = u32::try_from(thread_id)
        .map_err(|_| RegisterError::Unavailable("Thread id does not fit Windows DWORD".into()))?;
    let access = THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION | THREAD_SUSPEND_RESUME;
    let handle = match unsafe { OpenThread(access, false, tid) } {
        Ok(handle) => handle,
        Err(error) if error.code() == ERROR_ACCESS_DENIED.into() => {
            return Err(RegisterError::PermissionRequired(format!(
                "OpenThread({tid}) denied: {error}"
            )));
        }
        Err(error) => {
            return Err(RegisterError::Unavailable(format!(
                "OpenThread({tid}) failed: {error}"
            )));
        }
    };
    let result = capture_with_handle(handle, thread_id);
    let _ = unsafe { CloseHandle(handle) };
    result
}

fn capture_with_handle(handle: HANDLE, thread_id: u64) -> Result<RegisterSnapshot, RegisterError> {
    let suspended = unsafe { SuspendThread(handle) };
    if suspended == u32::MAX {
        return Err(RegisterError::Unavailable("SuspendThread failed".into()));
    }
    let mut context = Box::new(CONTEXT {
        ContextFlags: CONTEXT_FULL_AMD64,
        ..Default::default()
    });
    let got = unsafe { GetThreadContext(handle, context.as_mut()) };
    let _ = unsafe { ResumeThread(handle) };
    match got {
        Ok(()) => Ok(context_to_snapshot(context.as_ref(), thread_id)),
        Err(error) if error.code() == ERROR_ACCESS_DENIED.into() => Err(
            RegisterError::PermissionRequired(format!("GetThreadContext denied: {error}")),
        ),
        Err(error) => Err(RegisterError::Unavailable(format!(
            "GetThreadContext failed: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_returns_x86_64_gprs_with_nonzero_rip_or_rsp() {
        let snapshot = capture_current().unwrap();
        assert_eq!(snapshot.architecture, RegisterArchitecture::X86_64);
        let names: Vec<_> = snapshot
            .registers
            .iter()
            .map(|reg| reg.name.as_str())
            .collect();
        for expected in ["RAX", "RBX", "RSP", "RIP", "RFLAGS"] {
            assert!(names.contains(&expected), "missing {expected}");
        }
        let rsp = snapshot
            .registers
            .iter()
            .find(|reg| reg.name == "RSP")
            .unwrap()
            .bits;
        let rip = snapshot
            .registers
            .iter()
            .find(|reg| reg.name == "RIP")
            .unwrap()
            .bits;
        assert!(rsp != 0 || rip != 0);
    }
}
