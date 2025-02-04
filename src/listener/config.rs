use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// Network interface to capture packets from
    #[arg(short, long, default_value = "default")]
    pub interface: String,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
}
