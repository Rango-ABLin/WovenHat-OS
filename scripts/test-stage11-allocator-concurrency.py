"""Compile and exercise the actual Stage 11 allocator body with a host mmap stub.

The Rust body is extracted from libwoven without maintaining a divergent
copy. Only the freestanding kernel syscall expression is substituted.
"""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]

def main():
    source = (ROOT / "libwoven/src/lib.rs").read_text(encoding="utf-8")
    start = source.index("mod userspace_allocator {")
    finish = source.index("\npub mod process {", start)
    body = source[start:finish]
    needle = "unsafe { crate::syscall::invoke(8, ARENA_BYTES, 1, 0) }"
    assert body.count(needle) == 1
    body = body.replace(needle, "crate::mock_mmap()")
    body = body.replace("#[global_allocator]", "")
    body = body.replace("    static ALLOCATOR: Arena = Arena;", """    static ALLOCATOR: Arena = Arena;
    pub(super) fn allocate(n: usize) -> usize {
        // SAFETY: the test keeps the backing mapping alive for its duration.
        unsafe { ALLOCATOR.alloc(Layout::from_size_align(n, 16).unwrap()) as usize }
    }
    pub(super) fn initialized() -> bool {
        NEXT.load(Ordering::Acquire) != u64::MAX
            && END.load(Ordering::Acquire) > 0
    }
""")
    program = """
use std::sync::atomic::{AtomicUsize, Ordering};
static MMAP_CALLS: AtomicUsize = AtomicUsize::new(0);
static mut BACKING: [u128; 4096] = [0; 4096];
fn mock_mmap() -> u64 {
    MMAP_CALLS.fetch_add(1, Ordering::SeqCst);
    core::ptr::addr_of_mut!(BACKING) as *mut u128 as u64
}
""" + body + """
#[test]
fn concurrent_allocations_publish_once_and_do_not_overlap() {
    let threads: Vec<_> = (0..32).map(|_| std::thread::spawn(|| {
        let p = userspace_allocator::allocate(64);
        assert_ne!(p, 0);
        p
    })).collect();
    let mut pointers: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    pointers.sort_unstable();
    assert!(pointers.windows(2).all(|w| w[1] - w[0] >= 64));
    assert_eq!(MMAP_CALLS.load(Ordering::SeqCst), 1);
    assert!(userspace_allocator::initialized());
    assert_eq!(userspace_allocator::allocate(65536), 0);
}
"""
    with tempfile.TemporaryDirectory() as temp:
        directory = Path(temp)
        fixture = directory / "allocator.rs"
        fixture.write_text(program, encoding="utf-8")
        executable = directory / "allocator-test"
        subprocess.run(["rustc", "--edition=2024", "--test", str(fixture), "-o", str(executable)], check=True)
        subprocess.run([str(executable)], check=True)

if __name__ == "__main__":
    main()
