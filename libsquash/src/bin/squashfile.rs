use clap::{Args, Parser, Subcommand};

use libsquash::{extract_image_file, write_image_file, EncryptionType, Result};

use std::path::PathBuf;

extern crate hex;

fn enc_parse(s: &str) -> std::result::Result<EncryptionType, String> {
    Ok(match s {
        "chacha20" => EncryptionType::ChaCha20,
        "none" => EncryptionType::None,
        _ => return Err("Invalid encryption type".into()),
    })
}

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
    #[clap(short, long, value_parser)]
    key: Option<String>,
    #[clap(short, long, value_parser = enc_parse, default_value = "none")]
    enc_type: EncryptionType,
}

#[derive(Args)]
struct ExtractArgs {
    #[clap(short, long, value_parser)]
    target: PathBuf,
    #[clap(short, long, value_parser)]
    image: PathBuf,
    #[clap(short, long, value_parser)]
    key: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    Create(CreateArgs),
    Extract(ExtractArgs),
}

fn create(args: &CreateArgs) -> Result<()> {
    let key = match args.key {
        Some(ref s) => Some(hex::decode(s)?),
        None => None,
    };
    write_image_file(&args.source, &args.image, key.as_deref(), args.enc_type)
}

fn extract(args: &ExtractArgs) -> Result<()> {
    let key = match args.key {
        Some(ref s) => Some(hex::decode(s)?),
        None => None,
    };
    extract_image_file(&args.image, &args.target, key.as_deref())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Create(args) => create(args),
        Command::Extract(args) => extract(args),
    }
}
