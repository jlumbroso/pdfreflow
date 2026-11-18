//! The `pdfreflow` command line. All the work is in the library (`src/lib.rs`).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use pdfreflow::Report;
use pdfreflow::layout::PagePlan;

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

    /// Print the geometry that was found: columns, gap, and every crop.
    /// Prints numbers only, never any of the document's content.
    #[arg(long)]
    explain: bool,
}

fn explain(report: &Report) {
    let layout = &report.layout;
    eprintln!(
        "  left column   x {:.0}–{:.0}\n  gap           x {:.0}–{:.0}\n  right column  x {:.0}–{:.0}\n  text block    y {:.0}–{:.0}",
        layout.left.lo,
        layout.left.hi,
        layout.gutter.lo,
        layout.gutter.hi,
        layout.right.lo,
        layout.right.hi,
        layout.type_area.lo,
        layout.type_area.hi,
    );
    for (index, plan) in report.plans.iter().enumerate() {
        match plan {
            PagePlan::Keep => eprintln!("  page {:>3}: kept whole", index + 1),
            PagePlan::Crops(crops) => {
                let crops: Vec<String> = crops
                    .iter()
                    .map(|c| format!("[{:.0} {:.0} {:.0} {:.0}]", c.x0, c.y0, c.x1, c.y1))
                    .collect();
                eprintln!("  page {:>3}: {}", index + 1, crops.join("  "));
            }
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let output = cli
        .output
        .clone()
        .unwrap_or_else(|| cli.input.with_extension("reflow.pdf"));
    if output == cli.input {
        eprintln!("pdfreflow: the output would overwrite the input; choose another name with -o");
        return ExitCode::FAILURE;
    }

    match pdfreflow::reflow_file(&cli.input, &output) {
        Ok(report) => {
            eprintln!(
                "pdfreflow: {} pages -> {} pages, written to {}",
                report.source_pages,
                report.output_pages,
                output.display()
            );
            if cli.explain {
                explain(&report);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("pdfreflow: {}: {error}", cli.input.display());
            ExitCode::FAILURE
        }
    }
}
