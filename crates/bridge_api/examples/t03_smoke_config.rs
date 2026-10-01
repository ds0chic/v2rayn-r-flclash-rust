//! Prints the T03 Xray smoke config produced by `config_codegen`.
//!
//! Used by the T03 fault harness to validate the generated config with the
//! real Xray binary (`xray run -test -c`).

fn main() {
    match bridge_api::api::xray_smoke_config_json() {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("smoke config error: {} {}", error.code, error.message_key);
            std::process::exit(1);
        }
    }
}
