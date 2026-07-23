//! Unified SIMD dispatch table (x86_64 only).
//!
//! Replaces per-function `cpufeatures` cascades with a single dispatch
//! table, initialized once via an atomic state machine.  All three SIMD
//! dispatch points (`simd_skip_equal`, `simd_skip_while_digit`,
//! `simd_is_ascii`) read from the same table, eliminating redundant
//! CPUID calls and branch-heavy cascades on every comparison.

#![cfg(all(target_arch = "x86_64", not(kani)))]

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU8, Ordering};

use super::is_ascii::{
    simd_is_ascii_avx2, simd_is_ascii_avx512, simd_is_ascii_sse2, simd_is_ascii_sse41,
};
use super::skip_equal::{skip_avx2, skip_avx512, skip_sse2, skip_sse41, skip_sse42};
use super::skip_while_digit::{
    skip_while_digit_avx2, skip_while_digit_avx512, skip_while_digit_sse2,
};

// One cpufeatures macro per unique feature — used only during the
// one-time init below, never in the hot dispatch path.
cpufeatures::new!(cpuid_avx512, "avx512f", "avx512bw");
cpufeatures::new!(cpuid_avx2, "avx2");
cpufeatures::new!(cpuid_sse42, "sse4.2");
cpufeatures::new!(cpuid_sse41, "sse4.1");

const DISPATCH_UNINIT: u8 = 0;
const DISPATCH_LOCKED: u8 = 1;
const DISPATCH_DONE: u8 = 2;

pub(crate) struct Dispatch {
    pub(crate) skip_equal: unsafe fn(&[u8], &[u8], usize, usize) -> usize,
    pub(crate) skip_while_digit: unsafe fn(&[u8], usize) -> usize,
    pub(crate) is_ascii: unsafe fn(&[u8]) -> bool,
}

/// Wraps `UnsafeCell<MaybeUninit<Dispatch>>` so we can implement `Sync`.
struct DispatchCell(UnsafeCell<MaybeUninit<Dispatch>>);

// SAFETY: The state machine guards access — only one writer (LOCKED),
// readers only proceed after the Release store of DISPATCH_DONE.
unsafe impl Sync for DispatchCell {}

static DISPATCH_STATE: AtomicU8 = AtomicU8::new(DISPATCH_UNINIT);
static DISPATCH_VALUE: DispatchCell = DispatchCell(UnsafeCell::new(MaybeUninit::uninit()));

#[cold]
fn init_dispatch_lock() -> &'static Dispatch {
    if DISPATCH_STATE
        .compare_exchange(
            DISPATCH_UNINIT,
            DISPATCH_LOCKED,
            Ordering::Acquire,
            Ordering::Acquire,
        )
        .is_ok()
    {
        // We hold the lock — compute the dispatch table using cpufeatures.
        let avx512 = cpuid_avx512::get();
        let avx2 = cpuid_avx2::get();
        let sse42 = cpuid_sse42::get();
        let sse41 = cpuid_sse41::get();
        let d = Dispatch {
            skip_equal: if avx512 {
                skip_avx512
            } else if avx2 {
                skip_avx2
            } else if sse42 {
                skip_sse42
            } else if sse41 {
                skip_sse41
            } else {
                skip_sse2
            },
            skip_while_digit: if avx512 {
                skip_while_digit_avx512
            } else if avx2 {
                skip_while_digit_avx2
            } else {
                skip_while_digit_sse2
            },
            is_ascii: if avx512 {
                simd_is_ascii_avx512
            } else if avx2 {
                simd_is_ascii_avx2
            } else if sse41 {
                simd_is_ascii_sse41
            } else {
                simd_is_ascii_sse2
            },
        };
        unsafe {
            (*DISPATCH_VALUE.0.get()).write(d);
        }
        DISPATCH_STATE.store(DISPATCH_DONE, Ordering::Release);
    } else {
        // Another thread is initializing — spin until done.
        while DISPATCH_STATE.load(Ordering::Acquire) != DISPATCH_DONE {
            core::hint::spin_loop();
        }
    }
    unsafe { (*DISPATCH_VALUE.0.get()).assume_init_ref() }
}

/// Return the initialized dispatch table, initializing it on first call.
#[inline]
pub(crate) fn get_dispatch() -> &'static Dispatch {
    // Fast path: one acquire-load.
    if DISPATCH_STATE.load(Ordering::Acquire) == DISPATCH_DONE {
        unsafe {
            return (*DISPATCH_VALUE.0.get()).assume_init_ref();
        }
    }
    init_dispatch_lock()
}
