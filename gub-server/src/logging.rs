use std::sync::OnceLock;



struct Logger {
    filter: log::LevelFilter
}


impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.filter
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return
        }
        let level = record.level();
        let target = record.target();

        #[rustfmt::skip] let level = match level {
            log::Level::Error => "[\x1b[31;1mERROR\x1b[0m]",
            log::Level::Warn =>  "[\x1b[33;1mWARN \x1b[0m]",
            log::Level::Info =>  "[\x1b[37;1mINFO \x1b[0m]",
            log::Level::Debug => "[DEBUG]",
            log::Level::Trace => "[\x1b[2mTRACE\x1b[0m]",
        };
        eprintln!("{level} [{target}]: {msg}", msg = record.args());
    }

    fn flush(&self) { }
}

pub fn enable_logging(level: log::LevelFilter) {
    static LOGGER: OnceLock<Logger> = OnceLock::new();

    LOGGER.set(Logger { filter: level }).ok().expect("already set logger");
    let logger = LOGGER.get().unwrap();

    log::set_max_level(level);
    log::set_logger(logger).unwrap();
}
