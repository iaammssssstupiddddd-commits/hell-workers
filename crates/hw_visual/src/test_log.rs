//! Shared thread-local warning probe for real Bevy command log handlers.
use std::{cell::Cell, sync::Once};

thread_local! {
    static WARNINGS: Cell<usize> = const { Cell::new(0) };
}

pub(crate) struct WarningProbe;

impl log::Log for WarningProbe {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            WARNINGS.with(|count| count.set(count.get() + 1));
        }
    }
    fn flush(&self) {}
}

impl WarningProbe {
    pub(crate) fn start() {
        static INIT: Once = Once::new();
        static LOGGER: WarningProbe = WarningProbe;
        INIT.call_once(|| {
            log::set_logger(&LOGGER).expect("unit-test warning probe must own the logger");
            log::set_max_level(log::LevelFilter::Warn);
        });
        WARNINGS.with(|count| count.set(0));
    }
    pub(crate) fn count() -> usize {
        WARNINGS.with(Cell::get)
    }
    pub(crate) fn assert_clean() {
        assert_eq!(Self::count(), 0, "deferred command emitted a warning/error");
    }
}
