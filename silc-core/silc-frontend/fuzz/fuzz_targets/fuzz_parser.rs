#![no_main]

use libfuzzer_sys::fuzz_target;
use silc_frontend::a_ast::parsear;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // El frontend no debe entrar en pánico, solo retornar Result.
        let _ = parsear(s);
    }
});
