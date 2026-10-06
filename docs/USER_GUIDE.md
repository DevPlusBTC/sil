# SIL User Guide v0.1.0

**Instalación → Primer programa → Contratos → Build C99/WASM → LSP + VS Code**

---

## 1. Instalación Rápida

### Opción A: Desde crates.io (recomendado)
```bash
cargo install silc-cli
```
Verifica:
```bash
silc --help
# silc 0.1.0
# USAGE: silc <COMMAND>
# Commands: check, build, emit-smt
```

### Opción B: Desde fuente (desarrollo)
```bash
git clone https://github.com/DevPlusBTC/sil
cd sil/silc-core
cargo build --release -p silc-cli
# Binary en: target/release/silc
```

---

## 2. Primer Programa: Hello SIL

Crear `hola.sil`:
```sil
definir tarea saludar(nombre: Texto) -> Texto:
    asumir nombre.len > 0
    demostrar nombre.len > 0
    retornar nombre
```

Verificar:
```bash
silc check hola.sil
# hola.sil: OK (1 tareas verificadas)
```

---

## 3. Contratos: `asumir` / `demostrar`

El poder de SIL: **verificación formal en compilación**.

### 3.1 Precondiciones (`asumir`)
```sil
definir tarea raiz(x: Flotante64) -> Flotante64:
    asumir x >= 0.0        // Precondición: dominio válido
    // ... cálculo ...
    demostrar resultado >= 0.0
    retornar resultado
```

### 3.2 Postcondiciones (`demostrar`)
```sil
definir tarea descuento(precio: Entero64, pct: Entero64) -> Entero64:
    asumir precio > 0
    asumir pct >= 0
    asumir pct <= 50       // Máximo 50%
    let final = precio * (100 - pct) / 100
    demostrar final <= precio      // Nunca sube
    demostrar final >= precio / 2  // Máx 50% off
    retornar final
```

### 3.3 Error de SMT (compilación falla)
```sil
definir tarea malo(x: Entero64) -> Entero64:
    asumir x > 0
    demostrar x > 100    // FALSO si x=5
    retornar x
```
```
silc check malo.sil
Error: SMT SAT - contraejemplo: x=5 rompe x > 100
```

---

## 4. Build: C99 y WASM

### 4.1 Target C99
```bash
silc build hola.sil --target c99 -o hola.c
# hola.sil → hola.c (2385 bytes C99)
```

**Contenido generado** (`hola.c`):
```c
#include <stdio.h>
#include <stdlib.h>
/* Runtime SIL embebido: arena O(1), tipos, capacidades */
typedef struct SilArena_ { ... } SilArena_;
long long saludar(SilArena_* __arena, long long nombre) { ... }
```

**Compilar y ejecutar:**
```bash
gcc -std=c99 -O2 hola.c -o hola
./hola
```

### 4.2 Target WASM
```bash
silc build hola.sil --target wasm -o hola.wasm
# hola.sil → hola.wasm (158 bytes, magic \0asm)
```

**Validar WASM:**
```bash
wasmparser validate hola.wasm  # o usar wasmparser CLI
```

---

## 5. Verificación SMT Standalone

```bash
silc emit-smt hola.sil
```

Salida SMT-LIB2 (QF_LIA):
```smt
(set-logic QF_LIA)
(declare-fun nombre () Int)
(assert (> nombre 0))
(push)
(assert (not (> nombre 0)))
(check-sat)
(pop)
```
Útil para: depurar contratos, integrar con Z3/CVC5 directamente, auditoría formal.

---

## 6. Ejemplos Completos

### 6.1 Calculadora Segura
```sil
definir tarea sumar(a: Entero64, b: Entero64) -> Entero64:
    asumir a > -1000000
    asumir b > -1000000
    asumir a < 1000000
    asumir b < 1000000
    demostrar a + b > -2000000
    demostrar a + b < 2000000
    retornar a + b

definir tarea dividir(dividendo: Entero64, divisor: Entero64) -> Entero64:
    asumir divisor != 0
    demostrar dividendo / divisor * divisor == dividendo
    retornar dividendo / divisor
```

### 6.2 Validación de Entrada
```sil
definir tarea validar_edad(edad: Entero64) -> Booleano:
    asumir edad >= 0
    asumir edad <= 150
    demostrar edad >= 18 == (edad >= 18)
    retornar edad >= 18
```

### 6.3 Factorial Acotado
```sil
definir tarea factorial(n: Entero64) -> Entero64:
    asumir n >= 0
    asumir n <= 20        // Límite overflow 64-bit
    si n == 0:
        retornar 1
    sino:
        demostrar n > 0
        demostrar n <= 20
        retornar n * factorial(n - 1)
```

---

## 7. LSP + VS Code

### 7.1 Instalar Extensión
```bash
# Desde .vsix generado
code --install-extension silc-vscode-0.1.0.vsix
```

### 7.2 Features Activas
| Feature | Trigger | Qué ves |
|---------|---------|---------|
| Diagnostics | Auto (escribir) | Subrayado rojo/amarillo + mensaje humano |
| Completion | Escribir `.` | Variables, tareas, tipos en scope |
| Hover | Mouse sobre identificador | Tipo inferido + contratos |
| Toggle Diagnostics | Ctrl+Shift+P → "SIL: Toggle Diagnostics" | On/Off |

### 7.3 Diagnósticos en Lenguaje Natural
```
[ERROR] Contradicción de Restricción SMT
Ubicación: tarea dividir (Línea 12)
Análisis: divisor puede ser 0 → división por cero
Solución: Agregar `asumir divisor != 0` antes de dividir
```

---

## 8. Flujo de Trabajo Recomendado

```mermaid
graph LR
    A[Editar .sil en VS Code] --> B[silc check - verificación instantánea]
    B --> C{¿OK?}
    C -->|No| A
    C -->|Sí| D[silc build --target c99|wasm]
    D --> E[Compilar C / Ejecutar WASM]
    E --> F[Test / Deploy]
```

### 8.1 Iteración Rápida
```bash
# Terminal 1: Watch mode (si configuras)
watchexec -e sil "silc check *.sil"

# Terminal 2: VS Code con LSP activo
code .
```

---

## 9. Solución de Problemas

| Error | Causa | Solución |
|-------|-------|----------|
| `TokenInesperado` | Sintaxis CNL inválida | Revisar indentación (4 espacios), palabras clave |
| `SMT SAT` | Contrato imposible | Revisar `asumir`/`demostrar`; simplificar |
| `SMT timeout` | Postcondición compleja | Dividir en tareas menores; simplificar `demostrar` |
| `build: una sola tarea` | Múltiples `definir tarea` | 1 archivo = 1 tarea para `build` |
| `silc not found` | PATH | `cargo install silc-cli` o `export PATH+=target/release` |

---

## 10. Próximos Pasos

- **Stdlib:** Importar `silc-std` (próximamente v0.2.0)
- **Testing:** `silc test` (integración con `silc-test` crate)
- **Package Manager:** `silpm init / add / build` (v0.3.0)
- **Formateador:** `silfmt archivo.sil` (v0.2.0)

---

## 11. Recursos

- **Espec completa:** [SPEC.md](SPEC.md)
- **Whitepaper (aspiracional):** [WHITE_PAPER.md](WHITE_PAPER.md)
- **Issues/Contrib:** https://github.com/DevPlusBTC/sil
- **Crates.io:** `silc-frontend`, `silc-causal-ir`, `silc-backend`, `silc-runtime`

---

*Sil v0.1.0 - Lenguaje de Intención Semántica - Beta Funcional*