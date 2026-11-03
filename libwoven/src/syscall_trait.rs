/// Type-safe syscall trait
pub trait Syscalls {
    fn completion_port_create() -> u64;
    fn completion_port_close(port: u64) -> i32;
    fn completion_port_associate(op: u64, port: u64, cookie: u64) -> i32;
    fn async_create(class: u32) -> u64;
    fn getpid() -> u64;
    fn getticks() -> u64;
    fn exit(status: i32) -> !;
}
