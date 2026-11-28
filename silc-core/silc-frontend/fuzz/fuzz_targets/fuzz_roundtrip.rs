#![no_main]

use libfuzzer_sys::fuzz_target;
use silc_frontend::{a_ast, reader};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // El reader y el AST builder deben ser deterministas y no entrar en pánico.
        assert_eq!(reader::leer(s).is_ok(), reader::leer(s).is_ok());
        assert_eq!(a_ast::parsear(s).is_ok(), a_ast::parsear(s).is_ok());
    }
});
