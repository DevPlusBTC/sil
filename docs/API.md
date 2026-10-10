# SIL API Reference

## Module: `silc-std`

### Memory Management

```sil
tarea alloc<T>(size: Entero64) -> Puntero<T>
tarea free<T>(ptr: Puntero<T>) -> Void
tarea realloc<T>(ptr: Puntero<T>, new_size: Entero64) -> Puntero<T>
```

### Collections

```sil
// Array dinámico
tarea vec_new<T>(capacity: Entero64) -> Vec<T>
tarea vec_push<T>(v: Vec<T>, item: T) -> Void
tarea vec_get<T>(v: Vec<T>, index: Entero64) -> Option<T>
tarea vec_len<T>(v: Vec<T>) -> Entero64

// Hash map
tarea map_new<K, V>(capacity: Entero64) -> Map<K, V>
tarea map_insert<K, V>(m: Map<K, V>, key: K, value: V) -> Void
tarea map_get<K, V>(m: Map<K, V>, key: K) -> Option<V>

// Set
tarea set_new<T>(capacity: Entero64) -> Set<T>
tarea set_insert<T>(s: Set<T>, item: T) -> Booleano
tarea set_contains<T>(s: Set<T>, item: T) -> Booleano
```

### I/O

```sil
// File operations
tarea read_file(path: Texto) -> Result<Texto, ErrorIo>
tarea write_file(path: Texto, content: Texto) -> Result<Void, ErrorIo>
tarea append_file(path: Texto, content: Texto) -> Result<Void, ErrorIo>

// Standard I/O
tarea print(msg: Texto) -> Void
tarea println(msg: Texto) -> Void
tarea read_line() -> Texto
```

### Capabilities

```sil
// Network capabilities
definir capacidad NetworkAccess:
    permitir connect_a "host" puerto Entero64
    limite_ancho_banda: Texto

tarea open_connection(cap: NetworkAccess, host: Texto, port: Entero64) -> Socket

// File system capabilities
definir capacidad FileAccess:
    permitir read "ruta"
    permitir write "ruta"
    permitir delete "ruta"

tarea open_file(cap: FileAccess, path: Texto, mode: ModoArchivo) -> Result<Archivo, ErrorIo>
```

### Concurrency

```sil
// Channels
tarea channel_new<T>(capacity: Entero64) -> (Sender<T>, Receiver<T>)
tarea send<T>(tx: Sender<T>, value: T) -> Result<Void, ErrorSend>
tarea recv<T>(rx: Receiver<T>) -> Result<T, ErrorRecv>

// Tasks
tarea spawn<T>(f: fn() -> T) -> JoinHandle<T>
tarea await<T>(handle: JoinHandle<T>) -> T
```

## Types

### Primitives

| Type | Size | Range |
|------|------|-------|
| `Entero64` | 8 bytes | -2^63 to 2^63-1 |
| `Flotante64` | 8 bytes | IEEE 754 double |
| `Booleano` | 1 byte | `verdadero` / `falso` |
| `Texto` | variable | UTF-8 string |
| `Void` | 0 bytes | - |

### Composite

```sil
// Tuple
tupla(Entero64, Texto, Booleano)

// Result
variante Result<T, E>:
    ok(valor: T)
    error(e: E)

// Option
variante Option[T]:
    some(valor: T)
    none
```

## Contracts

### Preconditions (`asumir`)

```sil
tarea sqrt(x: Flotante64) -> Flotante64:
    asumir x >= 0.0
    ...
```

### Postconditions (`demostrar`)

```sil
tarea abs(x: Entero64) -> Entero64:
    demostrar resultado >= 0
    ...
```

## Error Handling

```sil
// Result type
tarea divide(a: Flotante64, b: Flotante64) -> Result<Flotante64, ErrorDivPorCero>

// Match
match divide(10.0, 0.0):
    ok(valor) -> print(valor)
    error(e) -> print("Error: división por cero")

// Unwrap with default
let valor = divide(10.0, 2.0) o 0.0
```

## Lifetimes

```sil
// Explicit lifetime
tarea borrow<'a>(x: &'a Texto) -> &'a Texto

// Reference counting
let rc = Rc::new(value)
let weak = Rc::downgrade(rc)
```

## Module System

```rust
// Import
importar std::io::{read_file, write_file}
importar std::collections::Map

// Export
exportar tarea public_function(x: Entero64) -> Entero64
exportar type PublicType
exportar capacidad PublicCapability
```

## Standard Library Organization

```
std/
├── io/          # File and network I/O
├── collections/ # Vec, Map, Set, etc.
├── sync/        # Channels, locks, atomics
├── fs/          # File system operations
├── net/         # Network protocols
├── time/        # Duration, Instant
├── process/     # Process management
└── prelude      # Common imports
```
