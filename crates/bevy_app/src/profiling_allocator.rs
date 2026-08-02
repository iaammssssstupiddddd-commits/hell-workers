//! `profiling-memory` 専用のmeasure区間allocator計数。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[global_allocator]
static GLOBAL_ALLOCATOR: MeasuringAllocator = MeasuringAllocator;

static TELEMETRY_LOCK: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static BASELINE_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static PEAK_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static ALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static REALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static ACCOUNTING_ERRORS: AtomicU64 = AtomicU64::new(0);

struct MeasuringAllocator;

// SAFETY: `System` satisfies `GlobalAlloc`; the wrapper only records atomic
// counters after successful allocations and before returning to the caller.
unsafe impl GlobalAlloc for MeasuringAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller upholds `GlobalAlloc::alloc`'s layout contract.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller upholds `GlobalAlloc::alloc_zeroed`'s layout contract.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The caller guarantees that `pointer` and `layout` describe a
        // live allocation returned by this allocator.
        unsafe { System.dealloc(pointer, layout) };
        record_deallocation(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: The caller upholds `GlobalAlloc::realloc`'s pointer, layout,
        // and new-size contract.
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            record_reallocation(layout.size(), new_size);
        }
        new_pointer
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MemoryMeasurement {
    pub(crate) baseline_live_bytes: u64,
    pub(crate) peak_live_bytes: u64,
    pub(crate) final_live_bytes: u64,
    pub(crate) allocated_bytes: u64,
    pub(crate) deallocated_bytes: u64,
    pub(crate) allocation_calls: u64,
    pub(crate) deallocation_calls: u64,
    pub(crate) reallocation_calls: u64,
    pub(crate) accounting_errors: u64,
}

pub(crate) fn begin_measurement() {
    lock_telemetry();
    ACTIVE.store(false, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    DEALLOCATED_BYTES.store(0, Ordering::Relaxed);
    ALLOCATION_CALLS.store(0, Ordering::Relaxed);
    DEALLOCATION_CALLS.store(0, Ordering::Relaxed);
    REALLOCATION_CALLS.store(0, Ordering::Relaxed);
    ACCOUNTING_ERRORS.store(0, Ordering::Relaxed);
    let baseline = LIVE_BYTES.load(Ordering::Relaxed);
    BASELINE_LIVE_BYTES.store(baseline, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(baseline, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::Relaxed);
    unlock_telemetry();
}

pub(crate) fn end_measurement() -> MemoryMeasurement {
    lock_telemetry();
    ACTIVE.store(false, Ordering::Relaxed);
    let final_live_bytes = LIVE_BYTES.load(Ordering::Relaxed);
    let peak_live_bytes = PEAK_LIVE_BYTES
        .load(Ordering::Relaxed)
        .max(final_live_bytes);
    let measurement = MemoryMeasurement {
        baseline_live_bytes: BASELINE_LIVE_BYTES.load(Ordering::Relaxed),
        peak_live_bytes,
        final_live_bytes,
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        deallocated_bytes: DEALLOCATED_BYTES.load(Ordering::Relaxed),
        allocation_calls: ALLOCATION_CALLS.load(Ordering::Relaxed),
        deallocation_calls: DEALLOCATION_CALLS.load(Ordering::Relaxed),
        reallocation_calls: REALLOCATION_CALLS.load(Ordering::Relaxed),
        accounting_errors: ACCOUNTING_ERRORS.load(Ordering::Relaxed),
    };
    unlock_telemetry();
    measurement
}

fn record_allocation(size: usize) {
    let bytes = u64::try_from(size).unwrap_or(u64::MAX);
    lock_telemetry();
    let live_bytes = checked_add(&LIVE_BYTES, bytes);
    if ACTIVE.load(Ordering::Relaxed) {
        checked_add(&ALLOCATED_BYTES, bytes);
        checked_add(&ALLOCATION_CALLS, 1);
        PEAK_LIVE_BYTES.fetch_max(live_bytes, Ordering::Relaxed);
    }
    unlock_telemetry();
}

fn record_deallocation(size: usize) {
    let bytes = u64::try_from(size).unwrap_or(u64::MAX);
    lock_telemetry();
    if ACTIVE.load(Ordering::Relaxed) {
        checked_add(&DEALLOCATED_BYTES, bytes);
        checked_add(&DEALLOCATION_CALLS, 1);
    }
    checked_sub(&LIVE_BYTES, bytes);
    unlock_telemetry();
}

fn record_reallocation(old_size: usize, new_size: usize) {
    let old_bytes = u64::try_from(old_size).unwrap_or(u64::MAX);
    let new_bytes = u64::try_from(new_size).unwrap_or(u64::MAX);
    lock_telemetry();
    checked_sub(&LIVE_BYTES, old_bytes);
    let live_bytes = checked_add(&LIVE_BYTES, new_bytes);
    if ACTIVE.load(Ordering::Relaxed) {
        checked_add(&ALLOCATED_BYTES, new_bytes);
        checked_add(&DEALLOCATED_BYTES, old_bytes);
        checked_add(&ALLOCATION_CALLS, 1);
        checked_add(&DEALLOCATION_CALLS, 1);
        checked_add(&REALLOCATION_CALLS, 1);
        PEAK_LIVE_BYTES.fetch_max(live_bytes, Ordering::Relaxed);
    }
    unlock_telemetry();
}

fn lock_telemetry() {
    while TELEMETRY_LOCK
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        std::hint::spin_loop();
    }
}

fn unlock_telemetry() {
    TELEMETRY_LOCK.store(false, Ordering::Release);
}

fn checked_add(counter: &AtomicU64, value: u64) -> u64 {
    let current = counter.load(Ordering::Relaxed);
    match current.checked_add(value) {
        Some(next) => {
            counter.store(next, Ordering::Relaxed);
            next
        }
        None => {
            counter.store(u64::MAX, Ordering::Relaxed);
            ACCOUNTING_ERRORS.fetch_add(1, Ordering::Relaxed);
            u64::MAX
        }
    }
}

fn checked_sub(counter: &AtomicU64, value: u64) -> u64 {
    let current = counter.load(Ordering::Relaxed);
    match current.checked_sub(value) {
        Some(next) => {
            counter.store(next, Ordering::Relaxed);
            next
        }
        None => {
            counter.store(0, Ordering::Relaxed);
            ACCOUNTING_ERRORS.fetch_add(1, Ordering::Relaxed);
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_records_allocations_without_requiring_exact_global_counts() {
        let mut reallocated = Vec::<u8>::with_capacity(8);
        begin_measurement();
        reallocated.reserve_exact(4096);
        let allocation = vec![0_u8; 4096];
        let measurement = end_measurement();

        assert!(measurement.allocation_calls >= 1);
        assert!(measurement.allocated_bytes >= allocation.len() as u64);
        assert!(measurement.reallocation_calls >= 1);
        assert!(measurement.peak_live_bytes >= measurement.baseline_live_bytes);
        assert!(measurement.peak_live_bytes >= measurement.final_live_bytes);
        assert_eq!(measurement.accounting_errors, 0);
        assert_eq!(
            measurement.baseline_live_bytes + measurement.allocated_bytes,
            measurement.final_live_bytes + measurement.deallocated_bytes,
        );
    }
}
