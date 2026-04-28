use seaki_dto_codegen::{check_generated_file, write_generated_file};
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();

    match args.as_slice() {
        [flag, path] if flag == "--check" => check_generated_file(path).map_err(|error| {
            format!("{error}\nRun `cargo run -p seaki-dto-codegen -- {path}` to regenerate.")
        }),
        [path] => write_generated_file(path).map_err(|error| error.to_string()),
        _ => Err(
            "usage: seaki-dto-codegen [--check] <output-path>\nexample: cargo run -p seaki-dto-codegen -- packages/dto/src/generated.ts"
                .to_string(),
        ),
    }
}
