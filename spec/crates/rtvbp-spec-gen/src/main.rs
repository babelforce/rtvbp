fn main() {
    match rtvbp_spec_gen::cli::run(std::env::args_os().skip(1)) {
        Ok(()) => {}
        Err(rtvbp_spec_gen::cli::CliError::Help) => {
            println!("{}", rtvbp_spec_gen::cli::CliError::Help);
        }
        Err(error) => {
            eprintln!("rtvbp-spec-gen: {error}");
            std::process::exit(1);
        }
    }
}
