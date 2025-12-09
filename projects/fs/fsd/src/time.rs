use core::time::Duration;

const MICROS_PER_SEC: usize = 1000000;
const CPU_FREQ: usize = 10_000_000; // 10 MHz

#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct TimeSpec {
    pub sec: i64,
    pub nsec: i64,
}

impl TimeSpec {
    pub const fn new(sec: i64, nsec: i64) -> Self {
        Self { sec, nsec }
    }
}

impl From<Duration> for TimeSpec {
    fn from(dur: Duration) -> Self {
        Self {
            sec: dur.as_secs() as i64,
            nsec: dur.subsec_nanos() as i64,
        }
    }
}

pub fn real_time() -> Duration {
    let hardware_ts = 0;
    let ms = hardware_ts / (CPU_FREQ / MICROS_PER_SEC);
    Duration::from_micros(ms as u64)
}
