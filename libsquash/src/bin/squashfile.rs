use clap::{Args, Parser, Subcommand};

use libsquash::write_image_file;
use libsquash::Result;

use std::path::PathBuf;

#[derive(Parser)]
#[clap(rename_all = "lower")]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Args)]
struct CreateArgs {
    #[clap(short, long, value_parser)]
    source: PathBuf,
    #[clap(short, long, value_parser)]
    image: PathBuf,
}

#[derive(Subcommand)]
enum Command {
    Create(CreateArgs),
}

fn create(args: &CreateArgs) -> Result<()> {
    write_image_file(&args.source, &args.image)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Create(args) => create(args),
    }
}
