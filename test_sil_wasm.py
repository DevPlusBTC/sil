#!/usr/bin/env python3
"""
test_sil_wasm.py - Pruebas unitarias para el Backend WebAssembly (WASM/WASI) de SIL.
"""

import unittest
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sil_wasm import WASMBackend


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
        ast = [{
            "tipo": "tarea",
            "nombre": "procesar_pago",
            "params": [("monto", "Entero64")],
            "retorno": "Entero64",
            "body": []
        }]
        wat = self.backend.compilar_ast_a_wat(ast)
        self.assertTrue(wat.startswith("(module"))
        self.assertIn("import \"wasi_snapshot_preview1\"", wat)
        self.assertIn("(export \"procesar_pago\")", wat)
        self.assertIn("(memory", wat)

    def test_encabezado_wasi_imports(self):
        ast = [{"tipo": "tarea", "nombre": "test", "params": [], "retorno": "Void", "body": []}]
        wat = self.backend.compilar_ast_a_wat(ast)
        self.assertIn("proc_exit", wat)
        self.assertIn("fd_write", wat)

    def test_funcion_con_parametros_multiples(self):
        ast = [{
            "tipo": "tarea",
            "nombre": "sumar",
            "params": [("a", "Entero64"), ("b", "Flotante64")],
            "retorno": "Entero64",
            "body": []
        }]
        wat = self.backend.compilar_ast_a_wat(ast)
        self.assertIn("(param $a i64)", wat)
        self.assertIn("(param $b f64)", wat)


if __name__ == "__main__":
    unittest.main(verbosity=2)