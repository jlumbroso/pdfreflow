//! pdfreflow — turn a two-column PDF into single-column pages for reading on a phone.
//!
//! Design record: `docs/adr/0002-mvp-pipeline-detect-the-two-columns-and-re-emit-them-as-pages.md`.
//! This file currently holds only the command-line surface; the detect and cut
//! steps are not written yet.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// Turn a two-column PDF into single-column pages that are easy to read on a phone.
///
/// Runs entirely on this machine: the input is never uploaded anywhere.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// The two-column PDF to read.
    input: PathBuf,

    /// Where to write the result. Defaults to the input's name with `.reflow.pdf`.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

impl Cli {
    fn output_path(&self) -> PathBuf {
        self.output
            .clone()
            .unwrap_or_else(|| self.input.with_extension("reflow.pdf"))
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    eprintln!(
        "pdfreflow: not implemented yet — would read {} and write {}",
        cli.input.display(),
        cli.output_path().display()
    );
    ExitCode::from(2)
}
