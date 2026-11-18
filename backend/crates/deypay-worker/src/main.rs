//! Process entry point for durable background work.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("The DEYPAY background worker is not configured in this foundation build.");
    ExitCode::FAILURE
}
