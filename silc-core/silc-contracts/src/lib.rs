//! silc-contracts - Runtime Contracts compartidos para todo el workspace SIL.
//!
//! Contratos de precondición, postcondición, invariantes y aritmética verificada.
//! Ejecutan SIEMPRE (release + debug). Fail-fast con panic descriptivo.
//! Diseño: zero-cost en éxito, panic informativo en fallo.

use std::panic::{catch_unwind, AssertUnwindSafe};

/// Ejecuta un contrato precondición. Panic si falla.
#[inline(always)]
pub fn require(cond: bool, msg: &str) {
    if !cond {
        panic!("PRECONDICIÓN VIOLADA: {msg}");
    }
}

/// Ejecuta un contrato postcondición. Panic si falla.
#[inline(always)]
pub fn ensure(cond: bool, msg: &str) {
    if !cond {
        panic!("POSTCONDICIÓN VIOLADA: {msg}");
    }
}

/// Ejecuta un invariante de estructura. Panic si falla.
#[inline(always)]
pub fn invariant(cond: bool, msg: &str) {
    if !cond {
        panic!("INVARIANTE VIOLADO: {msg}");
    }
}

/// Verifica que un puntero no es nulo (para FFI).
#[inline(always)]
pub fn require_non_null<T>(ptr: *const T, msg: &str) {
    require(!ptr.is_null(), msg);
}

/// Verifica que un slice no está vacío.
#[inline(always)]
pub fn require_non_empty<T>(slice: &[T], msg: &str) {
    require(!slice.is_empty(), msg);
}

/// Verifica que un índice está en rango.
#[inline(always)]
pub fn require_index(idx: usize, len: usize, msg: &str) {
    require(idx < len, msg);
}

/// Verifica que un valor numérico está en rango [min, max].
#[inline(always)]
pub fn require_range<T: PartialOrd>(val: T, min: T, max: T, msg: &str) {
    require(val >= min && val <= max, msg);
}

/// Verifica que un `Result` es `Ok`, panica con el error si no.
#[inline(always)]
pub fn require_ok<T, E: std::fmt::Debug>(res: Result<T, E>, msg: &str) -> T {
    match res {
        Ok(v) => v,
        Err(e) => panic!("{msg}: {e:?}"),
    }
}

/// Verifica que un `Option` es `Some`, panica si es `None`.
#[inline(always)]
pub fn require_some<T>(opt: Option<T>, msg: &str) -> T {
    match opt {
        Some(v) => v,
        None => panic!("{msg}: esperaba Some, encontrado None"),
    }
}

/// Ejecuta una operación con postcondición verificada.
#[inline(always)]
pub fn with_post<T, F, P>(f: F, post: P) -> T
where
    F: FnOnce() -> T,
    P: FnOnce(&T) -> bool,
{
    let result = f();
    ensure(post(&result), "postcondición falló");
    result
}

/// Ejecuta una función con precondición y postcondición.
#[inline(always)]
pub fn contract<T, F, Pre, Post>(pre: Pre, f: F, post: Post) -> T
where
    Pre: FnOnce() -> bool,
    F: FnOnce() -> T,
    Post: FnOnce(&T) -> bool,
{
    require(pre(), "precondición falló");
    let result = f();
    ensure(post(&result), "postcondición falló");
    result
}

/// Verifica que no hay desbordamiento en operación aritmética.
#[inline(always)]
pub fn checked_add(a: u64, b: u64, msg: &str) -> u64 {
    a.checked_add(b).unwrap_or_else(|| panic!("{msg}: desbordamiento en suma"))
}

/// Verifica que no hay desbordamiento en resta.
#[inline(always)]
pub fn checked_sub(a: u64, b: u64, msg: &str) -> u64 {
    a.checked_sub(b).unwrap_or_else(|| panic!("{msg}: desbordamiento en resta"))
}

/// Verifica que no hay desbordamiento en multiplicación.
#[inline(always)]
pub fn checked_mul(a: u64, b: u64, msg: &str) -> u64 {
    a.checked_mul(b).unwrap_or_else(|| panic!("{msg}: desbordamiento en multiplicación"))
}

/// Verifica que no hay división por cero.
#[inline(always)]
pub fn checked_div(a: u64, b: u64, msg: &str) -> u64 {
    if b == 0 {
        panic!("{msg}: división por cero");
    }
    a / b
}

/// Verifica que un string no está vacío y no excede longitud máxima.
#[inline(always)]
pub fn require_valid_ident(s: &str, max_len: usize, msg: &str) {
    require(!s.is_empty(), msg);
    require(s.len() <= max_len, msg);
    require(
        s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        msg,
    );
    require(
        s.chars().next().map_or(false, |c| c.is_ascii_alphabetic() || c == '_'),
        msg,
    );
}

/// Verifica que un path es seguro (sin traversal).
#[inline(always)]
pub fn require_safe_path(path: &str, msg: &str) {
    require(!path.is_empty(), msg);
    require(!path.contains(".."), msg);
    require(!path.contains('\0'), msg);
}

/// Ejecuta una closure capturando panics, retorna `Err` si panica.
#[inline(always)]
pub fn catch_panic<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    catch_unwind(AssertUnwindSafe(f)).map_err(|e| {
        let msg = if let Some(s) = e.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else {
            "panic desconocido".to_string()
        };
        format!("panic capturado: {msg}")
    })
}

/// Macro para precondición con mensaje formateado.
#[macro_export]
macro_rules! contract_pre {
    ($cond:expr, $($arg:tt)*) => {
        $crate::require($cond, &format!($($arg)*))
    };
}

/// Macro para postcondición con mensaje formateado.
#[macro_export]
macro_rules! contract_post {
    ($cond:expr, $($arg:tt)*) => {
        $crate::ensure($cond, &format!($($arg)*))
    };
}

/// Macro para invariante con mensaje formateado.
#[macro_export]
macro_rules! contract_invariant {
    ($cond:expr, $($arg:tt)*) => {
        $crate::invariant($cond, &format!($($arg)*))
    };
}

/// Macro para contrato completo: pre + fn + post.
#[macro_export]
macro_rules! contract {
    ($pre:expr, $f:expr, $post:expr) => {
        $crate::contract(
            || $pre,
            $f,
            |r| $post
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require_ok() {
        require(true, "debe pasar");
    }

    #[test]
    #[should_panic(expected = "PRECONDICIÓN VIOLADA")]
    fn test_require_fail() {
        require(false, "debe fallar");
    }

    #[test]
    fn test_ensure_ok() {
        ensure(true, "debe pasar");
    }

    #[test]
    #[should_panic(expected = "POSTCONDICIÓN VIOLADA")]
    fn test_ensure_fail() {
        ensure(false, "debe fallar");
    }

    #[test]
    fn test_invariant_ok() {
        invariant(true, "debe pasar");
    }

    #[test]
    #[should_panic(expected = "INVARIANTE VIOLADO")]
    fn test_invariant_fail() {
        invariant(false, "debe fallar");
    }

    #[test]
    fn test_require_ok_ok() {
        let r: Result<i32, &str> = Ok(42);
        assert_eq!(require_ok(r, "msg"), 42);
    }

    #[test]
    #[should_panic(expected = "msg: \"Err\"")]
    fn test_require_ok_fail() {
        let r: Result<i32, &str> = Err("Err");
        require_ok(r, "msg");
    }

    #[test]
    fn test_contract_pre_macro() {
        contract_pre!(1 + 1 == 2, "suma válida");
    }

    #[test]
    #[should_panic(expected = "PRECONDICIÓN VIOLADA")]
    fn test_contract_pre_macro_fail() {
        contract_pre!(1 + 1 == 3, "suma inválida");
    }

    #[test]
    fn test_checked_arithmetic() {
        assert_eq!(checked_add(1, 2, "suma"), 3);
        assert_eq!(checked_sub(5, 3, "resta"), 2);
        assert_eq!(checked_mul(3, 4, "mul"), 12);
        assert_eq!(checked_div(10, 2, "div"), 5);
    }

    #[test]
    #[should_panic(expected = "desbordamiento")]
    fn test_checked_add_overflow() {
        checked_add(u64::MAX, 1, "overflow");
    }

    #[test]
    #[should_panic(expected = "división por cero")]
    fn test_checked_div_zero() {
        checked_div(10, 0, "div by zero");
    }

    #[test]
    fn test_require_range() {
        require_range(5, 0, 10, "en rango");
    }

    #[test]
    #[should_panic(expected = "PRECONDICIÓN VIOLADA")]
    fn test_require_range_fail() {
        require_range(15, 0, 10, "fuera de rango");
    }

    #[test]
    fn test_catch_panic_ok() {
        let r = catch_panic(|| 42);
        assert_eq!(r, Ok(42));
    }

    #[test]
    fn test_catch_panic_err() {
        let r = catch_panic(|| panic!("boom"));
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("boom"));
    }
}