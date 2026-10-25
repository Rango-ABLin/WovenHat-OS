//! Compile the proposed VM transaction model independently of the kernel image.
#[path = "../kernel/src/vm_lease.rs"]
mod vm_lease;

#[path = "../kernel/src/vm_journal.rs"]
mod vm_journal;
