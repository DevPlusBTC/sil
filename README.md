# SIL - Source Implementation Language

**Versión:** 0.1.0 | **Estado:** Beta Funcional — **Frontend + IR + Intervalos + C99/WASM implementados; Z3/TPM/Fibras = STUBS**

Lenguaje de sistemas de ultra alto rendimiento con verificación formal en tiempo de compilación. Elimina corrupción de memoria, desbordamientos de búfer, condiciones de carrera y comportamientos no definidos directamente durante la fase de compilación.

## Qué FUNCIONA hoy (producción-ready para casos de uso limitados)

- ✅ **Frontend CNL completo**: Lexer + Parser determinista sin backtracking, keywords en español
- ✅ **Causal-IR completo**: SSA con invariantes SMT adjuntas, lowering AST → IR
- ✅ **Verificador SMT (Intervalos)**: Backend puro Rust, sound pero incompleto, cache persistente SHA3-512
- ✅ **Arenas O(1)**: Bump alloc, sub-arenas cíclicas O(1), promoción zero-copy, migración lazy (AST/IR)
- ✅ **Backend C99**: Emite C99 estricto, compila con `gcc -std=c99 -Wall -Wextra -Werror`, runtime Arenas embebido
- ✅ **Backend WASM**: Emite binario WASM válido con imports WASI (`proc_exit`, `fd_write`)
- ✅ **Contratos SMT**: `asumir`/`verificar`/`demostrar` con contraejemplos concretos
- ✅ **CLI completo**: `silc check|build|emit-smt|run`
- ✅ **150 tests pasando**

## Qué NO funciona (STUBS / Planificado)

| Componente | Estado | Detalle |
|------------|--------|---------|
| **Verificador Z3** | ⚠️ STUB | API rota (feature `smt` no funcional) |
| **TPM 2.0 / Hardware Root of Trust** | ⚠️ STUB | Framework de tipos solo; `tpm2_*` retornan ceros/OK falso |
| **Scheduler Fibras / Canales Lock-Free** | ⚠️ STUB | `Scheduler::spawn`/`ejecutar` retornan error |
| **io_uring / E/S Asíncrona Nativa** | ❌ NO IMPLEMENTADO | Solo declaración |
| **Backend LLVM / SPIR-V / GPU** | ❌ NO IMPLEMENTADO | Solo stubs |
| **Arenas Persistentes NVM / Migración Lazy** | ⚠️ STUB | Solo AST/IR |
| **eBPF Kernel / TPM 2.0 Real** | ❌ / ⚠️ STUB | Solo framework de tipos |
| **Certificaciones (EAL7/DO-178C/ASIL-D)** | 🎯 OBJETIVO | Objetivos de diseño, no logrados |

## Instalación

```bash
# Desde fuente (requerido - no publicado en crates.io)
git clone https://github.com/DevPlusBTC/silc.git
cd silc/silc-core
cargo build --release
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
├── silc-causal-ir/    # Causal-IR + SMT (Intervalos)  ✅
├── silc-backend/      # Codegen C99, WASM  ✅  (LLVM: stub)
├── silc-runtime/      # Arenas O(1), Capacidades (validación ✅, firma TPM: STUB)
├── silc-cli/          # CLI  ✅
├── silc-contracts/    # Contratos compartidos  ✅
└── silc-test/         # Tests integración  ✅

silc-vscode/           # Extensión VS Code  ⚠️ STUB
docs/                  # Documentación (WHITE_PAPER actualizado)
```

## Ecosistema (crates internos)

| Crate | Versión | Estado |
|-------|---------|--------|
| `silc-frontend` | 0.1.0 | ✅ Lexer, Parser CNL, AST |
| `silc-causal-ir` | 0.1.0 | ✅ Causal-IR, SMT (Intervalos) |
| `silc-backend` | 0.1.0 | ✅ Codegen C99, WASM |
| `silc-runtime` | 0.1.0 | ✅ Arenas, Capacidades (firma: STUB) |
| `silc-cli` | 0.1.0 | ✅ CLI |
| `silc-contracts` | 0.1.0 | ✅ Contratos compartidos |
| `silc-test` | 0.1.0 | ✅ Tests integración |

## Licencia

MIT

## Contribución

Ver [CONTRIBUTING.md](CONTRIBUTING.md) — **no existe aún** 📋