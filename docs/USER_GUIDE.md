# SIL User Guide

## Tabla de Contenidos

1. [Introducción](#introducción)
2. [Sintaxis Básica](#sintaxis-básica)
3. [Tipos de Datos](#tipos-de-datos)
4. [Contratos: asumir y demostrar](#contratos-asumir-y-demostrar)
5. [Gestión de Memoria](#gestión-de-memoria)
6. [Capacidades](#capacidades)
7. [Ejemplos](#ejemplos)
8. [Errores Comunes](#errores-comunes)

---

## Introducción

SIL (Source Implementation Language) es un lenguaje de programación de sistemas diseñado para eliminar categorías fundamentales de errores de software directamente durante la compilación.

### Filosofía

- **Sin comportamiento implícito:** Todo debe ser explícito
- **Verificación en compilación:** No en runtime
- **Cero sobrecoste:** La seguridad no cuesta tiempo de ejecución
- **Determinismo:** Gestión de recursos predecible

### Primer Programa

```sil
definir tarea saludar(nombre: Texto) -> Texto:
    asumir nombre.len > 0
    demostrar nombre.len > 0
    retornar nombre
```

Verificar:
```bash
silc check saludar.sil
# saludar.sil: OK (1 tareas verificadas)
```

---

## Sintaxis Básica

### Estructura de un Programa

Un programa SIL consiste en una o más definiciones de tareas:

```sil
definir tarea nombre(parametro: Tipo) -> TipoRetorno:
    asumir condicion
    demostrar condicion
    expresion
```

### Indentación

SIL usa **4 espacios** por nivel de indentación. No se usan tabs ni llaves.

```sil
definir tarea ejemplo(x: Entero64) -> Entero64:
    asumir x > 0
    demostrar x > 0
    retornar x + 1
```

### Comentarios

```sil
# Esto es un comentario de línea

# Los comentarios no afectan la verificación
definir tarea sin_comentarios() -> Entero64:
    retornar 42
```

---

## Tipos de Datos

### Tipos Primitivos

| Tipo | Descripción | Ejemplo |
|------|-------------|---------|
| `Entero64` | Entero de 64 bits | `42`, `-7` |
| `Flotante64` | Punto flotante 64 bits | `3.14`, `-0.5` |
| `Booleano` | Valor booleano | `verdadero`, `falso` |
| `Texto` | Cadena UTF-8 | `"hola"` |
| `Byte` | Byte (8 bits) | `0xFF` |
| `Void` | Sin valor | - |

### Literales

```sil
definir tarea literales() -> Entero64:
    retornar 42
```

### Operadores Aritméticos

| Operador | Descripción | Ejemplo |
|----------|-------------|---------|
| `+` | Suma | `a + b` |
| `-` | Resta | `a - b` |
| `*` | Multiplicación | `a * b` |
| `/` | División | `a / b` |

### Operadores Relacionales

| Operador | Descripción | Ejemplo |
|----------|-------------|---------|
| `>` | Mayor que | `x > 0` |
| `<` | Menor que | `x < 100` |
| `>=` | Mayor o igual | `x >= 0` |
| `<=` | Menor o igual | `x <= 100` |
| `==` | Igual | `x == 0` |
| `!=` | Diferente | `x != 0` |

---

## Contratos: asumir y demostrar

El corazón de SIL son los **contratos**: precondiciones (`asumir`) y postcondiciones (`demostrar`).

### `asumir` (Precondición)

Declara qué debe ser verdadero antes de que la tarea se ejecute.

```sil
definir tarea dividir(a: Entero64, b: Entero64) -> Entero64:
    asumir b != 0
    retornar a / b
```

### `demostrar` (Postcondición)

Declara qué debe ser verdadero después de que la tarea se ejecute.

```sil
definir tarea absoluto(x: Entero64) -> Entero64:
    demostrar resultado >= 0
    si x < 0:
        retornar -x
    retornar x
```

### Múltiples Contratos

```sil
definir tarea transferir(saldo: Entero64, monto: Entero64) -> Entero64:
    asumir saldo > 0
    asumir monto > 0
    asumir monto <= saldo
    demostrar saldo - monto >= 0
    retornar saldo - monto
```

---

## Gestión de Memoria

### Arenas O(1)

SIL gestiona la memoria con **Arenas** que asignan y liberan en tiempo constante.

```sil
definir tarea procesar(datos: Texto) -> Entero64:
    asumir datos.len > 0
    # La memoria se asigna en la arena de la tarea
    # y se libera automáticamente al retornar
    retornar datos.len
```

### Sin Garbage Collector

SIL no tiene recolector de basura. La memoria se libera determinísticamente.

---

## Ejemplos

### Ejemplo 1: Función Simple

```sil
definir tarea cuadrado(x: Entero64) -> Entero64:
    asumir x >= 0
    asumir x <= 46340
    demostrar x * x >= 0
    retornar x * x
```

### Ejemplo 2: Validación

```sil
definir tarea validar_edad(edad: Entero64) -> Booleano:
    asumir edad >= 0
    asumir edad <= 150
    retornar edad >= 18
```

### Ejemplo 3: Múltiples Tareas

```sil
definir tarea sumar(a: Entero64, b: Entero64) -> Entero64:
    retornar a + b

definir tarea restar(a: Entero64, b: Entero64) -> Entero64:
    retornar a - b

definir tarea calcular(x: Entero64, y: Entero64) -> Entero64:
    retornar sumar(x, y)
```

### Ejemplo 4: Control de Flujo

```sil
definir tarea clasificar(nota: Entero64) -> Texto:
    asumir nota >= 0
    asumir nota <= 100
    si nota >= 90:
        retornar "A"
    sino si nota >= 80:
        retornar "B"
    sino si nota >= 70:
        retornar "C"
    sino:
        retornar "F"
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
