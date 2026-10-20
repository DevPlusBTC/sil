# SIL - Source Implementation Language

**Versión:** 0.1.0-ALPHA | **Estado:** Fase 0 (Semilla) — **Frontend completo (tareas, servicios, modelos, memoria persistente, capacidades, módulos, migraciones) + IR + Intervalos + C99/WASM + Capacidades HMAC + Fibras + Canales + TPM 2.0 + E/S Async ✅ | Z3 (feature `smt`) = ✅ Listo para probar | LLVM = implementado sin validar en CI | eBPF = runtime Linux ✅ | NVM/migración = AST/parser ⚠️**

Lenguaje de sistemas de ultra alto rendimiento con verificación formal en tiempo de compilación. Elimina corrupción de memoria, desbordamientos de búfer, condiciones de carrera y comportamientos no definidos directamente durante la fase de compilación.

## 📊 Estado Real del Proyecto

| Métrica | Valor |
|---------|-------|
| **Tests** | **139/139 ✅** |
| **Crates** | 7/7 compilan |
| **Líneas código** | ~15,000 Rust |
| **Estado real** | **Fase 0 (Semilla)** |
| **Próximo hito** | LLVM + eBPF |

---

| **Tests** | **139/139 ✅** |
| **Crates** | 7/7 compilan |
| **Líneas código** | ~15,000 Rust |
| **Estado real** | **Fase 0 (Semilla)** |
| **Próximo hito** | LLVM + eBPF |

---

## Instalación

```bash
# Desde fuente (requerido - no publicado en crates.io)
git clone https://github.com/DevPlusBTC/silc.git
cd silc/silc-core
cargo build --release --features smt
```

## Uso Rápido

```bash
# Verificar un archivo SIL
silc check mi_servicio.sil

# Compilar a C99
silc build mi_servicio.sil --target c99 -o mi_servicio.c

# Compilar a WASM
silc build mi_servicio.sil --target wasm -o mi_servicio.wasm

# Emitir script SMT-LIB2
silc emit-smt mi_servicio.sil
```

## Ejemplo SIL

```sil
definir tarea procesar_pago(monto: Entero64, saldo: Entero64) -> Booleano:
    asumir que monto > 0
    asumir que saldo >= monto
    let nuevo_saldo = saldo - monto
    demostrar que nuevo_saldo >= 0
    retornar verdadero
    bajo restricciones:
        latencia_maxima: 200us
        gestion_memoria: arena
```

## Estructura del Proyecto

```
silc-core/
├── silc-frontend/     # Lexer + Parser CNL → AST  ✅
├── silc-causal-ir/    # Causal-IR + SMT (Intervalos + Z3 con feature `smt`)  ✅
├── silc-backend/      # Codegen C99, WASM  ✅  (LLVM: no implementado)
├── silc-runtime/      # Arenas O(1), Capacidades HMAC + TPM 2.0 + Scheduler Fibras + Canales  ✅
├── silc-cli/          # CLI  ✅
├── silc-contracts/    # Contratos compartidos  ✅
└── silc-test/         # Tests integración  ✅

silc-lsp/              # Servidor LSP (JSON-RPC)  ✅
silc-vscode/           # Extensión VS Code  ⚠️ STUB
docs/                  # Documentación (WHITE_PAPER actualizado)
```

## Ecosistema (crates internos)

| Crate | Versión | Estado |
|-------|---------|--------|
| `silc-frontend` | 0.1.0 | ✅ Lexer, Parser CNL, AST |
| `silc-causal-ir` | 0.1.0 | ✅ Causal-IR, SMT (Intervalos + Z3 con feature `smt`) |
| `silc-backend` | 0.1.0 | ✅ Codegen C99, WASM |
| `silc-runtime` | 0.1.0 | ✅ Arenas O(1), Capacidades HMAC, TPM 2.0, Scheduler Fibras, Canales |
| `silc-cli` | 0.1.0 | ✅ CLI |
| `silc-contracts` | 0.1.0 | ✅ Contratos compartidos |
| `silc-test` | 0.1.0 | ✅ Tests integración |

## Licencia

MIT

## Contribución

Ver [CONTRIBUTING.md](CONTRIBUTING.md) — **disponible** 📋