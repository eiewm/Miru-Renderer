use std::env;
pub struct Logger {
    prefix: &'static str,
}
impl Logger {
    pub const fn new(prefix: &'static str) -> Self {
        Self { prefix }
    }
    fn should_log(&self) -> bool {
        env::var("QUIET_LOGS").map_or(true, |v| v != "1")
    }
    pub fn log(&self, msg: &str) {
        if self.should_log() {
            println!("{} -> {}", self.prefix, msg);
        }
    }
    pub fn info(&self, msg: &str) {
        if self.should_log() {
            println!("{} info: {}", self.prefix, msg);
        }
    }
    pub fn warn(&self, msg: &str) {
        // Warnings and errors bypass QUIET_LOGS so batch callers still see actionable failures.
        eprintln!("{} warn: {}", self.prefix, msg);
    }
    pub fn err(&self, msg: &str) {
        eprintln!("{} err: {}", self.prefix, msg);
    }
    pub fn debug(&self, msg: &str) {
        if self.should_log() && env::var("RUST_LOG").is_ok() {
            println!("{} debug: {}", self.prefix, msg);
        }
    }
}
pub static LOG: Logger = Logger::new("[miru]");
