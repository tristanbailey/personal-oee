use chrono::Local;

fn main() {
    let outcome = oee_cli::run(std::env::args(), &oee_cli::Env::from_process(), Local::now());
    print!("{}", outcome.stdout);
    std::process::exit(outcome.exit);
}
