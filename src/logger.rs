use syslog::{Formatter3164, LoggerBackend};

pub trait Log {
    fn log_info(&mut self, message: &str);
    fn log_error(&mut self, message: &str);
}

pub struct SyslogLogger {
    writer: syslog::Logger<LoggerBackend, Formatter3164>
}

impl SyslogLogger {
    pub fn new() -> SyslogLogger {
        let formatter = Formatter3164 {
            hostname: None, // workaround fix for log format
            ..Default::default()
        };

        let writer = syslog::unix(formatter).expect("could not connect to syslog");
        SyslogLogger { writer }
    }
}

impl Log for SyslogLogger {
    fn log_info(&mut self, message: &str) {
        self.writer.info(message).expect("could not write to syslog");
    }

    fn log_error(&mut self, message: &str) {
        self.writer.err(message).expect("could not write to syslog");
    }
}

pub struct StdOutLogger {}

impl StdOutLogger {
    pub fn new() -> StdOutLogger {
        StdOutLogger {}
    }
}

impl Log for StdOutLogger {
    fn log_info(&mut self, message: &str) {
        println!("{}", message);
    }

    fn log_error(&mut self, message: &str) {
        println!("{}", message);

    }
}
