//! Generates `tokens::sys`, `tokens::bar`, `tokens::menu`, … from
//! `design-tokens/tokens.json`, with the generator in
//! `src/tokens/codegen.rs` (tested there). A token renamed in Penpot then
//! fails the build at the line that reads it.
//!
//! The export is read by the kit's own parser (`src/tokens/envelope.rs`),
//! so the build refuses exactly what the kit refuses at run time.

use std::{env, fs, path::Path};

#[allow(dead_code)]
#[path = "src/tokens/codegen.rs"]
mod codegen;
#[allow(dead_code)]
#[path = "src/tokens/envelope.rs"]
mod envelope;
#[allow(dead_code)]
#[path = "src/tokens/error.rs"]
mod error;

use envelope::Library;

const SOURCE: &str = "design-tokens/tokens.json";

fn main() {
    println!("cargo::rerun-if-changed={SOURCE}");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/tokens/codegen.rs");
    println!("cargo::rerun-if-changed=src/tokens/envelope.rs");
    println!("cargo::rerun-if-changed=src/tokens/error.rs");

    let json = fs::read_to_string(SOURCE).unwrap_or_else(|e| fail(&format!("{SOURCE}: {e}")));
    let library = Library::parse(&json).unwrap_or_else(|e| fail(&format!("{SOURCE}: {e}")));

    let out = codegen::generate(&library).unwrap_or_else(|e| fail(&e));

    let dest = Path::new(&env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("tokens.rs");
    fs::write(dest, out).expect("write generated tokens");
}

fn fail(message: &str) -> ! {
    panic!("design tokens: {message}")
}
