mod utils;

use cairo1_run::{cairo_run_program, Cairo1RunConfig};
use cairo_lang_sierra::ProgramParser;
use cairo_vm::types::layout_name::LayoutName;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(msg: &str);
}

#[wasm_bindgen(start)]
pub fn start() {
    crate::utils::set_panic_hook();
}

// TODO: check why this is needed. Seems wasm-bindgen expects us to use
// `std::error::Error` even if it's not yet in `core`
macro_rules! wrap_error {
    ($xp: expr) => {
        $xp.map_err(|e| JsError::new(e.to_string().as_str()))
    };
}

#[wasm_bindgen(js_name = runCairoProgram)]
pub fn run_cairo_program() -> Result<String, JsError> {
    let cairo_run_config = Cairo1RunConfig {
        layout: LayoutName::all_cairo,
        relocate_mem: true,
        trace_enabled: true,
        serialize_output: true,
        ..Default::default()
    };

    // using cairo-lang 1.1.1 and wasm-demo-cairo1/bitwise.sierra
    let sierra_program = match serde_json::from_slice(include_bytes!("../bitwise.sierra")) {
        Ok(sierra) => sierra,
        Err(_) => {
            let program_str = include_str!("../bitwise.sierra");

            let parser = ProgramParser::new();
            parser
                .parse(program_str)
                .map_err(|e| e.map_token(|t| t.to_string()))?
        }
    };

    let (_, _, serielized_output_option) =
        wrap_error!(cairo_run_program(&sierra_program, cairo_run_config))?;

    let output = serielized_output_option.unwrap();

    log(&output);

    Ok(output)
}
