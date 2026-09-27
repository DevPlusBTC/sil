#!/usr/bin/env python3
"""
sil_lsp.py - Servidor de Lenguaje (LSP) Oficial para SIL.
Proporciona soporte de IDE en tiempo real mediante el protocolo JSON-RPC 2.0.
"""

import sys
import json
import re
from typing import Dict, Any, List, Optional

# ==============================================================================
# LEXER Y PARSER SIMPLIFICADOS PARA LSP (INCREMENTAL)
# ==============================================================================

KEYWORDS_LSP = {
    "definir", "tarea", "servicio", "estructura", "variante", "opcion",
    "memoria_persistente", "migrar", "capacidad", "requerir", "importar",
    "exportar", "modulo", "vincular", "biblioteca_nativa", "funcion_externa",
    "retornar", "verificar", "que", "como", "con", "desde", "hasta",
    "bajo", "restricciones", "mientras", "si", "para", "cada", "en",
    "usando", "permitir", "escuchar", "cuando", "llegue", "evento",
    "asumir", "demostrar", "guardar", "en", "de", "forma", "convertir",
    "a", "enviar", "coincidir", "caso", "entonces", "cualquier", "otro",
    "let", "mut", "verdadero", "falso", "nulo", "atomica", "asincrona",
    "en_memoria", "sea", "mayor", "menor", "igual", "esta_activo",
    "probar", "propiedad", "al", "acceder_campo", "valor_predeterminado"
}

TIPOS_LSP = {
    "Entero64", "Flotante64", "Booleano", "Texto", "CapacidadHardware",
    "Void", "Tensor", "Lista", "Mapa", "Conjunto", "Tupla", "USD", "EUR"
}

class SILDocument:
    def __init__(self, uri: str, text: str):
        self.uri = uri
        self.text = text
        self.version = 0
        self.diagnostics: List[Dict] = []

    def update(self, text: str, version: int):
        self.text = text
        self.version = version

class SILLanguageServer:
    def __init__(self):
        self.documentos: Dict[str, SILDocument] = {}
        self.running = True

    def iniciar(self):
        """Bucle principal de escucha de solicitudes JSON-RPC vía stdio."""
        while self.running:
            try:
                linea = sys.stdin.readline()
                if not linea:
                    break
                if linea.startswith("Content-Length:"):
                    longitud = int(linea.split(":")[1].strip())
                    sys.stdin.readline()  # Leer línea en blanco obligatoria
                    cuerpo = sys.stdin.read(longitud)
                    solicitud = json.loads(cuerpo)
                    self.procesar_solicitud(solicitud)
            except Exception as e:
                sys.stderr.write(f"[LSP ERROR]: {str(e)}\n")

    def procesar_solicitud(self, req: dict):
        metodo = req.get("method")
        msg_id = req.get("id")

        if metodo == "initialize":
            self.responder(msg_id, {
                "capabilities": {
                    "textDocumentSync": 1,  # Full sync
                    "completionProvider": {"triggerCharacters": [":", ".", " ", "("]},
                    "hoverProvider": True,
                    "definitionProvider": True,
                    "documentSymbolProvider": True
                }
            })

        elif metodo == "initialized":
            pass  # Cliente listo

        elif metodo == "textDocument/didOpen":
            doc = req["params"]["textDocument"]
            uri = doc["uri"]
            self.documentos[uri] = SILDocument(uri, doc["text"])
            self.validar_documento(uri, doc["text"])

        elif metodo == "textDocument/didChange":
            uri = req["params"]["textDocument"]["uri"]
            cambios = req["params"]["contentChanges"]
            if uri in self.documentos:
                # Para simplicidad: full sync
                nuevo_texto = cambios[0]["text"]
                self.documentos[uri].update(nuevo_texto, req["params"]["textDocument"]["version"])
                self.validar_documento(uri, nuevo_texto)

        elif metodo == "textDocument/completion":
            pos = req["params"]["position"]
            uri = req["params"]["textDocument"]["uri"]
            items = self.obtener_completado(uri, pos)
            self.responder(msg_id, items)

        elif metodo == "textDocument/hover":
            pos = req["params"]["position"]
            uri = req["params"]["textDocument"]["uri"]
            hover = self.obtener_hover(uri, pos)
            self.responder(msg_id, hover)

        elif metodo == "shutdown":
            self.responder(msg_id, None)
            self.running = False

    def validar_documento(self, uri: str, texto: str):
        """Análisis sintáctico rápido + verificación de patrones básicos."""
        diagnostics = []

        lines = texto.split('\n')
        for i, line in enumerate(lines):
            stripped = line.strip()

            # Detectar tareas sin bloque de restricciones (warning)
            if stripped.startswith("definir tarea"):
                # Buscar si hay 'bajo restricciones:' en las siguientes líneas
                bloque = "\n".join(lines[i:])
                if "bajo restricciones:" not in bloque:
                    diagnostics.append({
                        "range": {"start": {"line": i, "character": 0}, "end": {"line": i, "character": len(line)}},
                        "severity": 2,  # Warning
                        "source": "SIL Linter",
                        "message": "Tarea sin bloque 'bajo restricciones:' — se recomienda especificar gestión_memoria, latencia, etc."
                    })

            # Detectar 'verificar' sin 'que'
            if "verificar" in stripped and "que" not in stripped:
                col = stripped.find("verificar")
                diagnostics.append({
                    "range": {"start": {"line": i, "character": col}, "end": {"line": i, "character": col + 9}},
                    "severity": 1,  # Error
                    "source": "SIL Parser",
                    "message": "Sintaxis inválida: 'verificar' debe ir seguido de 'que <condición>'"
                })

            # Detectar 'asumir'/'demostrar' sin condición
            for kw in ["asumir", "demostrar"]:
                if stripped == kw:
                    col = stripped.find(kw)
                    diagnostics.append({
                        "range": {"start": {"line": i, "character": col}, "end": {"line": i, "character": col + len(kw)}},
                        "severity": 1,
                        "source": "SIL Parser",
                        "message": f"'{kw}' requiere una condición lógica después"
                    })

        self.enviar_diagnosticos(uri, diagnostics)

    def obtener_completado(self, uri: str, position: Dict) -> List[Dict]:
        """Provee autocompletado contextual."""
        return [
            {"label": "tarea", "kind": 14, "detail": "Declarar una función/tarea en SIL", "insertText": "definir tarea ${1:nombre}(${2:param}: ${3:Tipo}) -> ${4:Tipo}:\n\t${0}"},
            {"label": "verificar", "kind": 14, "detail": "Aseveración estática SMT", "insertText": "verificar que ${1:condición}"},
            {"label": "asumir", "kind": 14, "detail": "Precondición lógica SMT", "insertText": "asumir ${1:condición}"},
            {"label": "demostrar", "kind": 14, "detail": "Postcondición/invariante SMT", "insertText": "demostrar ${1:condición}"},
            {"label": "retornar", "kind": 14, "detail": "Retorno de valor", "insertText": "retornar ${1:expresión}"},
            {"label": "let", "kind": 14, "detail": "Declarar variable inmutable", "insertText": "let ${1:nombre}: ${2:Tipo} = ${3:valor}"},
            {"label": "let mut", "kind": 14, "detail": "Declarar variable mutable", "insertText": "let mut ${1:nombre}: ${2:Tipo} = ${3:valor}"},
            {"label": "coincidir", "kind": 14, "detail": "Pattern matching exhaustivo", "insertText": "coincidir ${1:expr}:\n\ten caso ${2:Patron} entonces:\n\t\t${0}\n\ten cualquier otro caso entonces:\n\t\t${0}"},
            {"label": "bajo restricciones:", "kind": 14, "detail": "Bloque de restricciones de hardware", "insertText": "bajo restricciones:\n\tgestion_memoria: arena\n\tlatencia_maxima: 200us\n\tconcurrencia: modelo_actores_masivo\n\tseguridad_tipo: prueba_formal_matematica"},
            {"label": "Entero64", "kind": 7, "detail": "Tipo primitivo entero de 64 bits con verificación SMT de overflow"},
            {"label": "Flotante64", "kind": 7, "detail": "Tipo primitivo flotante IEEE 754 de 64 bits"},
            {"label": "Booleano", "kind": 7, "detail": "Tipo primitivo booleano estricto (0 o 1)"},
            {"label": "Texto", "kind": 7, "detail": "Cadena UTF-8 inmutable (puntero + longitud)"},
            {"label": "CapacidadHardware", "kind": 7, "detail": "Token de capacidad firmado por TPM 2.0 / Secure Enclave"},
            {"label": "Lista de ", "kind": 7, "detail": "Colección contigua en Arena, tamaño dinámico"},
            {"label": "Mapa de  a ", "kind": 7, "detail": "Mapa hash en Arena, claves únicas"},
            {"label": "definir estructura", "kind": 14, "detail": "Definir tipo producto (Product Type)", "insertText": "definir estructura ${1:Nombre}:\n\t${2:campo} como ${3:Tipo}\n"},
            {"label": "definir variante", "kind": 14, "detail": "Definir tipo suma (Sum Type / ADT)", "insertText": "definir variante ${1:Nombre}:\n\topcion ${2:Caso} con datos (${3:campo}: ${4:Tipo})\n"},
        ]

    def obtener_hover(self, uri: str, position: Dict) -> Dict:
        """Provee información hover para palabras clave."""
        doc = self.documentos.get(uri)
        if not doc:
            return {"contents": []}

        line = doc.text.split('\n')[position["line"]] if position["line"] < len(doc.text.split('\n')) else ""
        word = self._extract_word_at(line, position["character"])

        docs = {
            "tarea": "Declara una función de SIL. Sintaxis: `definir tarea nombre(params) -> Tipo: cuerpo`",
            "verificar": "Aseveración verificada en compile-time por SMT. `verificar que condición`",
            "asumir": "Precondición para el SMT Solver. `asumir condición`",
            "demostrar": "Postcondición que el SMT debe probar. `demostrar condición`",
            "Entero64": "Entero de 64 bits con rango [-2^63, 2^63-1]. Overflow verificado por SMT.",
            "CapacidadHardware": "Token de autoridad para I/O, firmado por TPM 2.0. TTL micro-segundos.",
            "bajo restricciones:": "Bloque de contratos de hardware: latencia, memoria, concurrencia, seguridad.",
        }

        content = docs.get(word, f"Símbolo SIL: {word}")
        return {"contents": [{"kind": "markdown", "value": f"**{word}**\n\n{content}"}]}

    def _extract_word_at(self, line: str, char: int) -> str:
        # Buscar palabra bajo el cursor
        for match in re.finditer(r'\b\w+\b', line):
            if match.start() <= char < match.end():
                return match.group(0)
        return ""

    def responder(self, msg_id: Any, result: Any):
        response = {"jsonrpc": "2.0", "id": msg_id, "result": result}
        output = json.dumps(response)
        sys.stdout.write(f"Content-Length: {len(output)}\r\n\r\n{output}")
        sys.stdout.flush()

    def enviar_diagnosticos(self, uri: str, diagnostics: List[Dict]):
        notification = {
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "diagnostics": diagnostics}
        }
        output = json.dumps(notification)
        sys.stdout.write(f"Content-Length: {len(output)}\r\n\r\n{output}")
        sys.stdout.flush()


if __name__ == "__main__":
    server = SILLanguageServer()
    server.iniciar()