//! Servidor LSP mínimo sobre stdio: JSON-RPC 2.0 + `Content-Length`.
//!
//! Soporta el ciclo de vida del protocolo, sincronización completa de
//! documentos y diagnósticos reales del frontend (lexer + parser). La
//! finalización de código ofrece palabras clave y tipos primitivos.
//!
//! El transporte usa stdin/stdout; los logs van a stderr (ver `main`).

use miette::{IntoDiagnostic, Result, WrapErr};
use serde_json::{json, Value};
use silc_frontend::error::ErrorFrontend;
use silc_frontend::lexer::lexear_con_errores;
use silc_frontend::parser::parsear;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

const PALABRAS_CLAVE: &[&str] = &[
    "definir",
    "tarea",
    "servicio",
    "estructura",
    "variante",
    "opcion",
    "modelo",
    "memoria_persistente",
    "capacidad",
    "modulo",
    "migrar",
    "requerir",
    "importar",
    "exportar",
    "asumir",
    "demostrar",
    "verificar",
    "let",
    "mut",
    "retornar",
    "si",
    "sino",
    "mientras",
    "para",
    "cada",
    "coincidir",
    "caso",
    "bajo",
    "restricciones",
    "permitir",
    "escuchar",
    "cuando",
    "llegue",
    "evento",
    "al",
    "acceder_campo",
    "valor_predeterminado",
];

const TIPOS: &[&str] = &[
    "Entero8",
    "Entero16",
    "Entero32",
    "Entero64",
    "EnteroSinSigno8",
    "EnteroSinSigno16",
    "EnteroSinSigno32",
    "EnteroSinSigno64",
    "Flotante32",
    "Flotante64",
    "Booleano",
    "Texto",
    "Unidad",
    "Lista",
    "Mapa",
    "Opcion",
];

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
                let items: Vec<Value> = PALABRAS_CLAVE
                    .iter()
                    .map(|k| json!({ "label": k, "kind": 14 }))
                    .chain(TIPOS.iter().map(|t| json!({ "label": t, "kind": 25 })))
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

/// Diagnósticos del documento: errores léxicos y el primer error de sintaxis.
fn diagnosticos(fuente: &str) -> Vec<Value> {
    let (tokens, errores_lexicos) = lexear_con_errores(fuente);
    let mut salida: Vec<Value> = errores_lexicos
        .into_iter()
        .map(|e| {
            diagnostico(
                fuente,
                e.span.start,
                e.span.len(),
                format!("Carácter no reconocido '{}'", e.caracter),
            )
        })
        .collect();

    if let Err(error) = parsear(&tokens) {
        let (inicio, longitud) = span_de_error(&error);
        salida.push(diagnostico(fuente, inicio, longitud, error.to_string()));
    }
    salida
}

fn span_de_error(error: &ErrorFrontend) -> (usize, usize) {
    match error {
        ErrorFrontend::CaracterDesconocido { span, .. }
        | ErrorFrontend::TokenInesperado { span, .. } => (span.offset(), span.len()),
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

    fn ids(procesado: &Procesado) -> Vec<Value> {
        procesado
            .mensajes
            .iter()
            .map(|m| m.get("method").cloned().unwrap_or(Value::Null))
            .collect()
    }

    #[test]
    fn initialize_declara_sincronizacion_y_completion() {
        let mut servidor = Servidor::default();
        let procesado = servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }));
        let capacidades = &procesado.mensajes[0]["result"]["capabilities"];
        assert_eq!(capacidades["textDocumentSync"], 1);
        assert!(capacidades["completionProvider"].is_object());
        assert!(!procesado.terminar);
    }

    #[test]
    fn did_open_publica_diagnosticos_de_sintaxis() {
        let mut servidor = Servidor::default();
        let procesado = servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": { "uri": "file:///a.sil", "text": "definir tarea f()\n    retornar 1\n" }
            }
        }));
        let notificacion = &procesado.mensajes[0];
        assert_eq!(notificacion["method"], "textDocument/publishDiagnostics");
        let diagnosticos = notificacion["params"]["diagnostics"].as_array().unwrap();
        assert_eq!(diagnosticos.len(), 1);
        assert_eq!(diagnosticos[0]["severity"], 1);
    }

    #[test]
    fn codigo_valido_no_publica_diagnosticos() {
        let mut servidor = Servidor::default();
        let procesado = servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///b.sil",
                    "text": "definir tarea f() -> Entero64:\n    retornar 0\n"
                }
            }
        }));
        assert!(procesado.mensajes[0]["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn did_change_reemplaza_el_contenido() {
        let mut servidor = Servidor::default();
        servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": { "uri": "file:///c.sil", "text": "definir tarea f() -> Entero64:\n    retornar 0\n" }
            }
        }));
        let procesado = servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///c.sil" },
                "contentChanges": [{ "text": "definir tarea f()\n    retornar 0\n" }]
            }
        }));
        assert_eq!(
            procesado.mensajes[0]["params"]["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn did_close_limpia_diagnosticos() {
        let mut servidor = Servidor::default();
        servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": { "uri": "file:///d.sil", "text": "definir tarea f()\n    retornar 1\n" }
            }
        }));
        let procesado = servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": "file:///d.sil" } }
        }));
        assert!(procesado.mensajes[0]["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(!servidor.documentos.contains_key("file:///d.sil"));
    }

    #[test]
    fn completion_devuelve_palabras_clave_y_tipos() {
        let mut servidor = Servidor::default();
        let procesado = servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/completion",
            "params": {}
        }));
        let items = procesado.mensajes[0]["result"]["items"].as_array().unwrap();
        assert!(items.iter().any(|i| i["label"] == "definir"));
        assert!(items.iter().any(|i| i["label"] == "Entero64"));
    }

    #[test]
    fn metodo_desconocido_devuelve_error_solo_si_es_request() {
        let mut servidor = Servidor::default();
        let request = servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "textDocument/definition",
            "params": {}
        }));
        assert_eq!(request.mensajes[0]["error"]["code"], -32601);

        let notificacion = servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/definition",
            "params": {}
        }));
        assert!(notificacion.mensajes.is_empty());
    }

    #[test]
    fn exit_termina_el_servidor() {
        let mut servidor = Servidor::default();
        let procesado = servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "method": "exit"
        }));
        assert!(procesado.terminar);
        assert_eq!(ids(&procesado).len(), 0);
    }

    #[test]
    fn caracter_invalido_genera_diagnostico_lexico() {
        let mut servidor = Servidor::default();
        let procesado = servidor.procesar(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": { "uri": "file:///e.sil", "text": "definir tarea f() -> Entero64:\n    retornar 0 @\n" }
            }
        }));
        let diagnosticos = procesado.mensajes[0]["params"]["diagnostics"]
            .as_array()
            .unwrap();
        assert!(diagnosticos
            .iter()
            .any(|d| d["message"].as_str().unwrap().contains("no reconocido")));
    }

    #[test]
    fn posicion_cuenta_utf16() {
        let texto = "a\nñb";
        assert_eq!(posicion(texto, 0), (0, 0));
        assert_eq!(posicion(texto, 2), (1, 0));
        assert_eq!(posicion(texto, 4), (1, 1));
    }
}
