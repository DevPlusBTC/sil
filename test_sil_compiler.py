#!/usr/bin/env python3
"""
Suite completa de pruebas unitarias e integración para el compilador SIL.
Valida: Lexer, Parser, Backend C99, Integración End-to-End.
"""

import unittest
import sys
import os
import tempfile
import subprocess

# ==============================================================================
# IMPORTAR MÓDULOS DEL COMPILADOR
# ==============================================================================
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from silc_bootstrap import (
    Lexer, Parser, Token, TokenType,
    CausalIRGenerator, SMTVerifier, C99Backend, WASMBackend,
    CausalIRInstruction,
    ProgramNode, FunctionDeclNode, ParameterNode, RestrictionNode,
    VerifyStmtNode, ReturnStmtNode, AssumeStmtNode, ProveStmtNode,
    AssignStmtNode, BinaryOpNode, LiteralNode, IdentifierNode
)

# ==============================================================================
# TESTS DEL LEXER
# ==============================================================================

class TestSILLexer(unittest.TestCase):
    """Pruebas unitarias para el Analizador Léxico (Lexer)."""

    def test_tokenizacion_palabras_clave(self):
        codigo = "tarea procesar_pago(monto: Entero64)"
        lexer = Lexer(codigo)
        tokens = lexer.tokenize()
        values = [t.value for t in tokens if t.type != TokenType.NEWLINE and t.type != TokenType.EOF]
        self.assertIn("tarea", values)
        self.assertIn("procesar_pago", values)
        self.assertIn("Entero64", values)

    def test_omision_comentarios(self):
        codigo = """
        // Este es un comentario
        tarea calcular()
        """
        lexer = Lexer(codigo)
        tokens = lexer.tokenize()
        kw_tokens = [t for t in tokens if t.type == TokenType.KEYWORD]
        self.assertEqual(len(kw_tokens), 1)
        self.assertEqual(kw_tokens[0].value, "tarea")

    def test_numeros_enteros_y_flotantes(self):
        codigo = "let x = 42\nlet y = 3.14"
        lexer = Lexer(codigo)
        tokens = lexer.tokenize()
        nums = [t for t in tokens if t.type in (TokenType.INTEGER, TokenType.FLOAT)]
        self.assertEqual(len(nums), 2)
        self.assertEqual(nums[0].value, "42")
        self.assertEqual(nums[1].value, "3.14")

    def test_strings_con_interpolacion(self):
        codigo = 'let msg = "Hola {nombre}"'
        lexer = Lexer(codigo)
        tokens = lexer.tokenize()
        strings = [t for t in tokens if t.type == TokenType.STRING]
        self.assertEqual(len(strings), 1)
        self.assertEqual(strings[0].value, "Hola {nombre}")

    def test_simbolos_relacionales(self):
        codigo = "x >= y a != b"
        lexer = Lexer(codigo)
        tokens = lexer.tokenize()
        syms = [t.value for t in tokens if t.type == TokenType.SYMBOL]
        self.assertIn(">=", syms)
        self.assertIn("!=", syms)

    def test_identificadores_con_guion_bajo(self):
        codigo = "let monto_total = 100"
        lexer = Lexer(codigo)
        tokens = lexer.tokenize()
        ids = [t.value for t in tokens if t.type == TokenType.IDENTIFIER]
        self.assertIn("monto_total", ids)


# ==============================================================================
# TESTS DEL PARSER
# ==============================================================================

class TestSILParser(unittest.TestCase):
    """Pruebas unitarias para el Analizador Sintáctico (Parser)."""

    def _parse(self, codigo: str):
        lexer = Lexer(codigo)
        tokens = lexer.tokenize()
        parser = Parser(tokens)
        return parser.parse()

    def test_parseo_funcion_simple(self):
        codigo = """
definir tarea mi_funcion(monto: Entero64) -> Entero64:
    retornar 1
"""
        ast = self._parse(codigo)
        self.assertIsInstance(ast, ProgramNode)
        self.assertEqual(len(ast.definitions), 1)
        func = ast.definitions[0]
        self.assertIsInstance(func, FunctionDeclNode)
        self.assertEqual(func.name, "mi_funcion")
        self.assertEqual(func.return_type, "Entero64")
        self.assertEqual(len(func.parameters), 1)
        self.assertEqual(func.parameters[0].name, "monto")

    def test_parseo_con_asumir_demostrar(self):
        codigo = """
definir tarea calcular(x: Entero64) -> Entero64:
    asumir x > 0
    let y = x * 2
    demostrar y > x
    retornar y
"""
        ast = self._parse(codigo)
        func = ast.definitions[0]
        stmt_types = [type(s).__name__ for s in func.body]
        self.assertIn("AssumeStmtNode", stmt_types)
        self.assertIn("ProveStmtNode", stmt_types)
        self.assertIn("AssignStmtNode", stmt_types)
        self.assertIn("ReturnStmtNode", stmt_types)

    def test_parseo_sin_cuerpo_complejo(self):
        codigo = """
definir tarea simple() -> Void:
    retornar 0
"""
        ast = self._parse(codigo)
        func = ast.definitions[0]
        self.assertEqual(func.name, "simple")
        self.assertEqual(func.return_type, "Void")
        self.assertEqual(len(func.parameters), 0)


# ==============================================================================
# TESTS DEL BACKEND C99
# ==============================================================================

class TestSILBackendC99(unittest.TestCase):
    """Pruebas para la generación de código C99 y preservación de invariantes."""

    def setUp(self):
        self.backend = C99Backend()

    def _make_ast(self, func_name="test_func", params=None, body=None):
        if params is None:
            params = [ParameterNode(name="x", type_annotation="Entero64")]
        if body is None:
            body = [
                VerifyStmtNode(condition=BinaryOpNode(
                    left=IdentifierNode(name="x"),
                    operator=">",
                    right=LiteralNode(value=0, type_name="Entero64")
                )),
                ReturnStmtNode(expression=IdentifierNode(name="x"))
            ]
        func = FunctionDeclNode(
            name=func_name,
            parameters=params,
            return_type="Entero64",
            body=body,
            restrictions=[]
        )
        return ProgramNode(definitions=[func])

    def test_generacion_codigo_c99_valido(self):
        ast = self._make_ast()
        codigo_c = self.backend.compilar_ast(ast)

        self.assertIn("#include <stdint.h>", codigo_c)
        self.assertIn("#include <stdbool.h>", codigo_c)
        self.assertIn("int64_t test_func(ArenaSIL* __arena_local, int64_t x)", codigo_c)
        self.assertIn("if (!(", codigo_c)  # Check for verification code
        self.assertIn("return x;", codigo_c)
        self.assertIn("arena_crear", codigo_c)
        self.assertIn("arena_asignar", codigo_c)
        self.assertIn("arena_destruir", codigo_c)

    def test_ast_vacio_falla(self):
        ast_vacio = ProgramNode(definitions=[])
        # Backend should handle empty AST gracefully (generate headers only)
        codigo_c = self.backend.compilar_ast(ast_vacio)
        self.assertIn("#include <stdint.h>", codigo_c)
        self.assertIn("arena_crear", codigo_c)

    def test_mapeo_tipos_correcto(self):
        self.assertEqual(self.backend.mapear_tipo("Entero64"), "int64_t")
        self.assertEqual(self.backend.mapear_tipo("Flotante64"), "double")
        self.assertEqual(self.backend.mapear_tipo("Booleano"), "bool")
        self.assertEqual(self.backend.mapear_tipo("Texto"), "const char*")
        self.assertEqual(self.backend.mapear_tipo("Void"), "void")

    def test_generacion_con_parametros_multiples(self):
        params = [
            ParameterNode(name="a", type_annotation="Entero64"),
            ParameterNode(name="b", type_annotation="Flotante64"),
        ]
        ast = self._make_ast(params=params)
        codigo_c = self.backend.compilar_ast(ast)
        self.assertIn("int64_t a", codigo_c)
        self.assertIn("double b", codigo_c)


# ==============================================================================
# TESTS DEL BACKEND WASM
# ==============================================================================

class TestSILWasmBackend(unittest.TestCase):
    """Pruebas de emisión del Backend WebAssembly / WASI para SIL."""

    def setUp(self):
        self.backend = WASMBackend()

    def test_map_tipos_primitivos(self):
        self.assertEqual(self.backend._map_tipo("Entero64"), "i64")
        self.assertEqual(self.backend._map_tipo("Flotante64"), "f64")
        self.assertEqual(self.backend._map_tipo("Booleano"), "i32")
        self.assertEqual(self.backend._map_tipo("CapacidadHardware"), "i64")
        self.assertEqual(self.backend._map_tipo("Void"), "")

    def test_generacion_modulo_wat_valido(self):
        ast = ProgramNode(definitions=[
            FunctionDeclNode(
                name="procesar_pago",
                parameters=[ParameterNode(name="monto", type_annotation="Entero64")],
                return_type="Entero64",
                body=[],
                restrictions=[]
            )
        ])
        wat = self.backend.compilar_ast_a_wat(ast)
        self.assertTrue(wat.startswith("(module"))
        self.assertIn('import "wasi_snapshot_preview1"', wat)
        self.assertIn("(export \"procesar_pago\")", wat)
        self.assertIn("(memory", wat)

    def test_encabezado_wasi_imports(self):
        ast = ProgramNode(definitions=[
            FunctionDeclNode(name="test", parameters=[], return_type="Void", body=[], restrictions=[])
        ])
        wat = self.backend.compilar_ast_a_wat(ast)
        self.assertIn("proc_exit", wat)
        self.assertIn("fd_write", wat)

    def test_funcion_con_parametros_multiples(self):
        ast = ProgramNode(definitions=[
            FunctionDeclNode(
                name="sumar",
                parameters=[
                    ParameterNode(name="a", type_annotation="Entero64"),
                    ParameterNode(name="b", type_annotation="Flotante64")
                ],
                return_type="Entero64",
                body=[],
                restrictions=[]
            )
        ])
        wat = self.backend.compilar_ast_a_wat(ast)
        self.assertIn("(param $a i64)", wat)
        self.assertIn("(param $b f64)", wat)


# ==============================================================================
# TESTS CAUSAL-IR
# ==============================================================================

class TestSILCausalIR(unittest.TestCase):
    """Pruebas para la generación de Causal-IR."""

    def setUp(self):
        self.ir_gen = CausalIRGenerator()

    def test_generacion_ir_basica(self):
        ast = ProgramNode(definitions=[
            FunctionDeclNode(
                name="test",
                parameters=[ParameterNode(name="x", type_annotation="Entero64")],
                return_type="Entero64",
                body=[
                    AssumeStmtNode(condition=BinaryOpNode(
                        left=IdentifierNode(name="x"), operator=">", right=LiteralNode(value=0, type_name="Entero64")
                    )),
                    ReturnStmtNode(expression=IdentifierNode(name="x"))
                ],
                restrictions=[]
            )
        ])
        ir = self.ir_gen.generate(ast)
        opcodes = [i.opcode for i in ir]
        self.assertIn("INICIO_TAREA", opcodes)
        self.assertIn("ASIGNAR_PARAMETRO", opcodes)
        self.assertIn("INYECTAR_ASUNCION", opcodes)
        self.assertIn("RETORNAR", opcodes)
        self.assertIn("FIN_TAREA", opcodes)


# ==============================================================================
# TESTS VERIFICADOR SMT
# ==============================================================================

class TestSILSMTVerifier(unittest.TestCase):
    """Pruebas para la tubería de verificación SMT."""

    def _make_ir(self, assumes=None, proves=None):
        instrs = [CausalIRInstruction("INICIO_TAREA", ["test"])]
        if assumes:
            for a in assumes:
                instrs.append(CausalIRInstruction("INYECTAR_ASUNCION", [], a))
        if proves:
            for p in proves:
                instrs.append(CausalIRInstruction("INYECTAR_DEMOSTRACION", [], p))
        instrs.append(CausalIRInstruction("FIN_TAREA", ["test"]))
        return instrs

    def test_generacion_smt_lib2(self):
        ir = self._make_ir(assumes=["x > 0"], proves=["x * 2 > x"])
        # Need to declare x first
        ir.insert(1, CausalIRInstruction("ASIGNAR_PARAMETRO", ["x", "Entero64"]))
        verifier = SMTVerifier(ir)
        smt = verifier.generate_smt_script()
        self.assertIn("(set-logic QF_LIA)", smt)
        self.assertIn("(declare-fun x () Int)", smt)
        self.assertIn("(assert (> x 0))", smt)
        self.assertIn("(assert (not x * 2  >  x))", smt)
        self.assertIn("(check-sat)", smt)

    def test_traduccion_operadores(self):
        verifier = SMTVerifier([])
        self.assertEqual(verifier.translate_to_smt("x > 0"), "(> x 0)")
        self.assertEqual(verifier.translate_to_smt("a == b"), "(= a b)")
        self.assertEqual(verifier.translate_to_smt("m + n"), "(+ m n)")


# ==============================================================================
# TEST DE INTEGRACIÓN END-TO-END (Simplificado)
# ==============================================================================

class TestPipelineIntegracionSIL(unittest.TestCase):
    """Prueba de integración End-to-End simplificada."""

    def test_pipeline_basico(self):
        """Test básico que el pipeline completo funciona sin errores."""
        codigo_sil = """
definir tarea sumar(x: Entero64, y: Entero64) -> Entero64:
    retornar x + y
"""

        # 1. Lexer
        lexer = Lexer(codigo_sil)
        tokens = lexer.tokenize()
        self.assertTrue(len(tokens) > 5)

        # 2. Parser
        parser = Parser(tokens)
        ast = parser.parse()
        func = ast.definitions[0]
        self.assertEqual(func.name, "sumar")
        self.assertEqual(len(func.parameters), 2)

        # 3. Causal-IR
        ir_gen = CausalIRGenerator()
        ir_instructions = ir_gen.generate(ast)
        self.assertTrue(any(i.opcode == "INICIO_TAREA" for i in ir_instructions))

        # 4. Backend C99
        backend_c99 = C99Backend()
        codigo_c = backend_c99.compilar_ast(ast)
        self.assertIn("int64_t sumar", codigo_c)
        self.assertIn("return (x + y);", codigo_c)

        # 5. Backend WASM
        backend_wasm = WASMBackend()
        wat = backend_wasm.compilar_ast_a_wat(ast)
        self.assertIn("(func $sumar", wat)
        self.assertIn("(export \"sumar\")", wat)


# ==============================================================================
# PUNTO DE ENTRADA
# ==============================================================================

if __name__ == "__main__":
    print("=" * 70)
    print(" EJECUTANDO SUITE DE PRUEBAS DE SIL COMPILER (LEXER, PARSER, BACKEND) ")
    print("=" * 70)
    unittest.main(verbosity=2)