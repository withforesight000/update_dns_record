use std::{io, os::fd::AsRawFd};

use nix::unistd::isatty;
use syslog::{Formatter3164, LoggerBackend};

pub trait Logger {
    fn log_info(&mut self, message: &str);
    fn log_error(&mut self, message: &str);
}

fn has_controlling_terminal() -> bool {
    isatty(io::stdout().as_raw_fd()).unwrap_or(false)
}

pub fn new() -> Box<dyn Logger> {
    if has_controlling_terminal() {
        Box::new(StdOutLogger::new()) as Box<dyn Logger>
    } else {
        Box::new(SyslogLogger::new()) as Box<dyn Logger>
    }
}

pub struct SyslogLogger {
    writer: syslog::Logger<LoggerBackend, Formatter3164>,
}

impl SyslogLogger {
    fn new() -> SyslogLogger {
        let formatter = Formatter3164 {
            hostname: None, // workaround fix for Logger format
            ..Default::default()
        };

        let writer = syslog::unix(formatter).expect("could not connect to syslog");
        SyslogLogger { writer }
    }
}

impl Logger for SyslogLogger {
    fn log_info(&mut self, message: &str) {
        self.writer
            .info(message)
            .expect("could not write to syslog");
    }

    fn log_error(&mut self, message: &str) {
        self.writer.err(message).expect("could not write to syslog");
    }
}

pub struct StdOutLogger {}

impl StdOutLogger {
    fn new() -> StdOutLogger {
        StdOutLogger {}
    }
}

impl Logger for StdOutLogger {
    fn log_info(&mut self, message: &str) {
        println!("{}", message);
    }

    fn log_error(&mut self, message: &str) {
        eprintln!("{}", message);
    }
}
