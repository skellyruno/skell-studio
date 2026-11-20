// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in Rust real-time allocation/timing audit ported from the supplied ML Studio source.
//!
//! Instrumentation is compiled away unless the `rt-audit` Cargo feature is enabled.
//! It observes Rust's global allocator only; native plugins may allocate elsewhere.
//! Never log, format, allocate, or lock inside the measured audio scope.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum Role {
    Callback,
    Feeder,
    Worker,
}

#[cfg(not(feature = "rt-audit"))]
pub struct Scope;

#[cfg(not(feature = "rt-audit"))]
impl Scope {
    #[inline(always)]
    pub fn enter(_: Role) -> Self { Self }
}

#[cfg(feature = "rt-audit")]
pub use enabled::*;

#[cfg(feature = "rt-audit")]
mod enabled {
    use super::Role;
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
        sync::atomic::{AtomicU64, Ordering::Relaxed},
        time::Instant,
    };

    thread_local! {
        static ROLE: Cell<Option<usize>> = const { Cell::new(None) };
    }

    struct Counters {
        allocations: AtomicU64,
        frees: AtomicU64,
        calls: AtomicU64,
        max_ns: AtomicU64,
    }

    static COUNTERS: [Counters; 3] = [const {
        Counters {
            allocations: AtomicU64::new(0),
            frees: AtomicU64::new(0),
            calls: AtomicU64::new(0),
            max_ns: AtomicU64::new(0),
        }
    }; 3];

    /// Install exactly once in the opt-in binary via `#[global_allocator]`.
    /// Ordinary audio builds must keep their original allocator.
    pub struct Allocator;

    #[inline]
    fn count(free: bool) {
        let _ = ROLE.try_with(|r| {
            if let Some(index) = r.get() {
                let counter = &COUNTERS[index];
                if free {
                    counter.frees.fetch_add(1, Relaxed);
                } else {
                    counter.allocations.fetch_add(1, Relaxed);
                }
            }
        });
    }

    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            count(false);
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            count(false);
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            count(true);
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            // Count the allocation request, but only count the old block as freed when
            // System actually returns a replacement pointer. On failure, the old block
            // remains valid and must not be reported as freed.
            let replacement = unsafe { System.realloc(ptr, layout, new_size) };
            count(false);
            if !replacement.is_null() {
                count(true);
            }
            replacement
        }
    }

    /// Scope can nest; dropping restores the prior role on this thread.
    pub struct Scope {
        previous: Option<usize>,
        role: usize,
        started: Instant,
    }

    impl Scope {
        #[inline]
        pub fn enter(role: Role) -> Self {
            let role = role as usize;
            Self {
                previous: ROLE.with(|r| r.replace(Some(role))),
                role,
                started: Instant::now(),
            }
        }
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            ROLE.with(|r| r.set(self.previous));
            let counter = &COUNTERS[self.role];
            counter.calls.fetch_add(1, Relaxed);
            counter.max_ns.fetch_max(
                self.started.elapsed().as_nanos().min(u64::MAX as u128) as u64,
                Relaxed,
            );
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct Snapshot {
        pub allocations: u64,
        pub frees: u64,
        pub calls: u64,
        pub max_ns: u64,
    }

    /// Read/reset only on a control or test thread, not from audio callbacks.
    pub fn take(role: Role) -> Snapshot {
        let c = &COUNTERS[role as usize];
        Snapshot {
            allocations: c.allocations.swap(0, Relaxed),
            frees: c.frees.swap(0, Relaxed),
            calls: c.calls.swap(0, Relaxed),
            max_ns: c.max_ns.swap(0, Relaxed),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::alloc::{GlobalAlloc, Layout};

        #[test]
        fn role_scopes_restore_thread_local_roles() {
            ROLE.with(|role| role.set(None));
            {
                let _outer = Scope::enter(Role::Feeder);
                ROLE.with(|role| assert_eq!(role.get(), Some(Role::Feeder as usize)));
                {
                    let _inner = Scope::enter(Role::Worker);
                    ROLE.with(|role| assert_eq!(role.get(), Some(Role::Worker as usize)));
                }
                ROLE.with(|role| assert_eq!(role.get(), Some(Role::Feeder as usize)));
            }
            ROLE.with(|role| assert_eq!(role.get(), None));
        }

        #[test]
        fn allocator_counts_allocations_and_frees_inside_a_scope() {
            // Callback is isolated from the role used by the nesting test, so parallel test
            // execution cannot make this snapshot include unrelated scope calls.
            let _ = take(Role::Callback);
            let layout = Layout::from_size_align(8, 8).expect("valid test allocation layout");
            let mut allocated = false;
            {
                let _scope = Scope::enter(Role::Callback);
                let ptr = unsafe { Allocator.alloc(layout) };
                if !ptr.is_null() {
                    let grown_layout = Layout::from_size_align(16, 8).expect("valid grown layout");
                    let grown = unsafe { Allocator.realloc(ptr, layout, grown_layout.size()) };
                    if !grown.is_null() {
                        unsafe { Allocator.dealloc(grown, grown_layout) };
                        allocated = true;
                    }
                }
            }
            assert!(allocated, "system allocator should satisfy a small test allocation");
            let snapshot = take(Role::Callback);
            assert_eq!(snapshot.calls, 1);
            assert!(snapshot.allocations >= 2, "allocation/reallocation requests were not counted: {snapshot:?}");
            assert!(snapshot.frees >= 2, "reallocation and final deallocation were not counted: {snapshot:?}");
        }
    }
}
