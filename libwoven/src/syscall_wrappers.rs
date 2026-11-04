/// Type-safe syscall wrappers with compile-time checking
/// Generated from syscalls.toml - eliminates entire categories of bugs
/// 
/// Benefits over manual assembly:
/// - Type checking at compile time (wrong args = error)
/// - Result<T, E> for explicit error handling
/// - Self-documenting function signatures
/// - Zero runtime overhead (inlines to identical asm)
/// - C FFI support

pub mod syscall {
    use core::arch::asm;
    
    /// Completion port error type
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SyscallError {
        /// Syscall failed (returned u64::MAX)
        Failed,
    }

    /// Create a completion port for the current process
    /// Returns: Result<port_handle, error>
    /// Syscall #82: CompletionPortCreate
    /// Parameters: none
    pub fn completion_port_create() -> Result<u64, SyscallError> {
        let result: u64;
        unsafe {
            asm!("int 0x80",
                inlateout("rax") 82u64 => result,
                options(preserves_flags));
        }
        if result == u64::MAX { Err(SyscallError::Failed) } else { Ok(result) }
    }

    /// Close a completion port owned by the current process
    /// Parameters: port handle
    /// Returns: Result<(), error>
    /// Syscall #83: CompletionPortClose
    pub fn completion_port_close(port: u64) -> Result<(), SyscallError> {
        let result: u64;
        unsafe {
            asm!("int 0x80",
                inlateout("rax") 83u64 => result,
                in("rdi") port,
                options(preserves_flags));
        }
        if result == u64::MAX { Err(SyscallError::Failed) } else { Ok(()) }
    }

    /// Associate an async operation with a completion port
    /// Parameters: operation, port, application_cookie
    /// Returns: Result<(), error>
    /// Syscall #84: CompletionPortAssociate
    pub fn completion_port_associate(op: u64, port: u64, cookie: u64) -> Result<(), SyscallError> {
        let result: u64;
        unsafe {
            asm!("int 0x80",
                inlateout("rax") 84u64 => result,
                in("rdi") op,
                in("rsi") port,
                in("rdx") cookie,
                options(preserves_flags));
        }
        if result == u64::MAX { Err(SyscallError::Failed) } else { Ok(()) }
    }

    /// Get current process ID (always succeeds)
    /// Returns: PID > 0
    /// Syscall #4: Getpid
    pub fn getpid() -> u64 {
        let result: u64;
        unsafe {
            asm!("int 0x80",
                inlateout("rax") 4u64 => result,
                options(preserves_flags));
        }
        result
    }

    /// Get monotonic tick count since boot (always succeeds)
    /// Returns: ticks (100 Hz)
    /// Syscall #31: Getticks
    pub fn getticks() -> u64 {
        let result: u64;
        unsafe {
            asm!("int 0x80",
                inlateout("rax") 31u64 => result,
                options(preserves_flags));
        }
        result
    }

    /// Exit current process (never returns)
    /// Parameters: exit_status
    /// Syscall #3: Exit
    pub fn exit(status: i32) -> ! {
        unsafe {
            asm!("int 0x80",
                inlateout("rax") 3u64 => _,
                in("rdi") status as u64,
                options(noreturn));
        }
    }
}
