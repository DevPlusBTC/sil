# SIL User Guide

## Tabla de Contenidos

1. [Introducción](#introducción)
2. [Sintaxis Básica](#sintaxis-básica)
3. [Tipos de Datos](#tipos-de-datos)
4. [Contratos: assume y prove](#contratos-assume-y-prove)
5. [Gestión de Memoria](#gestión-de-memoria)
6. [Control de Flujo](#control-de-flujo)
7. [Ejemplos](#ejemplos)
8. [Errores Comunes](#errores-comunes)

---

## Introducción

SIL (Semantic Intention Language) es un lenguaje de programación de sistemas diseñado para eliminar categorías fundamentales de errores de software directamente durante la compilación.

### Filosofía

- **Sin comportamiento implícito:** Todo debe ser explícito
- **Verificación en compilación:** No en runtime
- **Cero sobrecoste:** La seguridad no cuesta tiempo de ejecución
- **Determinismo:** Gestión de recursos predecible

### Primer Programa

```lisp
(task incrementar
  (params (x i64))
  (returns i64)
  (body
    (assume (> x 0))
    (prove (> (+ x 1) 1))
    (return (+ x 1))))
```

Verificar:
```bash
silc check incrementar.sil
# incrementar.sil: OK (1 tareas verificadas)
```

---

## Sintaxis Básica

### Estructura de un Programa

Un programa SIL consiste en formas S-expression entre paréntesis. La cabeza de
cada lista dice de qué forma se trata; los hijos son sus argumentos:

```lisp
(task nombre
  (params (parametro Tipo))
  (returns TipoRetorno)
  (body
    (assume condicion)
    (prove condicion)))
```

### Delimitadores y Comentarios

SIL v0.1.0 usa **paréntesis** como único delimitador. No hay INDENT/DEDENT ni
llaves: el anidamiento de listas define la estructura.

Los comentarios empiezan con `;` y llegan hasta el final de la línea:

```lisp
; Esto es un comentario
(task sin_comentarios
  (params)
  (returns i64)
  (body (return 42)))   ; comentario al final de línea
```

---

## Tipos de Datos

### Tipos Primitivos

| Tipo | Descripción | Ejemplo |
|------|-------------|---------|
| `i64` | Entero de 64 bits | `42`, `-7` |
| `f64` | Punto flotante 64 bits | `3.14`, `-0.5` |
| `booleano` | Valor booleano | `verdadero`, `falso` |
| `texto` | Cadena UTF-8 | `"hola"` |
| `capacidad` | Token de capacidad | - |
| `void` | Sin valor | - |
| `USD` / `EUR` | Moneda | `100` |

### Tipos Compuestos

| Tipo | S-exp | Descripción |
|------|-------|-------------|
| Lista | `(list TIPO)` | Secuencia homogénea |
| Mapa | `(map CLAVE VALOR)` | Asociación clave→valor |
| Conjunto | `(set TIPO)` | Colección sin duplicados |
| Tupla | `(tuple T1 T2 ...)` | Producto heterogéneo |
| Nominal | `MiTipo` | Tipo definido por el usuario |

### Operadores Aritméticos

Prefijo uniforme: `(op a b)`.

| Operador | Descripción | Ejemplo |
|----------|-------------|---------|
| `+` | Suma | `(+ a b)` |
| `-` | Resta | `(- a b)`; `-x` → `(- 0 x)` |
| `*` | Multiplicación | `(* a b)` |
| `/` | División | `(/ a b)` |

### Operadores Relacionales

| Operador | Descripción | Ejemplo |
|----------|-------------|---------|
| `>` | Mayor que | `(> x 0)` |
| `<` | Menor que | `(< x 100)` |
| `>=` | Mayor o igual | `(>= x 0)` |
| `<=` | Menor o igual | `(<= x 100)` |
| `==` | Igual | `(== x 0)` |
| `!=` | Diferente | `(!= x 0)` |

---

## Contratos: assume y prove

El corazón de SIL son los **contratos**: precondiciones (`assume`) y postcondiciones (`prove`).

### `assume` (Precondición)

Declara qué debe ser verdadero antes de que la tarea se ejecute.

```lisp
(task dividir
  (params (a i64) (b i64))
  (returns i64)
  (body
    (assume (!= b 0))
    (return (/ a b))))
```

### `prove` (Postcondición)

Declara qué debe ser verdadero después de que la tarea se ejecute.

```lisp
(task absoluto
  (params (x i64))
  (returns i64)
  (body
    (prove (>= x 0))
    (if (< x 0)
      (body (return (- 0 x)))
      (body (return x)))))
```

### Múltiples Contratos

```lisp
(task transferir
  (params (saldo i64) (monto i64))
  (returns i64)
  (body
    (assume (> saldo 0))
    (assume (> monto 0))
    (assume (<= monto saldo))
    (prove (>= (- saldo monto) 0))
    (return (- saldo monto))))
```

---

## Gestión de Memoria

### Arenas O(1)

SIL gestiona la memoria con **Arenas** que asignan y liberan en tiempo constante.

```lisp
(task procesar
  (params (datos texto))
  (returns i64)
  (body
    (assume (> (get datos len) 0))
    (return (get datos len))))
```

### Sin Garbage Collector

SIL no tiene recolector de basura. La memoria se libera determinísticamente.

---

## Ejemplos

### Ejemplo 1: Función Simple

```lisp
(task cuadrado
  (params (x i64))
  (returns i64)
  (body
    (assume (>= x 0))
    (assume (<= x 46340))
    (prove (>= (* x x) 0))
    (return (* x x))))
```

### Ejemplo 2: Validación

```lisp
(task validar_edad
  (params (edad i64))
  (returns booleano)
  (body
    (assume (>= edad 0))
    (assume (<= edad 150))
    (return (>= edad 18))))
```

### Ejemplo 3: Múltiples Tareas

```lisp
(task sumar
  (params (a i64) (b i64))
  (returns i64)
  (body (return (+ a b))))

(task restar
  (params (a i64) (b i64))
  (returns i64)
  (body (return (- a b))))
```

### Ejemplo 4: Control de Flujo

El lenguaje soporta condicionales con `if`, `else` y anidamiento:

```lisp
(task clasificar
  (params (nota i64))
  (returns texto)
  (body
    (assume (>= nota 0))
    (assume (<= nota 100))
    (if (>= nota 90)
      (body (return "A"))
      (else (body
        (if (>= nota 80)
          (body (return "B"))
          (else (body
            (if (>= nota 70)
              (body (return "C"))
              (else (body (return "F"))))))))))))
```

**Notas de sintaxis:**
- Cada condición va en posición prefija: `(if COND CUERPO (else CUERPO)?)`.
- Un `CUERPO` es `(body SENTENCIA*)`; `restrictions` debe ser su último hijo si aparece.
- `else` anida otra forma `if` dentro de su cuerpo para expresar `sino si`.

---

## Control de Flujo

SIL admite condicionales con `if`/`else` (ver Ejemplo 4), bucles con
`while`/`for` y sus invariantes con `invariant`.

```lisp
(task contar
  (params (n i64))
  (returns i64)
  (body
    (while (> n 0)
      (body
        (invariant (>= n 0))
        (return n)))
    (return 0)))
```

---

## Pruebas

Ejecutar las pruebas del proyecto:

```bash
cargo test --workspace
```

---

## Licencia

MIT
