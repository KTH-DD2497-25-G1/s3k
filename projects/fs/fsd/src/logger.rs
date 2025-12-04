use log::{Level, LevelFilter, Metadata, Record};

#[macro_export]
#[cfg(feature = "nocolor")]
macro_rules! with_color {
    ($color_code: expr, $fmt: expr $(, $($arg: tt)+)?) => {
        use crate::println;
        println!($fmt, $($($arg)+)?);
    }
}

#[macro_export]
#[cfg(not(feature = "nocolor"))]
macro_rules! with_color {
    ($color_code: expr, $fmt: expr $(, $($arg: tt)+)?) => {
        use s3k_common::println;
        println!(
            concat!("\x1b[{}m", $fmt, "\x1b[0m"),
            $color_code,
            $($($arg)+)?
        );
    }
}

fn level_color(level: Level) -> u8 {
    match level {
        Level::Error => 31,
        Level::Warn => 93,
        Level::Info => 34,
        Level::Debug => 32,
        Level::Trace => 90,
    }
}

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            with_color!(
                level_color(record.level()),
                "[{:5}] [fsd] {}",
                record.level(),
                record.args(),
            );
        }
    }
    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

pub fn init() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::Trace);
}
