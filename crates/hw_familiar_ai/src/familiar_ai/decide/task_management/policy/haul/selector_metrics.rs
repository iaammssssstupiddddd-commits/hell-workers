//! パフォーマンス計測カウンター — ソースセレクタの呼び出し回数とスキャン数を追跡する。

use std::sync::atomic::{AtomicU32, Ordering};

static SOURCE_SELECTOR_CALLS: AtomicU32 = AtomicU32::new(0);
static SOURCE_SELECTOR_CACHE_BUILD_SCANNED_ITEMS: AtomicU32 = AtomicU32::new(0);
static SOURCE_SELECTOR_CANDIDATE_SCANNED_ITEMS: AtomicU32 = AtomicU32::new(0);

pub(super) fn mark_source_selector_call() {
    SOURCE_SELECTOR_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub(super) fn mark_cache_build_scanned_item() {
    SOURCE_SELECTOR_CACHE_BUILD_SCANNED_ITEMS.fetch_add(1, Ordering::Relaxed);
}

pub(super) fn mark_candidate_scanned_item() {
    SOURCE_SELECTOR_CANDIDATE_SCANNED_ITEMS.fetch_add(1, Ordering::Relaxed);
}

/// ソースセレクタの計測スナップショットを取得し、カウンターをリセットする。
/// 戻り値: (呼び出し数, キャッシュビルド時スキャン数, 候補スキャン数)
pub fn take_source_selector_scan_snapshot() -> (u32, u32, u32) {
    (
        SOURCE_SELECTOR_CALLS.swap(0, Ordering::Relaxed),
        SOURCE_SELECTOR_CACHE_BUILD_SCANNED_ITEMS.swap(0, Ordering::Relaxed),
        SOURCE_SELECTOR_CANDIDATE_SCANNED_ITEMS.swap(0, Ordering::Relaxed),
    )
}
