#![no_main]

use libfuzzer_sys::fuzz_target;
use silc_frontend::a_ast::parsear;
use silc_frontend::reader::leer;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // El frontend no debe entrar en pánico con entradas arbitrarias.
        let _ = leer(s);
        let _ = parsear(s);
    }
});