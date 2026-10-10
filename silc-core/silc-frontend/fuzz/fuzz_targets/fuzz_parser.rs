#![no_main]

use libfuzzer_sys::fuzz_target;
use silc_frontend::{lexer::lexear, parser::parsear};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let tokens = lexear(s);
        let _ = parsear(&tokens); // Parser no debe panic, solo retornar Result
    }
});