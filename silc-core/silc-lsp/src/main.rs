use serde::Serialize;
use std::io::{self, Read, Write};
use std::collections::{HashMap, BTreeMap};
use std::sync::mpsc;
use tracing::{info, error, debug};

// ---------- JSON-RPC 3.17 Types (mínimos) ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
enum LspMessage {
    Initialize(InitializeParams),
    Initialized(InitializedParams),
    Shutdown(ShutdownParams),
    Exit(ExitParams),
    TextDocumentDidOpen(TextDocumentDidOpenParams),
    TextDocumentDidChange(TextDocumentDidChangeParams),
    TextDocumentDidClose(TextDocumentDidCloseParams),
    TextDocumentPublishDiagnostics(LspPublishDiagnostics),
    DebugStart,
    DebugStep,
    DebugPause,
    DebugRestore,
}

#[derive(Debug, Clone, Serialize)]
struct InitializeParams {
    cap: Option<ClientInfo>,
    root_uri: Option<String>,
    cap_abilities: ServerCapabilities,
}

#[derive(Debug, Clone, Serialize)]
struct ClientInfo {
    name: String,
    version: Option<i32>,
}

#[derive(Debug, Clone, Serialize)
struct ServerCapabilities {
    text_document_sync: TextDocumentSyncCapability,
    completion_provider: Option<CompletionCap>,
    hover_provider: bool,
    debug_provider: bool,
}

#[derive(Debug, Clone, Serialize)
struct CompletionCap {
    trigger_characters: Vec<String>,
}

#[derive(Debug, Clone, Serialize)
struct CancelParams {
    id: u64,
}

#[derive(Debug, Clone, Serialize)
struct TextDocumentDidOpenParams {
    text_document: TextDocumentIdentifier,
    text: String,
}

#[derive(Debug, Clone, Serialize)
struct TextDocumentDidChangeParams {
    text_document: TextDocumentIdentifier,
    content_changes: Vec<TextDocumentContentChangeEvent>,
}

#[derive(Debug, Clone, Serialize)
struct TextDocumentContentChangeEvent {
    range: Option<Range>,
    text: String,
    range_length: Option<i32>,
}

#[derive(Debug, Clone, Serialize)
struct TextDocumentIdentifier {
    uri: String,
}

#[derive(Debug, Clone, Serialize)
struct Range {
    start: Position,
    end: Position,
}

#[derive(Debug, Clone, Serialize)
struct Position {
    line: u32,
    character: u32,
}

// Diagnostic
#[derive(Debug, Clone, Serialize)
struct LspDiagnostic {
    range: Range,
    severity: i32,
    message: String,
    source: String,
}

// Publish diagnostics
#[derive(Debug, Clone, Serialize)
struct LspPublishDiagnostics {
    uri: String,
    diagnostics: Vec<LspDiagnostic>,
}

// Debug types
#[derive(Debug, Clone, Serialize)]
struct DebugStartParams {
    session_id: String,
    program_uri: String,
}

#[derive(Debug, Clone, Serialize)
struct DebugStepParams {
    session_id: String,
    target_step: i32,
}

#[derive(Debug, Clone, Serialize)
struct DebugPauseResult {
    session_id: String,
    current_ip: u64,
    current_arena: String,
    call_stack: Vec<String>,
}

#[derive(Debug, Clone, Serialize)
struct DebugRestoreParams {
    session_id: String,
    checkpoint_id: String,
}

// Response
#[derive(Debug, Clone, Serialize)
struct LspResponse {
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
}

// ---------- JSON-RPC 3.17 I/O ----------

fn send_message(msg: &impl Serialize) -> std::io::Result<()> {
    let json = serde_json::to_string(msg).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("JSON error: {}", e))
    })?;
    let len = json.len().to_string() + "\n";
    let mut stdout = io::stdout();
    stdout.write_all(len.as_bytes())?;
    stdout.write_all(b"\n\n")?;
    stdout.write_all(json.as_bytes())?;
    stdout.flush()
}

fn read_message() -> std::io::Result<LspMessage> {
    let mut len_line = String::new();
    io::stdin().read_line(&mut len_line).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error reading length: {}", e))
    })?;
    let len: usize = len_line.trim().parse().map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error parsing length: {}", e))
    })?;
    let mut sep = [0u8; 2];
    io::stdin().read_exact(&mut sep).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error reading separator: {}", e))
    })?;
    let mut json_bytes = vec![0u8; len];
    io::stdin().read_exact(&mut json_bytes).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error reading json: {}", e))
    })?;
    let json_str = String::from_utf8(json_bytes).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error converting json: {}", e))
    })?;
    let msg: LspMessage = serde_json::from_str(&json_str).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error parsing json message: {}", e))
    })?;
    Ok(msg)
}

// ---------- Core Debug Data Structures ----------

/// Evento de debugging registrado durante ejecución
#[derive(Debug, Clone, Serialize)]
struct DebugEvent {
    event_id: u64,
    timestamp: u64,
    task_id: String,
    arena: String,
    operation: DebugOperation,
    ip: u64,  // program counter
    old_arena_offset: u64,
    new_arena_offset: u64,
    success: bool,
}

#[derive(Debug, Clone, Serialize)]
enum DebugOperation {
    Alloc,
    Free,
    Assign,
    Call,
    Return,
    CheckSMT,
}

/// Grafo causal semántico (SCG) - estructura en memoria para time-travel
struct TimeTravelEngine {
    /// Todos los eventos registrados
    events: BTreeMap<u64, DebugEvent>,
    /// Map de checkpoint para restore rápido
    checkpoints: HashMap<String, u64>,
    /// Siguiente ID de evento
    next_id: u64,
    /// ID de sesión actual
    current_session: String,
}

impl TimeTravelEngine {
    fn new(session_id: String) -> Self {
        TimeTravelEngine {
            events: BTreeMap::new(),
            checkpoints: HashMap::new(),
            next_id: 0,
            current_session: session_id,
        }
    }

    /// Registra un evento durante ejecución
    fn record_event(&mut self, event: DebugEvent) {
        let id = self.next_id;
        self.events.insert(id, event);
        self.next_id += 1;
    }

    /// Hace checkpoint para restore
    fn make_checkpoint(&mut self, name: String) {
        self.checkpoints.insert(name, self.next_id);
    }

    /// Restore a checkpoint anterior
    fn restore_checkpoint(&mut self, name: &str) -> Option<BTreeMap<u64, DebugEvent>> {
        if let Some(&id) = self.checkpoints.get(name) {
            // Retornar todos los eventos hasta ese checkpoint
            let mut result = BTreeMap::new();
            for (&ev_id, event) in &self.events {
                if ev_id <= id {
                    result.insert(ev_id, event.clone());
                }
            }
            Some(result)
        } else {
            None
        }
    }

    /// Obtener eventos en rango temporal
    fn get_events_in_range(&self, start_id: u64, end_id: u64) -> BTreeMap<u64, DebugEvent> {
        self.events.range(start_id..=end_id).cloned()
    }

    /// Obtener camino de causa para un evento
    fn get_cause_chain(&self, event_id: u64) -> Vec<DebugEvent> {
        let mut chain = Vec::new();
        let mut current = Some(event_id);
        
        while let Some(&eid) = current {
            if let Some(event) = self.events.get(&eid) {
                chain.push(event.clone());
                // El "cause" sería el evento previo que condujo a este
                // Simplificación:events previos en tiempo
                if eid > 1 {
                    current = Some(eid - 1);
                } else {
                    current = None;
                }
            } else {
                break;
            }
        }
        chain
    }
}

// Thread-local engine (simplificado para demo)
thread_local!(static ENGINE: Option<TimeTravelEngine> = None);

// ---------- JSON-RPC I/O (mismos helpers que antes) ----------

fn send_message(msg: &impl Serialize) -> std::io::Result<()> {
    let json = serde_json::to_string(msg).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("JSON error: {}", e))
    })?;
    let len = json.len().to_string() + "\n";
    let mut stdout = io::stdout();
    stdout.write_all(len.as_bytes())?;
    stdout.write_all(b"\n\n")?;
    stdout.write_all(json.as_bytes())?;
    stdout.flush()
}

fn read_message() -> std::io::Result<LspMessage> {
    let mut len_line = String::new();
    io::stdin().read_line(&mut len_line).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error reading length: {}", e))
    })?;
    let len: usize = len_line.trim().parse().map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error parsing length: {}", e))
    })?;
    let mut sep = [0u8; 2];
    io::stdin().read_exact(&mut sep).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error reading separator: {}", e))
    })?;
    let mut json_bytes = vec![0u8; len];
    io::stdin().read_exact(&mut json_bytes).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error reading json: {}", e))
    })?;
    let json_str = String::from_utf8(json_bytes).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error converting json: {}", e))
    })?;
    let msg: LspMessage = serde_json::from_str(&json_str).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("error parsing json message: {}", e))
    })?;
    Ok(msg)
}

fn send_notification(name: &str, params: &impl Serialize) -> std::io::Result<()> {
    let msg = serde_json::json!({"jsonrpc": "3.0", "method": name, "params": params});
    send_message(&msg)
}

// ---------- Análisis Frontend (usando silc check) ----------

fn analyze_sil(sil_content: &str) -> (Vec<LspDiagnostic>, Option<Vec<serde_json::Value>>, Option<serde_json::Value>, Option<DebugEvent>) {
    use std::process::Command;
    
    let mut diagnostics = Vec::new();
    let mut completion_items: Option<Vec<serde_json::Value>> = None;
    let mut hover_value: Option<serde_json::Value> = None;
    let mut debug_event: Option<DebugEvent> = None;
    
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let mut f = std::io::BufWriter::new(std::fs::File::create(temp_file.path()).unwrap());
    write!(f, "{}", sil_content).unwrap();
    drop(f);
    
    let output = Command::new("silc")
        .args(["check", temp_file.path().to_str().unwrap()])
        .output();
    
    match output {
        Ok(out) => {
            if out.status.success() {
                return (diagnostics, completion_items, hover_value, debug_event);
            }
            let stderr = String::from_utf8_lossy(&out.stderr);
            for line in stderr.lines() {
                let line = line.trim();
                if line.is_empty() { continue; }
                if line.contains("TokenInesperado") {
                    diagnostics.push(LspDiagnostic {
                        range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 } },
                        severity: 1,
                        message: format!("Sintaxis: {}", line),
                        source: "silc".to_string(),
                    });
                } else if line.contains("verificación SMT falló") {
                    diagnostics.push(LspDiagnostic {
                        range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 } },
                        severity: 1,
                        message: format!("SMT: {}", line),
                        source: "silc".to_string(),
                    });
                } else if line.contains("Error") || line.contains("error") {
                    diagnostics.push(LspDiagnostic {
                        range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 } },
                        severity: 1,
                        message: format!("Error: {}", line),
                        source: "silc".to_string(),
                    });
                }
            }
        }
        Err(e) => {
            debug!("Error: {}", e);
        }
    }
    
    // Completado mínimo
    if sil_content.contains("definir") || sil_content.contains("asumir") {
        completion_items = Some(vec![
            serde_json::json!({"label": "definir"}),
            serde_json::json!({"label": "asumir"}),
        ]);
    }
    
    // Hover sobre "verificar"
    if sil_content.contains("verificar") {
        hover_value = Some(serde_json::json!({"value": "verificar: valida propiedad lógica con SMT"}));
    }
    
    // Evento de debug sintético para demostración
    debug_event = Some(DebugEvent {
        event_id: 1,
        timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        task_id: "main".to_string(),
        arena: "arena_local".to_string(),
        operation: DebugOperation::Alloc,
        ip: 0,
        old_arena_offset: 0,
        new_arena_offset: 8,
        success: true,
    });
    
    (diagnostics, completion_items, hover_value, debug_event)
}

// ---------- Main LSP Loop ----------

fn main() {
    info!("silc-lsp starting (debug mode)");
    
    // Inicializar motor de time-travel por sesión
    let session_id = "session_default".to_string();
    ENGINE.with(|engine| {
        *engine = Some(TimeTravelEngine::new(session_id));
    });
    
    loop {
        let msg = match read_message() {
            Ok(m) => m,
            Err(e) => {
                error!("Error reading: {}", e);
                break;
            }
        };
        
        match msg {
            LspMessage::Initialize(params) => {
                let caps = serde_json::json!({
                    "textDocumentSync": 2,
                    "completionProvider": {"triggerCharacters": [" ", "\n"]},
                    "hoverProvider": true,
                    "debugProvider": true,
                });
                let resp = serde_json::json!({"capabilities": caps});
                send_message(&serde_json::json!({
                    "jsonrpc": "3.0", "id": None, "result": resp
                })).unwrap();
            }
            LspMessage::Initialized(_) => { info!("initialized"); }
            LspMessage::Shutdown(_) => { info!("shutdown"); }
            LspMessage::Exit(_) => { info!("exit"); break; }
            LspMessage::TextDocumentDidOpen(params) => {
                let (diagnostics, _, _, _debug_event) = analyze_sil(&params.text);
                let lsp_diags: Vec<serde_json::Value> = diagnostics.iter().map(|d| {
                    serde_json::json!({
                        "range": {"start": {"line": d.range.start.line, "character": d.range.start.character}, "end": {"line": d.range.end.line, "character": d.range.end.character}},
                        "severity": d.severity, "message": d.message, "source": d.source
                    })
                }).collect();
                let _ = send_notification("textDocument/publishDiagnostics", serde_json::json!({
                    "uri": params.text_document.uri, "diagnostics": lsp_diags
                }));
            }
            LspMessage::TextDocumentDidChange => {
                // Simplificado: volver a analizar
            }
            LspMessage::DebugStart => {
                // Iniciar sesión de debug
                if let Some(engine) = ENGINE.with(|e| e.clone()) {
                    // Engine ya inicializado
                }
            }
            LspMessage::DebugStep => {
                // Avanzar un paso en el tiempo
                if let Some(mut engine) = ENGINE.with(|e| e.clone()) {
                    // Obtener próximo evento y reportar
                    // Simplificado: reportar siguiente evento
                    if let Some((&id, event)) = engine.events.range(1..).next() {
                        let _ = send_message(&serde_json::json!({
                            "jsonrpc": "3.0",
                            "id": None,
                            "result": serde_json::json!({
                                "event": serde_json::json!({
                                    "id": event.event_id,
                                    "timestamp": event.timestamp,
                                    "task_id": event.task_id,
                                    "arena": event.arena,
                                    "operation": format!("{:?}", event.operation),
                                    "ip": event.ip,
                                    "success": event.success
                                })
                            })
                        })).unwrap();
                    }
                }
            }
            LspMessage::DebugPause => {
                // Reportar estado actual del motor
                if let Some(engine) = ENGINE.with(|e| e.clone()) {
                    let events_count = engine.events.len();
                    let _ = send_message(&serde_json::json!({
                        "jsonrpc": "3.0",
                        "id": None,
                        "result": serde_json::json!({
                            "events_recorded": events_count,
                            "checkpoints": engine.checkpoints.keys().collect::<Vec<_>>()
                        })
                    })).unwrap();
                }
            }
            LspMessage::DebugRestore => {
                // Restaurar checkpoint
                info!("Debug restore requested");
            }
        }
    }
}

fn send_notification(name: &str, params: &impl Serialize) -> std::io::Result<()> {
    let msg = serde_json::json!({"jsonrpc": "3.0", "method": name, "params": params});
    send_message(&msg)
}