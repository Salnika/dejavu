use clap::Parser;
use dejavu::cli::dejavu_cli::Cli;

fn main() {
    let cli = Cli::parse();
    let code = match dejavu::cli::run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("dejavu: {err:#}");
            1
        }
    };
    std::process::exit(code);
}
