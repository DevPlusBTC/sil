//! Servidor LSP mínimo sobre stdio: JSON-RPC 2.0 + `Content-Length`.
//!
//! Soporta el ciclo de vida del protocolo, sincronización completa de
//! documentos y diagnósticos reales del frontend (reader S-exp + AST builder).
//! La finalización de código ofrece formas S-exp y tipos primitivos.
//!
//! El transporte usa stdin/stdout; los logs van a stderr (ver `main`).

use miette::{IntoDiagnostic, Result, WrapErr};
use serde_json::{json, Value};
use silc_frontend::a_ast::parsear;
use silc_frontend::error::ErrorFrontend;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

/// Palabras clave S-exp para completion (formas de cabeza + sub-formas).
const FORMA_CABEZA: &[&str] = &[
    "task",
    "struct",
    "model",
    "memory",
    "variant",
    "case",
    "service",
    "handler",
    "net",
    "protocol",
    "capability",
    "resource",
    "port",
    "bandwidth",
    "module",
    "export",
    "migration",
    "from",
    "to",
    "field",
    "record",
    "default",
    "restrictions",
    "params",
    "returns",
    "requires",
    "ensures",
    "body",
    "if",
    "else",
    "assert",
    "assume",
    "prove",
    "invariant",
    "while",
    "for",
    "let",
    "let-mut",
    "return",
    "request",
    "validate",
    "send",
    "recv",
    "spawn",
    "chan",
    "capacity",
    "io-read",
    "io-write",
    "io-send",
    "io-recv",
    "get",
    "ttl",
    "as",
];

const TIPOS: &[&str] = &[
    "i64",
    "f64",
    "booleano",
    "texto",
    "capacidad",
    "void",
    "USD",
    "EUR",
    "list",
    "map",
    "set",
    "tuple",
];

const PERMISOS: &[&str] = &["lectura_archivo", "escritura_archivo", "red", "ejecucion"];

const OPERADORES: &[&str] = &["+", "-", "*", "/", ">", "<", "==", "!=", ">=", "<="];

const LITERALES: &[&str] = &["verdadero", "falso"];

/// Estado de los documentos abiertos en el editor.
#[derive(Default)]
pub struct Servidor {
    documentos: HashMap<String, String>,
}

/// Resultado de procesar un mensaje: respuestas/notificaciones a enviar y si
/// el servidor debe terminar (mensaje `exit`).
pub struct Procesado {
    pub mensajes: Vec<Value>,
    pub terminar: bool,
}

impl Servidor {
    pub fn procesar(&mut self, msg: &Value) -> Procesado {
        let id = msg.get("id").cloned();
        let metodo = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let mut mensajes = Vec::new();
        let mut terminar = false;

        match metodo {
            "initialize" => mensajes.push(respuesta(
                id,
                json!({
                    "capabilities": {
                        "textDocumentSync": 1,
                        "completionProvider": { "resolveProvider": false }
                    },
                    "serverInfo": { "name": "silc-lsp", "version": env!("CARGO_PKG_VERSION") }
                }),
            )),
            "initialized" => {}
            "shutdown" => mensajes.push(respuesta(id, Value::Null)),
            "exit" => terminar = true,
            "textDocument/didOpen" => {
                if let (Some(uri), Some(contenido)) = (
                    texto(&params, &["textDocument", "uri"]),
                    texto(&params, &["textDocument", "text"]),
                ) {
                    self.documentos.insert(uri.clone(), contenido.clone());
                    mensajes.push(self.publicar(&uri, &contenido));
                }
            }
            "textDocument/didChange" => {
                let uri = texto(&params, &["textDocument", "uri"]);
                let contenido = params
                    .get("contentChanges")
                    .and_then(Value::as_array)
                    .and_then(|cambios| cambios.last())
                    .and_then(|cambio| cambio.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if let Some(uri) = uri {
                    self.documentos.insert(uri.clone(), contenido.clone());
                    mensajes.push(self.publicar(&uri, &contenido));
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = texto(&params, &["textDocument", "uri"]) {
                    self.documentos.remove(&uri);
                    mensajes.push(self.publicar(&uri, ""));
                }
            }
            "textDocument/completion" => {
                let items: Vec<Value> = FORMA_CABEZA
                    .iter()
                    .map(|k| json!({ "label": k, "kind": 14 }))
                    .chain(TIPOS.iter().map(|t| json!({ "label": t, "kind": 25 })))
                    .chain(PERMISOS.iter().map(|p| json!({ "label": p, "kind": 25 })))
                    .chain(OPERADORES.iter().map(|o| json!({ "label": o, "kind": 25 })))
                    .chain(LITERALES.iter().map(|l| json!({ "label": l, "kind": 25 })))
                    .collect();
                mensajes.push(respuesta(
                    id,
                    json!({ "isIncomplete": false, "items": items }),
                ));
            }
            _ => {
                if let Some(id) = id {
                    mensajes.push(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32601, "message": format!("método no soportado: {metodo}") }
                    }));
                }
            }
        }

        Procesado { mensajes, terminar }
    }

    fn publicar(&self, uri: &str, contenido: &str) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": { "uri": uri, "diagnostics": diagnosticos(contenido) }
        })
    }
}

fn respuesta(id: Option<Value>, resultado: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "result": resultado
    })
}

fn texto(params: &Value, ruta: &[&str]) -> Option<String> {
    let mut actual = params;
    for clave in ruta {
        actual = actual.get(clave)?;
    }
    actual.as_str().map(str::to_string)
}

/// Diagnósticos del documento: primer error del reader o del AST builder.
fn diagnosticos(fuente: &str) -> Vec<Value> {
    match parsear(fuente) {
        Ok(_) => Vec::new(),
        Err(error) => {
            let (inicio, longitud) = span_de_error(&error);
            vec![diagnostico(fuente, inicio, longitud, error.to_string())]
        }
    }
}

fn span_de_error(error: &ErrorFrontend) -> (usize, usize) {
    match error {
        ErrorFrontend::CaracterDesconocido { span, .. }
        | ErrorFrontend::TokenInesperado { span, .. }
        | ErrorFrontend::ExpresionDemasiadoProfunda { span, .. }
        | ErrorFrontend::FormaDesconocida { span, .. }
        | ErrorFrontend::AridadIncorrecta { span, .. }
        | ErrorFrontend::IdentificadorInvalido { span, .. } => (span.offset(), span.len()),
        ErrorFrontend::FinInesperado => (usize::MAX, 0),
    }
}

fn diagnostico(fuente: &str, inicio: usize, longitud: usize, mensaje: String) -> Value {
    let inicio = inicio.min(fuente.len());
    let fin = (inicio + longitud).min(fuente.len());
    let (linea_ini, col_ini) = posicion(fuente, inicio);
    let (linea_fin, col_fin) = posicion(fuente, fin);
    json!({
        "range": {
            "start": { "line": linea_ini, "character": col_ini },
            "end": { "line": linea_fin, "character": col_fin }
        },
        "severity": 1,
        "source": "silc",
        "message": mensaje
    })
}

/// Convierte un desplazamiento en bytes a (línea, columna UTF-16).
fn posicion(fuente: &str, byte: usize) -> (u32, u32) {
    let mut linea = 0u32;
    let mut columna = 0u32;
    for (indice, caracter) in fuente.char_indices() {
        if indice >= byte {
            break;
        }
        if caracter == '\n' {
            linea += 1;
            columna = 0;
        } else {
            columna += caracter.len_utf16() as u32;
        }
    }
    (linea, columna)
}

/// Sirve LSP por stdio hasta recibir `exit` o cerrarse la entrada.
pub fn servir() -> Result<()> {
    let entrada = io::stdin();
    let mut lector = io::BufReader::new(entrada.lock());
    let mut salida = io::stdout();
    let mut servidor = Servidor::default();

    while let Some(cuerpo) = leer_mensaje(&mut lector).into_diagnostic()? {
        let mensaje: Value = match serde_json::from_str(&cuerpo) {
            Ok(m) => m,
            Err(error) => {
                eprintln!("silc lsp: JSON inválido: {error}");
                continue;
            }
        };
        let procesado = servidor.procesar(&mensaje);
        for respuesta in procesado.mensajes {
            escribir_mensaje(&mut salida, &respuesta)?;
        }
        if procesado.terminar {
            break;
        }
    }
    Ok(())
}

fn leer_mensaje<R: BufRead>(lector: &mut R) -> io::Result<Option<String>> {
    let mut longitud: Option<usize> = None;
    loop {
        let mut linea = String::new();
        if lector.read_line(&mut linea)? == 0 {
            return Ok(None);
        }
        let linea = linea.trim_end_matches(['\r', '\n']);
        if linea.is_empty() {
            break;
        }
        if let Some(valor) = linea.strip_prefix("Content-Length:") {
            longitud = valor.trim().parse::<usize>().ok();
        }
    }
    let Some(longitud) = longitud else {
        return Ok(None);
    };
    let mut cuerpo = vec![0u8; longitud];
    lector.read_exact(&mut cuerpo)?;
    Ok(Some(String::from_utf8_lossy(&cuerpo).into_owned()))
}

fn escribir_mensaje<W: Write>(salida: &mut W, mensaje: &Value) -> Result<()> {
    let cuerpo = serde_json::to_string(mensaje).into_diagnostic()?;
    write!(salida, "Content-Length: {}\r\n\r\n{}", cuerpo.len(), cuerpo)
        .into_diagnostic()
        .wrap_err("no se pudo escribir la respuesta LSP")?;
    salida.flush().into_diagnostic()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_declara_sincronizacion_y_completion() {
        let mut s = Servidor::default();
        let r = s.procesar(&json!({
            "id": 1,
            "method": "initialize",
            "params": { "capabilities": {} }
        }));
        let capacidades = &r.mensajes[0]["result"]["capabilities"];
        assert_eq!(capacidades["textDocumentSync"], 1);
        assert!(capacidades["completionProvider"].is_object());
    }

    #[test]
    fn exit_termina_el_servidor() {
        let mut s = Servidor::default();
        let r = s.procesar(&json!({ "id": 1, "method": "exit" }));
        assert!(r.terminar);
    }

    #[test]
    fn codigo_valido_no_publica_diagnosticos() {
        let mut s = Servidor::default();
        let src = "(task f (params) (returns i64) (body (return 42)))";
        let r = s.procesar(&json!({
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///test.sil", "text": src } }
        }));
        let diag = r.mensajes[0]
            .get("params")
            .unwrap()
            .get("diagnostics")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(diag.is_empty(), "diagnósticos inesperados: {diag:?}");
    }

    #[test]
    fn did_open_publica_diagnosticos_de_sintaxis() {
        let mut s = Servidor::default();
        // Un error real: paréntesis sin cerrar
        let src = "(task f (params";
        let r = s.procesar(&json!({
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///test.sil", "text": src } }
        }));
        let diag = r.mensajes[0]
            .get("params")
            .unwrap()
            .get("diagnostics")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(!diag.is_empty());
    }

    #[test]
    fn did_close_limpia_diagnosticos() {
        let mut s = Servidor::default();
        let src = "(task f (params) (returns i64) (body (return 42)))";
        s.procesar(&json!({
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///test.sil", "text": src } }
        }));
        let r = s.procesar(&json!({
            "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": "file:///test.sil" } }
        }));
        let diag = r.mensajes[0]
            .get("params")
            .unwrap()
            .get("diagnostics")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(diag.is_empty());
    }

    #[test]
    fn caracter_invalido_genera_diagnostico_lexico() {
        let mut s = Servidor::default();
        let src = "(task f (params) (body (return @)))"; // @ no es válido
        let r = s.procesar(&json!({
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///test.sil", "text": src } }
        }));
        let diag = r.mensajes[0]
            .get("params")
            .unwrap()
            .get("diagnostics")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(!diag.is_empty());
    }

    #[test]
    fn posicion_cuenta_utf16() {
        assert_eq!(posicion("a", 0), (0, 0));
        assert_eq!(posicion("a\nb", 2), (1, 0));
        assert_eq!(posicion("a\nb", 1), (0, 1));
    }

    #[test]
    fn metodo_desconocido_devuelve_error_solo_si_es_request() {
        let mut s = Servidor::default();
        // Notificación (sin id) no debe responder
        let r = s.procesar(&json!({ "method": "inexistente" }));
        assert!(r.mensajes.is_empty());
        // Request (con id) debe responder con error
        let r = s.procesar(&json!({ "id": 1, "method": "inexistente" }));
        assert_eq!(
            r.mensajes[0].get("error").unwrap().get("code").unwrap(),
            -32601
        );
    }

    #[test]
    fn completion_devuelve_palabras_clave_y_tipos() {
        let mut s = Servidor::default();
        let r = s.procesar(&json!({
            "id": 1,
            "method": "textDocument/completion",
            "params": {}
        }));
        let items = r.mensajes[0]
            .get("result")
            .unwrap()
            .get("items")
            .unwrap()
            .as_array()
            .unwrap();
        let labels: Vec<&str> = items
            .iter()
            .filter_map(|i| i.get("label").and_then(Value::as_str))
            .collect();
        assert!(labels.contains(&"task"));
        assert!(labels.contains(&"if"));
        assert!(labels.contains(&"i64"));
        assert!(labels.contains(&"list"));
    }

    #[test]
    fn did_change_reemplaza_el_contenido() {
        let mut s = Servidor::default();
        let src1 = "(task f (params) (returns i64) (body (return 1)))";
        s.procesar(&json!({
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///test.sil", "text": src1 } }
        }));
        let src2 = "(task f (params) (returns i64) (body (return 2)))";
        let r = s.procesar(&json!({
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///test.sil" },
                "contentChanges": [{ "text": src2 }]
            }
        }));
        let diag = r.mensajes[0]
            .get("params")
            .unwrap()
            .get("diagnostics")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(diag.is_empty());
    }
}
