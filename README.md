# SIL - Source Implementation Language

**Versión:** 0.1.0-ALPHA | **Estado:** Fase 0 (Semilla) — **Parser completo (tareas, servicios, modelos, memoria persistente, capacidades, módulos, migraciones) + IR de tareas + Intervalos + C99/WASM + Capacidades HMAC + Fibras (Win32 y ucontext) + Canales + E/S Async ✅ | Z3 (feature `smt`, activa por defecto en el CLI) ✅ | CI: última ejecución fallando (ver Actions) | LLVM = implementado, job de CI **llvm-test** (ubuntu‑latest, LLVM 17) | eBPF e io_uring = opt-in (`SIL_EBPF=1`, `SIL_IO_URING=1`) | TPM 2.0 = sólo Windows | sub-arenas cíclicas ✅ (16 niveles, reclamo O(1)); NVM y migración = declaradas en AST, sin runtime**

Lenguaje de sistemas de ultra alto rendimiento con verificación formal en tiempo de compilación. Elimina corrupción de memoria, desbordamientos de búfer, condiciones de carrera y comportamientos no definidos directamente durante la fase de compilación.

## 📊 Estado Real del Proyecto

| Métrica | Valor |
|---------|-------|
| **Tests Rust** | **169/169 ✅** |
| **Tests Python** | **31/31 ✅** |
| **Tests C (runtime)** | **40 casos / 222 comprobaciones ✅** |
| **Crates** | 7/7 compilan |
| **Líneas código** | ~15,000 Rust |
| **Estado real** | **Fase 0 (Semilla)** |
| **Próximo hito** | Codegen de servicios/red; decidir LLVM (job de CI o experimental); validar eBPF con libbpf 1.x |

---

## Verificación del runtime (sin toolchain de Rust)

El runtime nativo (`sil-rt`, C99) y el runtime que **emite** el backend C99 se
compilan y ejecutan en local y en CI:

```bash
scripts/run_c_tests.sh            # estricto + ASan/UBSan + TSan + runtime emitido
scripts/run_c_tests.sh --bench    # medidas reproducibles (arena y canales)
```

Cubre arenas (alineación, desbordamiento, ciclos, promoción), canales SPSC/MPSC
(contrapresión, concurrencia real), fibras (round-robin, `yield`), SHA-256/HMAC
contra vectores FIPS 180-4 y RFC 4231, capacidades Zero-Trust y el runtime
embebido que `silc build --target c99` inyecta en cada programa.
Estado, hallazgos y hoja de ruta: [`docs/AUDITORIA.md`](docs/AUDITORIA.md).

## Instalación

```bash
# Desde fuente (el CLI no está publicado en crates.io)
git clone https://github.com/DevPlusBTC/sil.git
cd sil/silc-core
cargo install --path silc-cli
# o: cargo build --release -p silc-cli
```

Las librerías `silc-frontend`, `silc-causal-ir`, `silc-backend` y `silc-runtime`
sí están publicadas en crates.io.

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
        latencia_maxima: baja
        gestion_memoria: arena
```

## Estructura del Proyecto

```
silc-core/
├── silc-frontend/     # Lexer + Parser CNL → AST  ✅
├── silc-causal-ir/    # Causal-IR + SMT (Intervalos; Z3 por defecto)  ✅
├── silc-backend/      # Codegen C99, WASM; LLVM con feature `llvm` (sin validar en CI)  ✅
├── silc-runtime/      # Arenas O(1) + ciclos, Capacidades HMAC + TPM 2.0 + Scheduler Fibras + Canales  ✅
├── silc-cli/          # CLI + servidor LSP (`silc lsp`, JSON-RPC sobre stdio)  ✅
├── silc-contracts/    # Contratos compartidos  ✅
└── silc-test/         # Tests integración  ✅

silc-vscode/           # Extensión VS Code (cliente LSP; compila con `npm run compile`)  ✅
docs/                  # Documentación técnica
```

## Ecosistema (crates internos)

| Crate | Versión | Estado |
|-------|---------|--------|
| `silc-frontend` | 0.1.0 | ✅ Lexer, Parser CNL, AST |
| `silc-causal-ir` | 0.1.0 | ✅ Causal-IR, SMT (Intervalos; Z3 por defecto) |
| `silc-backend` | 0.1.0 | ✅ Codegen C99, WASM |
| `silc-runtime` | 0.1.0 | ✅ Arenas O(1), Capacidades HMAC, TPM 2.0, Scheduler Fibras, Canales |
| `silc-cli` | 0.1.0 | ✅ CLI |
| `silc-contracts` | 0.1.0 | ✅ Contratos compartidos |
| `silc-test` | 0.1.0 | ✅ Tests integración |

## Licencia

MIT

## Contribución

Ver [CONTRIBUTING.md](CONTRIBUTING.md) — **disponible** 📋