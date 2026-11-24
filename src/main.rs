use std::env;
use std::process;

use wasm_img_metadata::cli::run_cli_with_args;

fn main() {
    let (json_output, exit_code) = run_cli_with_args(env::args());
    println!("{json_output}");
    if exit_code != 0 {
        process::exit(exit_code);
    }
}
