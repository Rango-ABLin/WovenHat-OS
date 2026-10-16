#![no_std]

#[unsafe(no_mangle)]
pub extern "C" fn woven_answer() -> u64 {
    42
}
