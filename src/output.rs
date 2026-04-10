use std::fmt;

/// Tag prefixes for daemon stdout lines
pub enum Tag {
    Console,
    Log(f64),
    Status,
    Flash,
    Error,
    Recover,
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tag::Console => write!(f, "[console]"),
            Tag::Log(ts) => write!(f, "[log {:.3}]", ts),
            Tag::Status => write!(f, "[status]"),
            Tag::Flash => write!(f, "[flash]"),
            Tag::Error => write!(f, "[error]"),
            Tag::Recover => write!(f, "[recover]"),
        }
    }
}

/// Print a tagged line to stdout
pub fn print_tagged(tag: Tag, message: &str) {
    println!("{} {}", tag, message);
}
