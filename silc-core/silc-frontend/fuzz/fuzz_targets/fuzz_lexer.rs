#![no_main]

use libfuzzer_sys::fuzz_target;
use silc_frontend::lexer::lexear;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Lexer no debe panic, solo retornar tokens o errores
        let _ = lexear(s);
    }
});