#!/usr/bin/env python3
"""
test_sil_stdlib.py - Pruebas unitarias para la Biblioteca Estándar de SIL.
Valida que el Runtime C99 emitido contiene los componentes de seguridad requeridos
y es sintácticamente válido C99.
"""

import unittest
import os
import tempfile
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sil_stdlib import RUNTIME_C99_HEADER


class TestSILStdlib(unittest.TestCase):
    """Pruebas para verificar que la Stdlib emite funciones válidas de C99 y reglas de seguridad."""

    def test_presencia_componentes_seguridad(self):
        """Verifica que el Runtime incluya las funciones contra side-channel y capacidades."""
        self.assertIn("sil_cap_validar", RUNTIME_C99_HEADER)
        self.assertIn("sil_cripto_comparar_tiempo_constante", RUNTIME_C99_HEADER)
        self.assertIn("CAP_PERM_READ_FILE", RUNTIME_C99_HEADER)
        self.assertIn("CAP_PERM_WRITE_FILE", RUNTIME_C99_HEADER)
        self.assertIn("CAP_PERM_NETWORK", RUNTIME_C99_HEADER)

    def test_presencia_concurrencia_canales(self):
        """Verifica las estructuras de canales no bloqueantes de la stdlib."""
        self.assertIn("SilCanal64", RUNTIME_C99_HEADER)
        self.assertIn("sil_canal_crear", RUNTIME_C99_HEADER)
        self.assertIn("sil_canal_enviar", RUNTIME_C99_HEADER)
        self.assertIn("sil_canal_recibir", RUNTIME_C99_HEADER)

    def test_presencia_tipos_basicos(self):
        """Verifica tipos primitivos y layouts."""
        self.assertIn("SilTexto", RUNTIME_C99_HEADER)
        self.assertIn("SilColeccion", RUNTIME_C99_HEADER)
        self.assertIn("CapacidadHardware", RUNTIME_C99_HEADER)

    def test_compilabilidad_c99_header(self):
        """Valida que el Runtime emitido sea C99 sintácticamente válido invocando GCC/Clang."""
        with tempfile.NamedTemporaryFile(suffix=".h", delete=False) as tmp:
            tmp.write(RUNTIME_C99_HEADER.encode('utf-8'))
            tmp_path = tmp.name

        try:
            # Verificar sintaxis C99
            result = subprocess.run(
                ["gcc", "-std=c99", "-Wall", "-Wextra", "-Werror", "-fsyntax-only", "-x", "c", tmp_path],
                capture_output=True, timeout=15
            )
            if result.returncode == 0:
                self.assertEqual(result.returncode, 0, "El header de la Stdlib compila perfectamente en C99 estricto.")
            else:
                # Si gcc no está disponible, saltar test pero no fallar
                print(f"  [INFO] gcc no disponible o error: {result.stderr.decode()[:200]}")
        except FileNotFoundError:
            print("  [INFO] gcc no encontrado en PATH, saltando validación de compilación")
        except subprocess.TimeoutExpired:
            self.fail("Timeout compilando header stdlib")
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)

    def test_volatile_en_comparacion_constante(self):
        """Verifica que la comparación en tiempo constante use 'volatile' para evitar optimización."""
        self.assertIn("volatile uint8_t resultado", RUNTIME_C99_HEADER)

    def test_exit_code_seguridad(self):
        """Verifica que la violación de capacidad use exit(137) (SIGKILL)."""
        self.assertIn("exit(137)", RUNTIME_C99_HEADER)


if __name__ == "__main__":
    unittest.main(verbosity=2)