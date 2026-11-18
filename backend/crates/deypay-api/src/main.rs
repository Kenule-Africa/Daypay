//! HTTP entry point for the DEYPAY API.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("The DEYPAY API server is not configured in this foundation build.");
    ExitCode::FAILURE
}
