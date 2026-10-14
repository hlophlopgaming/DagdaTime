pub trait MemoryReclaimer {
    fn reclaim(&self);
}
pub struct SystemReclaimer;
impl MemoryReclaimer for SystemReclaimer {
    fn reclaim(&self) {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        // glibc may retain freed UI allocations in thread arenas after renderer destruction.
        unsafe {
            libc::malloc_trim(0);
        }
    }
}
