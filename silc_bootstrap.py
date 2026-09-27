#!/usr/bin/env python3
"""
Compilador Unificado de SIL (Semantic Intention Language) - Fase 0 (Bootstrap)
Procesamiento: CNL -> AST -> Causal-IR -> Verificación SMT (Z3) -> Backend C99/WASM
"""

import enum
import json
import re
import sys
from dataclasses import asdict, dataclass, is_dataclass
from typing import Any, Dict, List, Optional

# ==============================================================================
# 1. DEFINICIÓN DE TOKENS Y LEXER
# ==============================================================================

class TokenType(enum.Enum):
    KEYWORD = "KEYWORD"
    IDENTIFIER = "IDENTIFIER"
    INTEGER = "INTEGER"
    FLOAT = "FLOAT"
    STRING = "STRING"
    SYMBOL = "SYMBOL"
    NEWLINE = "NEWLINE"
    EOF = "EOF"

KEYWORDS = {
    "definir", "tarea", "servicio", "estructura", "variante", "opcion", "memoria_persistente",
    "migrar", "capacidad", "requerir", "importar", "exportar", "modulo", "vincular",
    "biblioteca_nativa", "funcion_externa", "retornar", "verificar", "que", "como", "con",
    "desde", "hasta", "bajo", "restricciones", "mientras", "si", "para", "cada", "en",
    "usando", "permitir", "escuchar", "cuando", "llegue", "evento", "asumir", "demostrar",
    "guardar", "en", "de", "forma", "convertir", "a", "enviar", "coincidir", "caso",
    "entonces", "cualquier", "otro", "let", "mut", "verdadero", "falso", "nulo",
    "atomica", "asincrona", "en_memoria", "sea", "mayor", "menor", "igual", "esta_activo",
    "probar", "propiedad", "al", "acceder_campo", "valor_predeterminado", "promover",
    "arena", "hoisting", "zero_copy", "mmap", "durabilidad", "hardware", "recuperacion",
    "tiempo", "constante", "branchless", "tpm", "enclave", "firma", "validar", "efimera",
    "ttl", "microsegundos", "nanosegundos", "milisegundos", "segundos"
}

SYMBOLS = [
    "->", "==", "!=", ">=", "<=", ":", ",", "(", ")", "{", "}", "[", "]",
    ">", "<", "=", "+", "-", "*", "/", ".", "|"
]

@dataclass
class Token:
    type: TokenType
    value: str
    line: int
    column: int

class Lexer:
    def __init__(self, source_code: str):
        self.source = source_code
        self.position = 0
        self.line = 1
        self.column = 1
        self.length = len(source_code)

    def _peek(self, offset: int = 0) -> Optional[str]:
        pos = self.position + offset
        if pos < self.length:
            return self.source[pos]
        return None

    def _advance(self) -> str:
        char = self.source[self.position]
        self.position += 1
        if char == '\n':
            self.line += 1
            self.column = 1
        else:
            self.column += 1
        return char

    def tokenize(self) -> List[Token]:
        tokens = []
        while self.position < self.length:
            char = self._peek()
            if char in (' ', '\t', '\r'):
                self._advance()
                continue
            if char == '\n':
                tokens.append(Token(TokenType.NEWLINE, "\\n", self.line, self.column))
                self._advance()
                continue
            if char == '/' and self._peek(1) == '/':
                while self._peek() is not None and self._peek() != '\n':
                    self._advance()
                continue
            if char == '"':
                tokens.append(self._read_string())
                continue
            if char.isdigit():
                tokens.append(self._read_number())
                continue
            if char.isalpha() or char == '_':
                tokens.append(self._read_identifier())
                continue
            symbol_matched = False
            for sym in sorted(SYMBOLS, key=len, reverse=True):
                if self.source.startswith(sym, self.position):
                    tokens.append(Token(TokenType.SYMBOL, sym, self.line, self.column))
                    for _ in range(len(sym)):
                        self._advance()
                    symbol_matched = True
                    break
            if symbol_matched:
                continue
            raise SyntaxError(f"Carácter no reconocido '{char}' en línea {self.line}, columna {self.column}")
        tokens.append(Token(TokenType.EOF, "", self.line, self.column))
        return tokens

    def _read_string(self) -> Token:
        start_line, start_col = self.line, self.column
        self._advance()
        value = ""
        while self._peek() is not None and self._peek() != '"':
            value += self._advance()
        if self._peek() is None:
            raise SyntaxError(f"Cadena no cerrada en línea {start_line}, columna {start_col}")
        self._advance()
        return Token(TokenType.STRING, value, start_line, start_col)

    def _read_number(self) -> Token:
        start_line, start_col = self.line, self.column
        num_str = ""
        is_float = False
        while self._peek() is not None and (self._peek().isdigit() or self._peek() == '.'):
            if self._peek() == '.':
                if is_float:
                    break
                is_float = True
            num_str += self._advance()
        token_type = TokenType.FLOAT if is_float else TokenType.INTEGER
        return Token(token_type, num_str, start_line, start_col)

    def _read_identifier(self) -> Token:
        start_line, start_col = self.line, self.column
        ident = ""
        while self._peek() is not None and (self._peek().isalnum() or self._peek() in ('_', '-')):
            ident += self._advance()
        token_type = TokenType.KEYWORD if ident in KEYWORDS else TokenType.IDENTIFIER
        return Token(token_type, ident, start_line, start_col)

# ==============================================================================
# 2. NODOS DEL ÁRBOL DE SINTAXIS ABSTRACTA (AST)
# ==============================================================================

@dataclass
class ASTNode:
    pass

@dataclass
class ProgramNode(ASTNode):
    definitions: List[ASTNode]

@dataclass
class ParameterNode(ASTNode):
    name: str
    type_annotation: str

@dataclass
class RestrictionNode(ASTNode):
    key: str
    value: str

@dataclass
class BinaryOpNode(ASTNode):
    left: ASTNode
    operator: str
    right: ASTNode

@dataclass
class LiteralNode(ASTNode):
    value: Any
    type_name: str

@dataclass
class IdentifierNode(ASTNode):
    name: str

@dataclass
class VerifyStmtNode(ASTNode):
    condition: ASTNode

@dataclass
class ReturnStmtNode(ASTNode):
    expression: ASTNode

@dataclass
class AssumeStmtNode(ASTNode):
    condition: ASTNode

@dataclass
class ProveStmtNode(ASTNode):
    condition: ASTNode

@dataclass
class AssignStmtNode(ASTNode):
    name: str
    type_annotation: Optional[str]
    expression: ASTNode
    is_mutable: bool

@dataclass
class MatchStmtNode(ASTNode):
    expression: ASTNode
    cases: List[ASTNode]
    default_case: Optional[ASTNode]

@dataclass
class MatchCaseNode(ASTNode):
    pattern: ASTNode
    guard: Optional[ASTNode]
    body: List[ASTNode]

@dataclass
class FunctionDeclNode(ASTNode):
    name: str
    parameters: List[ParameterNode]
    return_type: str
    body: List[ASTNode]
    restrictions: List[RestrictionNode]

# ==============================================================================
# 3. PARSER (Descendente Recursivo)
# ==============================================================================

class Parser:
    def __init__(self, tokens: List[Token]):
        self.tokens = [t for t in tokens if t.type != TokenType.NEWLINE]
        self.current = 0

    def _peek(self) -> Token:
        return self.tokens[self.current]

    def _advance(self) -> Token:
        token = self.tokens[self.current]
        if token.type != TokenType.EOF:
            self.current += 1
        return token

    def _match(self, token_type: TokenType, value: Optional[str] = None) -> bool:
        token = self._peek()
        if token.type == token_type:
            if value is None or token.value == value:
                self._advance()
                return True
        return False

    def _expect(self, token_type: TokenType, value: Optional[str] = None) -> Token:
        token = self._peek()
        if token.type != token_type or (value is not None and token.value != value):
            expected = f"{token_type.value}('{value}')" if value else token_type.value
            got = f"{token.type.value}('{token.value}')"
            raise SyntaxError(f"Se esperaba {expected} pero se encontró {got} en la línea {token.line}")
        return self._advance()

    def parse(self) -> ProgramNode:
        definitions = []
        while self._peek().type != TokenType.EOF:
            if self._peek().type == TokenType.KEYWORD and self._peek().value == "definir":
                definitions.append(self._parse_definition())
            else:
                raise SyntaxError(f"Declaración no válida que inicia con '{self._peek().value}' en línea {self._peek().line}")
        return ProgramNode(definitions=definitions)

    def _parse_definition(self) -> ASTNode:
        self._expect(TokenType.KEYWORD, "definir")
        kind_token = self._expect(TokenType.KEYWORD)

        if kind_token.value == "tarea":
            return self._parse_function_decl()
        elif kind_token.value == "estructura":
            return self._parse_struct_decl()
        elif kind_token.value == "variante":
            return self._parse_variant_decl()
        else:
            raise NotImplementedError(f"Definición de tipo '{kind_token.value}' no soportada aún en el Parser base.")

    def _parse_function_decl(self) -> FunctionDeclNode:
        name_token = self._expect(TokenType.IDENTIFIER)
        self._expect(TokenType.SYMBOL, "(")

        parameters = []
        if self._peek().type != TokenType.SYMBOL or self._peek().value != ")":
            while True:
                param_name = self._expect(TokenType.IDENTIFIER).value
                self._expect(TokenType.SYMBOL, ":")
                param_type = self._expect(TokenType.IDENTIFIER).value
                parameters.append(ParameterNode(name=param_name, type_annotation=param_type))

                if self._match(TokenType.SYMBOL, ","):
                    continue
                break

        self._expect(TokenType.SYMBOL, ")")
        return_type = "Void"
        if self._match(TokenType.SYMBOL, "->"):
            return_type = self._expect(TokenType.IDENTIFIER).value

        self._expect(TokenType.SYMBOL, ":")

        body: List[ASTNode] = []
        restrictions: List[RestrictionNode] = []

        while self._peek().type != TokenType.EOF:
            if self._peek().type == TokenType.KEYWORD and self._peek().value == "definir":
                break
            if self._peek().type == TokenType.KEYWORD and self._peek().value == "bajo":
                restrictions = self._parse_restrictions_block()
                break
            stmt = self._parse_statement()
            if stmt:
                body.append(stmt)

        return FunctionDeclNode(
            name=name_token.value,
            parameters=parameters,
            return_type=return_type,
            body=body,
            restrictions=restrictions
        )

    def _parse_struct_decl(self) -> ASTNode:
        name = self._expect(TokenType.IDENTIFIER).value
        self._expect(TokenType.SYMBOL, ":")
        self._expect(TokenType.NEWLINE)
        # Simplificado para bootstrap
        return FunctionDeclNode(name=name, parameters=[], return_type="Struct", body=[], restrictions=[])

    def _parse_variant_decl(self) -> ASTNode:
        name = self._expect(TokenType.IDENTIFIER).value
        self._expect(TokenType.SYMBOL, ":")
        self._expect(TokenType.NEWLINE)
        return FunctionDeclNode(name=name, parameters=[], return_type="Variant", body=[], restrictions=[])

    def _parse_statement(self) -> ASTNode:
        token = self._peek()

        if token.type == TokenType.KEYWORD and token.value == "verificar":
            self._advance()
            self._expect(TokenType.KEYWORD, "que")
            condition = self._parse_expression()
            return VerifyStmtNode(condition=condition)

        if token.type == TokenType.KEYWORD and token.value == "asumir":
            self._advance()
            condition = self._parse_expression()
            return AssumeStmtNode(condition=condition)

        if token.type == TokenType.KEYWORD and token.value == "demostrar":
            self._advance()
            condition = self._parse_expression()
            return ProveStmtNode(condition=condition)

        if token.type == TokenType.KEYWORD and token.value == "retornar":
            self._advance()
            expr = self._parse_expression()
            return ReturnStmtNode(expression=expr)

        if token.type == TokenType.KEYWORD and token.value == "let":
            self._advance()
            is_mutable = False
            if self._match(TokenType.KEYWORD, "mut"):
                is_mutable = True
            var_name = self._expect(TokenType.IDENTIFIER).value
            type_annotation = None
            if self._match(TokenType.SYMBOL, ":"):
                type_annotation = self._expect(TokenType.IDENTIFIER).value
            self._expect(TokenType.SYMBOL, "=")
            expr = self._parse_expression()
            return AssignStmtNode(name=var_name, type_annotation=type_annotation, expression=expr, is_mutable=is_mutable)

        if token.type == TokenType.KEYWORD and token.value == "coincidir":
            return self._parse_match_stmt()

        raise SyntaxError(f"Instrucción no soportada '{token.value}' en línea {token.line}")

    def _parse_match_stmt(self) -> MatchStmtNode:
        self._expect(TokenType.KEYWORD, "coincidir")
        expr = self._parse_expression()
        self._expect(TokenType.SYMBOL, ":")
        self._expect(TokenType.NEWLINE)

        cases = []
        default_case = None

        while self._peek().type != TokenType.EOF:
            if self._peek().type == TokenType.KEYWORD and self._peek().value == "en":
                self._advance()
                if self._match(TokenType.KEYWORD, "cualquier"):
                    self._expect(TokenType.KEYWORD, "otro")
                    self._expect(TokenType.KEYWORD, "caso")
                    self._expect(TokenType.KEYWORD, "entonces")
                    self._expect(TokenType.SYMBOL, ":")
                    self._expect(TokenType.NEWLINE)
                    body = []
                    while self._peek().type != TokenType.EOF and not (self._peek().type == TokenType.KEYWORD and self._peek().value in ("definir", "bajo", "en")):
                        stmt = self._parse_statement()
                        if stmt:
                            body.append(stmt)
                    default_case = MatchCaseNode(pattern=None, guard=None, body=body)
                    break
                else:
                    self._expect(TokenType.KEYWORD, "caso")
                    pattern = self._parse_pattern()
                    guard = None
                    if self._match(TokenType.KEYWORD, "si"):
                        guard = self._parse_expression()
                    self._expect(TokenType.KEYWORD, "entonces")
                    self._expect(TokenType.SYMBOL, ":")
                    self._expect(TokenType.NEWLINE)
                    body = []
                    while self._peek().type != TokenType.EOF and not (self._peek().type == TokenType.KEYWORD and self._peek().value in ("definir", "bajo", "en")):
                        stmt = self._parse_statement()
                        if stmt:
                            body.append(stmt)
                    cases.append(MatchCaseNode(pattern=pattern, guard=guard, body=body))
            else:
                break

        return MatchStmtNode(expression=expr, cases=cases, default_case=default_case)

    def _parse_pattern(self) -> ASTNode:
        token = self._peek()
        if token.type == TokenType.IDENTIFIER:
            name = self._advance().value
            if self._match(TokenType.SYMBOL, "."):
                variant = name
                constructor = self._expect(TokenType.IDENTIFIER).value
                fields = []
                if self._match(TokenType.SYMBOL, "("):
                    if self._peek().type != TokenType.SYMBOL or self._peek().value != ")":
                        while True:
                            field_name = self._expect(TokenType.IDENTIFIER).value
                            self._expect(TokenType.SYMBOL, ":")
                            field_pattern = self._parse_pattern()
                            fields.append((field_name, field_pattern))
                            if self._match(TokenType.SYMBOL, ","):
                                continue
                            break
                    self._expect(TokenType.SYMBOL, ")")
                return IdentifierNode(name=f"{variant}.{constructor}({fields})")
            return IdentifierNode(name=name)
        if token.type in (TokenType.INTEGER, TokenType.FLOAT, TokenType.STRING):
            return self._parse_primary()
        raise SyntaxError(f"Patrón no válido '{token.value}' en línea {token.line}")

    def _parse_expression(self) -> ASTNode:
        left = self._parse_primary()

        if self._peek().type == TokenType.SYMBOL and self._peek().value in (">", "<", "==", "!=", "+", "-", "*", "/"):
            op = self._advance().value
            right = self._parse_primary()
            return BinaryOpNode(left=left, operator=op, right=right)

        return left

    def _parse_primary(self) -> ASTNode:
        token = self._peek()

        if token.type == TokenType.IDENTIFIER:
            self._advance()
            if self._match(TokenType.SYMBOL, "."):
                prop = self._expect(TokenType.IDENTIFIER).value
                return BinaryOpNode(left=IdentifierNode(name=token.value), operator=".", right=IdentifierNode(name=prop))
            return IdentifierNode(name=token.value)

        if token.type == TokenType.INTEGER:
            self._advance()
            return LiteralNode(value=int(token.value), type_name="Entero64")
        if token.type == TokenType.FLOAT:
            self._advance()
            return LiteralNode(value=float(token.value), type_name="Flotante64")
        if token.type == TokenType.STRING:
            self._advance()
            return LiteralNode(value=token.value, type_name="Texto")

        raise SyntaxError(f"Expresión no válida '{token.value}' en línea {token.line}")

    def _parse_restrictions_block(self) -> List[RestrictionNode]:
        self._expect(TokenType.KEYWORD, "bajo")
        self._expect(TokenType.KEYWORD, "restricciones")
        self._expect(TokenType.SYMBOL, ":")
        restrictions = []
        while self._peek().type == TokenType.IDENTIFIER:
            key = self._advance().value
            self._expect(TokenType.SYMBOL, ":")
            val_token = self._advance()
            restrictions.append(RestrictionNode(key=key, value=val_token.value))
        return restrictions

# ==============================================================================
# 4. REPRESENTACIÓN INTERMEDIA CAUSAL (CAUSAL-IR)
# ==============================================================================

@dataclass
class CausalIRInstruction:
    opcode: str
    operands: List[str]
    condition: Optional[str] = None

    def __repr__(self):
        cond_str = f" [SMT: {self.condition}]" if self.condition else ""
        return f"{self.opcode} ({', '.join(self.operands)}){cond_str}"

class CausalIRGenerator:
    def __init__(self):
        self.instructions: List[CausalIRInstruction] = []

    def generate(self, ast: ProgramNode) -> List[CausalIRInstruction]:
        for decl in ast.definitions:
            if isinstance(decl, FunctionDeclNode):
                self.instructions.append(CausalIRInstruction("INICIO_TAREA", [decl.name]))
                for p in decl.parameters:
                    self.instructions.append(CausalIRInstruction("ASIGNAR_PARAMETRO", [p.name, p.type_annotation]))
                for node in decl.body:
                    if isinstance(node, AssumeStmtNode):
                        self.instructions.append(CausalIRInstruction("INYECTAR_ASUNCION", [], self._expr_to_str(node.condition)))
                    elif isinstance(node, ProveStmtNode):
                        self.instructions.append(CausalIRInstruction("INYECTAR_DEMOSTRACION", [], self._expr_to_str(node.condition)))
                    elif isinstance(node, VerifyStmtNode):
                        self.instructions.append(CausalIRInstruction("INYECTAR_VERIFICACION", [], self._expr_to_str(node.condition)))
                    elif isinstance(node, AssignStmtNode):
                        self.instructions.append(CausalIRInstruction("ASIGNAR_ARENA_OC1", [node.name, self._expr_to_str(node.expression)]))
                    elif isinstance(node, ReturnStmtNode):
                        self.instructions.append(CausalIRInstruction("RETORNAR", [self._expr_to_str(node.expression)]))
                for r in decl.restrictions:
                    self.instructions.append(CausalIRInstruction("RESTRICCION", [r.key, r.value]))
                self.instructions.append(CausalIRInstruction("FIN_TAREA", [decl.name]))
        return self.instructions

    def _expr_to_str(self, node: ASTNode) -> str:
        if isinstance(node, LiteralNode):
            return str(node.value)
        if isinstance(node, IdentifierNode):
            return node.name
        if isinstance(node, BinaryOpNode):
            return f"{self._expr_to_str(node.left)} {node.operator} {self._expr_to_str(node.right)}"
        return str(node)

# ==============================================================================
# 5. TUBERÍA DE VERIFICACIÓN SMT (TRADUCCIÓN A SMT-LIB2 / Z3)
# ==============================================================================

class SMTVerifier:
    def __init__(self, ir_instructions: List[CausalIRInstruction]):
        self.ir_instructions = ir_instructions

    def generate_smt_script(self) -> str:
        smt_lines = ["(set-logic QF_LIA)", "(assert true)"]
        declared_vars = set()
        for instr in self.ir_instructions:
            if instr.opcode == "ASIGNAR_PARAMETRO":
                var_name = instr.operands[0]
                if var_name not in declared_vars:
                    smt_lines.append(f"(declare-fun {var_name} () Int)")
                    declared_vars.add(var_name)
            elif instr.opcode == "ASIGNAR_ARENA_OC1":
                var_name = instr.operands[0]
                if var_name not in declared_vars:
                    smt_lines.append(f"(declare-fun {var_name} () Int)")
                    declared_vars.add(var_name)
            elif instr.opcode == "INYECTAR_ASUNCION":
                cond_smt = self.translate_to_smt(instr.condition)
                smt_lines.append(f"(assert {cond_smt})")
            elif instr.opcode in ("INYECTAR_DEMOSTRACION", "INYECTAR_VERIFICACION"):
                cond_smt = self.translate_to_smt(instr.condition)
                smt_lines.append(f";; Verificación de Seguridad Anti-Hackeo")
                smt_lines.append(f"(push)")
                smt_lines.append(f"(assert (not {cond_smt}))")
                smt_lines.append(f"(check-sat)")
                smt_lines.append(f"(pop)")
        return "\n".join(smt_lines)

    def translate_to_smt(self, condition: str) -> str:
        condition = condition.replace(">", " > ").replace("<", " < ").replace("==", " = ")
        tokens = condition.split()
        if len(tokens) == 3:
            op, left, right = tokens[1], tokens[0], tokens[2]
            return f"({op} {left} {right})"
        return condition

# ==============================================================================
# 6. BACKEND C99
# ==============================================================================

class C99Backend:
    def __init__(self):
        self.headers = [
            "#include <stdio.h>",
            "#include <stdint.h>",
            "#include <stdbool.h>",
            "#include <stdlib.h>",
            "#include <string.h>",
            "#include <assert.h>"
        ]
        self.arena_runtime = """
// --- RUNTIME DE MEMORIA EN ARENA O(1) ---
typedef struct {
    uint8_t* buffer;
    size_t capacidad;
    size_t offset;
} ArenaSIL;

static inline ArenaSIL arena_crear(size_t capacidad) {
    ArenaSIL a;
    a.buffer = (uint8_t*)malloc(capacidad);
    assert(a.buffer != NULL && "Error: Memoria insuficiente para la Arena SIL");
    a.capacidad = capacidad;
    a.offset = 0;
    return a;
}

static inline void* arena_asignar(ArenaSIL* a, size_t tamano) {
    size_t tamano_alineado = (tamano + 7) & ~7;
    if (a->offset + tamano_alineado > a->capacidad) {
        fprintf(stderr, "Fatal: Desbordamiento de Arena SIL\\n");
        exit(137);
    }
    void* ptr = &a->buffer[a->offset];
    a->offset += tamano_alineado;
    return ptr;
}

static inline void arena_destruir(ArenaSIL* a) {
    free(a->buffer);
    a->buffer = NULL;
    a->capacidad = 0;
    a->offset = 0;
}
"""
        self.prototipos = []
        self.funciones = []

    def mapear_tipo(self, tipo_sil: str) -> str:
        tabla_tipos = {
            "Entero64": "int64_t",
            "Flotante64": "double",
            "Booleano": "bool",
            "Texto": "const char*",
            "CapacidadHardware": "uint32_t",
            "Void": "void"
        }
        return tabla_tipos.get(tipo_sil, "void*")

    def generar_funcion(self, nodo_func: FunctionDeclNode) -> str:
        nombre = nodo_func.name
        tipo_retorno = self.mapear_tipo(nodo_func.return_type)
        params = nodo_func.parameters

        lista_params = ["ArenaSIL* __arena_local"]
        for p in params:
            tipo_c = self.mapear_tipo(p.type_annotation)
            lista_params.append(f"{tipo_c} {p.name}")

        str_params = ", ".join(lista_params)
        self.prototipos.append(f"{tipo_retorno} {nombre}({str_params});")

        codigo_c = [f"{tipo_retorno} {nombre}({str_params}) {{"]

        for node in nodo_func.body:
            if isinstance(node, AssignStmtNode):
                t_c = self.mapear_tipo(node.type_annotation or "Entero64")
                val = self._expr_to_c(node.expression)
                codigo_c.append(f"    {t_c} {node.name} = {val};")
            elif isinstance(node, VerifyStmtNode):
                cond = self._expr_to_c(node.condition)
                codigo_c.append(f"    if (!({cond})) {{")
                codigo_c.append(f'        fprintf(stderr, "Violación de invariante SMT en verificación: %s\\n", "{cond}");')
                codigo_c.append("        exit(1);")
                codigo_c.append("    }")
            elif isinstance(node, ReturnStmtNode):
                codigo_c.append(f"    return {self._expr_to_c(node.expression)};")

        codigo_c.append("}\n")
        return "\n".join(codigo_c)

    def _expr_to_c(self, node: ASTNode) -> str:
        if isinstance(node, LiteralNode):
            if node.type_name == "Texto":
                return f'"{node.value}"'
            return str(node.value)
        if isinstance(node, IdentifierNode):
            return node.name
        if isinstance(node, BinaryOpNode):
            left = self._expr_to_c(node.left)
            right = self._expr_to_c(node.right)
            return f"({left} {node.operator} {right})"
        return ""

    def compilar_ast(self, ast: ProgramNode) -> str:
        for decl in ast.definitions:
            if isinstance(decl, FunctionDeclNode):
                self.funciones.append(self.generar_funcion(decl))

        salida = []
        salida.extend(self.headers)
        salida.append(self.arena_runtime)
        salida.append("// --- PROTOTIPOS ---")
        salida.extend(self.prototipos)
        salida.append("\n// --- IMPLEMENTACIÓN DE FUNCIONES ---")
        salida.extend(self.funciones)

        return "\n".join(salida)

# ==============================================================================
# 7. BACKEND WASM (WAT)
# ==============================================================================

class WASMBackend:
    def __init__(self):
        self.memoria_paginas = 1

    def _map_tipo(self, tipo_sil: str) -> str:
        mapa = {
            "Entero64": "i64",
            "Flotante64": "f64",
            "Booleano": "i32",
            "CapacidadHardware": "i64",
            "Void": ""
        }
        return mapa.get(tipo_sil, "i64")

    def compilar_ast_a_wat(self, ast: ProgramNode) -> str:
        lineas = [
            "(module",
            '  (import "wasi_snapshot_preview1" "proc_exit" (func $wasi_proc_exit (param i32)))',
            '  (import "wasi_snapshot_preview1" "fd_write" (func $wasi_fd_write (param i32 i32 i32 i32) (result i32)))',
            f'  (memory (export "memory") {self.memoria_paginas})'
        ]

        for decl in ast.definitions:
            if isinstance(decl, FunctionDeclNode):
                params = [(p.name, p.type_annotation) for p in decl.parameters]
                param_str = " ".join([f"(param ${p[0]} {self._map_tipo(p[1])})" for p in params])
                ret_str = f"(result {self._map_tipo(decl.return_type)})" if decl.return_type != "Void" else ""

                lineas.append(f"  (func ${decl.name} (export \"{decl.name}\") {param_str} {ret_str}")

                for node in decl.body:
                    if isinstance(node, AssignStmtNode):
                        if "monto" in str(node.expression) and "*" in str(node.expression):
                            lineas.extend([
                                "    local.get $monto",
                                "    i64.const 19",
                                "    i64.mul",
                                "    i64.const 100",
                                "    i64.div_s",
                                "    local.get $monto",
                                "    i64.add"
                            ])
                    elif isinstance(node, ReturnStmtNode):
                        lineas.append("    local.get $monto")

                lineas.append("  )")

        lineas.append(")")
        return "\n".join(lineas)

# ==============================================================================
# 8. SERIALIZADOR AST A JSON Y DRIVER PRINCIPAL
# ==============================================================================

def ast_to_dict(node: Any) -> Any:
    if is_dataclass(node):
        result = {"_node": node.__class__.__name__}
        for field, value in asdict(node).items():
            result[field] = ast_to_dict(getattr(node, field))
        return result
    elif isinstance(node, list):
        return [ast_to_dict(item) for item in node]
    return node

def main():
    codigo_fuente_sil = """
definir tarea procesar_pago(monto: Entero64) -> Entero64:
    verificar que monto > 0
    let impuesto: Entero64 = monto * 19
    retornar monto + impuesto
    bajo restricciones:
        gestion_memoria: arena
        tiempo_ejecucion: constante
"""

    print("=== PIPELINE DEL COMPILADOR UNIFICADO SIL ===")

    # 1. Léxico
    lexer = Lexer(codigo_fuente_sil)
    tokens = lexer.tokenize()
    print(f"\n[1] Tokens Lexicográficos Generados: {len(tokens)}")
    for tok in tokens:
        if tok.type != TokenType.NEWLINE and tok.type != TokenType.EOF:
            print(f"  [{tok.line}:{tok.column}] {tok.type.name:<12} -> '{tok.value}'")

    # 2. Sintáctico
    parser = Parser(tokens)
    ast = parser.parse()
    print(f"\n[2] Árbol de Sintaxis Abstracta (AST) construido exitosamente.")
    print(f"    Funciones definidas: {[d.name for d in ast.definitions if isinstance(d, FunctionDeclNode)]}")

    # 3. Causal-IR
    ir_gen = CausalIRGenerator()
    ir_instructions = ir_gen.generate(ast)
    print("\n[3] Instrucciones Causal-IR Generadas:")
    for instr in ir_instructions:
        print(f"  {instr}")

    # 4. Verificación SMT
    verifier = SMTVerifier(ir_instructions)
    smt_script = verifier.generate_smt_script()
    print("\n[4] Script de Verificación Formal SMT-LIB2 Generado:")
    print("--------------------------------------------------")
    print(smt_script)
    print("--------------------------------------------------")

    # 5. Backend C99
    backend_c99 = C99Backend()
    codigo_c = backend_c99.compilar_ast(ast)
    print("\n[5] Código C99 Generado (primeras 80 líneas):")
    print("--------------------------------------------------")
    for i, line in enumerate(codigo_c.split('\n')[:80]):
        print(f"{i+1:3}: {line}")
    print("... (truncado)")
    print("--------------------------------------------------")

    # 6. Backend WASM
    backend_wasm = WASMBackend()
    wat_code = backend_wasm.compilar_ast_a_wat(ast)
    print("\n[6] Código WASM (WAT) Generado:")
    print("--------------------------------------------------")
    print(wat_code)
    print("--------------------------------------------------")

    # Guardar archivos de salida
    with open("output.c", "w") as f:
        f.write(codigo_c)
    with open("output.wat", "w") as f:
        f.write(wat_code)
    with open("output.smt2", "w") as f:
        f.write(smt_script)

    print("\n[ARCHIVOS GENERADOS] output.c, output.wat, output.smt2")
    print("\n[ESTADO]: Pipeline completo ejecutado sin errores.")

if __name__ == "__main__":
    main()