# SPEC v2 — Gramática S-expression canónica de SIL

**Estado:** contrato de reescritura del frontend (`silc-frontend`).
**Ámbito:** sustituye la sintaxis CNL indentada por una sintaxis S-expression.
**Destino:** el AST de `silc-core/silc-frontend/src/ast.rs` (sin cambios de tipos).
**Fuentes de verdad:** `ast.rs`, `reader.rs`, `a_ast.rs`, `examples/sexp/*.sexp`.

Este documento es normativo. Cada forma listada aquí corresponde a un nodo del AST
y cada nodo del AST aparece aquí. Si una forma no aparece, es un defecto de este
documento, no una libertad del implementador.

---

## 1. Modelo léxico

### 1.1 Alfabeto del documento

Un documento es una secuencia de *sexps* separados por espacio en blanco. El
espacio en blanco (espacio, tabulador, `\r`, `\n`) y los comentarios no tienen
significado sintáctico: **no hay INDENT/DEDENT, no hay saltos de línea
significativos, no hay precedencia de operadores**.

```ebnf
Documento     ::= { Sexp } ;
Sexp          ::= Atomo | Texto | Numero | Lista ;
Lista         ::= "(" { Sexp } ")" ;
Atomo         ::= Identificador | Operador ;
Identificador ::= [A-Za-z_] [A-Za-z0-9_]* ;
Operador      ::= "+" | "-" | "*" | "/" | ">" | "<" | "==" | "!=" | ">=" | "<=" ;
Numero        ::= Entero | Flotante ;
Texto         ::= '"' { CarTexto | Escape } '"' ;
Comentario    ::= ";" { cualquier-carácter-salvo-NL } ;
```

### 1.2 Átomos

Un átomo es una palabra sin comillas. Se distinguen dos clases, y solo dos:

| Clase | Forma | Ejemplos |
|---|---|---|
| Identificador | `[A-Za-z_][A-Za-z0-9_]*` (ASCII; sin acentos, sin `ñ`, sin `-`) | `sumar`, `x`, `gestion_memoria`, `USD`, `i64` |
| Operador | conjunto fijo de 10 símbolos | `+ - * / > < == != >= <=` |

**No hay palabras reservadas.** `tarea`, `si`, `retornar`, `i64` son átomos
ordinarios: un átomo es un átomo. Lo que distingue a `(task ...)` de
`(foo ...)` es la **posición de cabeza de lista**, no el vocabulario. En
consecuencia, `(let tarea 1)` es legal: declara una variable llamada `tarea`.

Excepción única y explícita (§9, `identificador_reservado`): `verdadero` y
`falso` no pueden usarse como nombre en posición de enlace (parámetro, campo,
variable, token de capacidad), porque en posición de expresión se leen como
literales booleanos.

Caracteres **no** válidos en un átomo: `(`, `)`, `"`, `;`, `:`, `,`, `.`, `[`,
`]`, `{`, `}`, `|`, `=`, `'`, y cualquier carácter fuera de ASCII en
identificadores. Un átomo que empiece por dígito es un número, no un átomo.

### 1.3 Números

| Clase | Léxico | Tipo destino | Regla |
|---|---|---|---|
| Entero | `[+-]?[0-9]+` | `Literal::Entero(i64)` | debe caber en `i64`; sin `_`, sin `0x`, sin exponente |
| Flotante | `[+-]?[0-9]+\.[0-9]+` | `Literal::Flotante(f64)` | el punto es obligatorio; dígitos a ambos lados; sin exponente |

Desambiguación de signo (regla léxica, sin ambigüedad): un token que empieza por
dígito es número; un token que empieza por `+` o `-` **seguido inmediatamente de
un dígito** es número con signo; en cualquier otro caso `+` y `-` son átomos
operadores. Así `-5` es el entero −5, y `(- x 5)` usa el operador `-`.

Desbordamiento de `i64` o flotante mal formado → `numero_invalido`.

### 1.4 Textos

Delimitados por `"`. Escapes reconocidos (idénticos a `unescape_string` del
lexer actual):

| Escape | Produce |
|---|---|
| `\\` | `\` |
| `\"` | `"` |
| `\n` | salto de línea (U+000A) |
| `\t` | tabulador (U+0009) |
| `\r` | retorno de carro (U+000D) |

Un escape no reconocido (`\q`) **se conserva literalmente** como dos caracteres
`\` y `q`. Esto replica el comportamiento actual y evita romper rutas Windows
(`"C:\temp"` → `C:<TAB>emp`, aviso registrado en §10).

Un texto sin cerrar antes del fin de archivo → `texto_sin_cerrar`. Un `\n` real
dentro de un texto **no** se permite (el lexer actual tampoco lo acepta).

### 1.5 Booleanos

`verdadero` y `falso` son átomos. El lector los convierte a
`Literal::Booleano(true|false)` **solo en posición de expresión**. No son formas:
`(verdadero)` es `forma_desconocida`, no un literal.

### 1.6 Comentarios

`;` hasta el final de la línea. Un `;` dentro de un texto no es comentario.
No se admiten comentarios de bloque. Un BOM UTF-8 (`U+FEFF`) al inicio del
archivo se ignora.

### 1.7 Qué se pierde respecto a CNL (y es deliberado)

| CNL | S-expression |
|---|---|
| INDENT/DEDENT | anidamiento por paréntesis |
| precedencia y asociatividad | árbol explícito |
| `si ... entonces ... :` | `(if COND BODY)` |
| paréntesis de agrupación | ya no existen: el árbol es la agrupación |
| `-x` (menos unario) | `(- 0 x)` |
| `x.prop` | `(get x prop)` |
| `a < b < c` (rechazado) | `(< (< a b) c)` (aceptado por el lector, rechazado por el checker) |

---

## 2. Formas de nivel superior

Un documento es `Program { defs: Vec<Decl> }`: una secuencia de formas de nivel
superior, sin envoltorio.

| # | `Decl` | Forma canónica |
|---|---|---|
| 1 | `Tarea` | `(task NOMBRE (params PARAM*) (returns TIPO)? (requires EXPR*) (ensures EXPR*) BODY)` |
| 2 | `Estructura` | `(struct NOMBRE (fields CAMPO*))` |
| 3 | `Variante` | `(variant NOMBRE (case NOMBRE (fields CAMPO*))*)` |
| 4 | `Servicio` | `(service NOMBRE (net ...)? (handler ...)* (task ...)* (restrictions RESTRICCION*)?)` |
| 5 | `Modelo` | `(model NOMBRE (fields CAMPO*))` |
| 6 | `MemoriaPersistente` | `(memory NOMBRE (fields CAMPO*))` |
| 7 | `Capacidad` | `(capability NOMBRE (resource "RECURSO") (port N) (bandwidth "LIMITE"))` |
| 8 | `Modulo` | `(module NOMBRE (export CLASE NOMBRE)*)` |
| 9 | `Migracion` | `(migration ORIGEN (from V) (to V) (field "CAMPO") (record REG) (default EXPR) (restrictions RESTRICCION*)?)` |

Formas auxiliares usadas arriba:

```ebnf
PARAM        ::= "(" NOMBRE TIPO ")" ;
CAMPO        ::= "(" NOMBRE TIPO ")" ;
BODY         ::= "(" "body" STMT* [ "(" "restrictions" RESTRICCION* ")" ] ")" ;
RESTRICCION  ::= "(" CLAVE VALOR ")" ;   ; ambos átomos, ninguno numérico
CLASE        ::= "service" | "task" | "struct" | "variant" | "capability" ;
```

### 2.1 `task`

```lisp
(task sumar
  (params (x i64) (y i64))
  (returns i64)
  (requires (gt x 0))
  (ensures (gt (add x y) y))
  (body
    (return (add x y))))
```

| Elemento | Obligatorio | Aridad | Nota |
|---|---|---|---|
| `params` | sí | 2 | lista plana de pares; `(params)` válido |
| `returns` | no | 2 | ausente ⇒ `TareaDecl.retorno == None` (tarea Void) |
| `requires` | sí | ≥ 2 | repetible, una forma por precondición |
| `ensures` | sí | ≥ 2 | repetible, una forma por postcondición |
| `body` | sí | ≥ 1 | `Cuerpo` |

Orden canónico: `params`, `returns`, `requires`*, `ensures`*, `body`. El lector
acepta cualquier orden de los hijos nombrados y exige como máximo uno de
`params`/`returns`/`body`; `requires`/`ensures` pueden repetirse o faltar
(equivale a vacío). Un `(returns ...)` repetido → `forma_duplicada`.

### 2.2 `struct`, `model`, `memory`

Las tres comparten `(fields CAMPO*)` porque el AST las modela igual.

```lisp
(struct Cliente (fields (id i64) (nombre texto)))
(model  Cliente (fields (id i64)))
(memory Sesion  (fields (token texto) (expira i64)))
```

`(fields)` vacío es legal (el parser actual acepta una estructura sin campos).

### 2.3 `variant`

```lisp
(variant Resultado
  (case Ok  (fields (valor i64)))
  (case Err (fields)))
```

`(fields ...)` es **obligatorio** incluso sin datos, para que la aridad sea
uniforme: `(case Err)` es `aridad_invalida`. Cada `case` tiene exactamente
3 elementos.

### 2.4 `service`

```lisp
(service Pasarela
  (net 8080 (protocol tcp))
  (handler Pago (params (monto i64))
    (body
      (assert (gt monto 0))
      (return monto)))
  (task health
    (params)
    (returns texto)
    (requires)
    (ensures)
    (body (return "ok")))
  (restrictions (gestion_memoria arena)))
```

| Hijo | Cardinalidad | Forma |
|---|---|---|
| `net` | 0 o 1 | `(net PUERTO)` o `(net PUERTO (protocol PROTO))` |
| `handler` | 0..n | `(handler EVENTO (params PARAM*) BODY)` |
| `task` | 0..n | idéntica a §2.1 |
| `restrictions` | 0 o 1 | `(restrictions RESTRICCION*)` |

`ServicioDecl.red == None` ⇔ no hay `net`. Dos `net` o dos `restrictions` →
`forma_duplicada`. `PROTO` es un átomo libre (`tcp`, `udp`, `quic`, …): el
parser actual no valida el vocabulario.

### 2.5 `capability`

```lisp
(capability RedSalida
  (resource "api.ejemplo.com:443")
  (port 443)
  (bandwidth "10Gbps"))
```

Los tres hijos son obligatorios y en ese orden. `resource` y `bandwidth` exigen
texto entre comillas (`texto_esperado`); `port` exige entero no negativo
(`puerto_fuera_de_rango`). En CNL el límite se escribía como
`limite_ancho_banda: "..."`; el nombre de la forma pasa a `bandwidth` y el
`resource` absorbe el `permitir conectar a "..."`.

### 2.6 `module`

```lisp
(module Pagos
  (export service Pasarela)
  (export task cobrar)
  (export struct Recibo))
```

`CLASE` ∈ `{service, task, struct, variant, capability}` (las cinco variantes de
`ClaseExportada`). Cualquier otro átomo → `clase_exportada_invalida`.
`(export ...)` puede repetirse; un módulo sin exportaciones es `(module X)`.

### 2.7 `migration`

```lisp
(migration Usuario
  (from v1)
  (to   v2)
  (field "email")
  (record Usuario)
  (default "")
  (restrictions (gestion_memoria arena)))
```

| Hijo | Obligatorio | Nota |
|---|---|---|
| `from`, `to` | sí | átomos de versión; no se valida su forma |
| `field` | sí | texto entre comillas |
| `record` | sí | átomo |
| `default` | sí | expresión arbitraria |
| `restrictions` | no | 0 o 1 |

Orden canónico el de la tabla; el lector no lo exige.

---

## 3. Sentencias

`Cuerpo` = `(body STMT* (restrictions RESTRICCION*)?)`. `restrictions`, si
aparece, es **el último hijo** del `body`; en cualquier otra posición →
`restricciones_fuera_de_lugar` (hoy `bajo restricciones` cierra el cuerpo).

| # | `Stmt` | Forma | Aridad de lista |
|---|---|---|---|
| 1 | `Si` | `(if COND BODY (else BODY)?)` | 3 o 4 |
| 2 | `Verificar` | `(assert COND)` | 2 |
| 3 | `Asumir` | `(assume COND)` | 2 |
| 4 | `Demostrar` | `(prove COND)` | 2 |
| 5 | `InvarianteBucle` | `(invariant COND)` | 2 |
| 6 | `Mientras` | `(while COND BODY)` | 3 |
| 7 | `Para` | `(for VAR ITER BODY)` | 4 |
| 8 | `Asignar` (sin tipo, sin mut) | `(let NOMBRE EXPR)` | 3 |
| 9 | `Asignar` (con tipo, sin mut) | `(let NOMBRE TIPO EXPR)` | 4 |
| 10 | `Asignar` (sin tipo, mut) | `(let-mut NOMBRE EXPR)` | 3 |
| 11 | `Asignar` (con tipo, mut) | `(let-mut NOMBRE TIPO EXPR)` | 4 |
| 12 | `Retornar` (con expr) | `(return EXPR)` | 2 |
| 13 | `Retornar` (sin expr) | `(return)` | 1 |
| 14 | `SolicitarCapacidad` | `(request PERMISO "RECURSO" (ttl N)? (as NOMBRE)?)` | 3..5 |
| 15 | `ValidarCapacidad` | `(validate NOMBRE)` | 2 |
| 16 | `EnviarCanal` | `(send EXPR CANAL)` | 3 |
| 17 | `RecibirCanal` | `(recv CANAL NOMBRE)` | 3 |
| 18 | `GenerarFibra` | `(spawn TAREA)` | 2 |
| 19 | `CrearCanal` | `(chan NOMBRE (capacity N)?)` | 2 o 3 |
| 20 | `OperarIO` `LeerArchivo` | `(io-read "RUTA" CAP NOMBRE)` | 4 |
| 21 | `OperarIO` `EscribirArchivo` | `(io-write EXPR "RUTA" CAP)` | 4 |
| 22 | `OperarIO` `EnviarRed` | `(io-send EXPR "DESTINO" CAP)` | 4 |
| 23 | `OperarIO` `RecibirRed` | `(io-recv "DESTINO" CAP NOMBRE)` | 4 |

Detalles normativos:

- **`let` vs `let-mut`.** Dos cabezas distintas en vez de un `mut` posicional:
  elimina todo átomo contextual y toda ambigüedad de aridad. `TIPO` se distingue
  de `EXPR` porque un tipo es un átomo del conjunto de §5 o una lista con cabeza
  `list`/`map`/`set`/`tuple`.
- **`return` sin expresión.** Solo válido si la tarea contenedora es Void
  (`(returns ...)` ausente o `void`). Ver §9.
- **`if` con `sino si`.** El CNL anidaba: `(if c1 B1 (else (body (if c2 B2 ...))))`.
  No hay forma especial.
- **`request`.** Orden fijo `ttl` antes de `as`. `PERMISO` ∈
  `{lectura_archivo, escritura_archivo, red, ejecucion}`; otro átomo →
  `permiso_invalido`. `N` es entero no negativo (u64).
- **`chan`.** `(capacity N)` es opcional; ausente ⇒ `CrearCanal.capacidad == None`
  (canal sin buffer declarado).
- **`io-*`.** Cuatro cabezas en vez de `(io OP ...)` para que cada una tenga
  aridad fija 4. `CAP` es el átomo que nombra la capacidad ya solicitada.

---

## 4. Expresiones

Prefijo uniforme, sin excepciones: `(op a b)`. Los operandos son sexps.

| # | `Expr` | Forma | Aridad |
|---|---|---|---|
| 1 | `Var(Ident)` | `NOMBRE` (átomo) | — |
| 2 | `Lit(Entero)` | `42`, `-7` | — |
| 3 | `Lit(Flotante)` | `1.5` | — |
| 4 | `Lit(Texto)` | `"hola"` | — |
| 5 | `Lit(Booleano)` | `verdadero`, `falso` | — |
| 6 | `AccesoProp` | `(get BASE PROP)` | 3 |
| 7 | `BinOp` | `(OP LHS RHS)` | 3 |

### 4.1 Los 10 `BinOp`

| `BinOp` | Símbolo S-exp | CNL simbólico | CNL verbal |
|---|---|---|---|
| `Gt` | `>` | `>` | `sea mayor a` |
| `Lt` | `<` | `<` | `sea menor a` |
| `Eq` | `==` | `==` | `sea igual a` |
| `Ne` | `!=` | `!=` | — (solo simbólico) |
| `Ge` | `>=` | `>=` | — (solo simbólico) |
| `Le` | `<=` | `<=` | — (solo simbólico) |
| `Add` | `+` | `+` | — |
| `Sub` | `-` | `-` | — |
| `Mul` | `*` | `*` | — |
| `Div` | `/` | `/` | — |

Las formas verbales del CNL desaparecen: el lector solo entiende el símbolo.
`a sea mayor a b` se escribe `(> a b)`.

### 4.2 Reglas de construcción

- **Asociatividad explícita.** `a + b + c` (CNL: izquierda) ⇒ `(+ (+ a b) c)`.
  `a - b - c` ⇒ `(- (- a b) c)`. `a * b / c` ⇒ `(/ (* a b) c)`.
  `a + b * c` ⇒ `(+ a (* b c))` (precedencia CNL conservada por el árbol).
- **Menos unario.** `-x` ⇒ `(- 0 x)`. Nunca `(- x)` (aridad 2 es inválida).
- **Comparaciones.** El CNL no permitía encadenar (`a < b < c` era error). En
  S-exp `(< (< a b) c)` es sintácticamente válido; rechazarlo es trabajo del
  checker de tipos (§9, `comparacion_encadenada`).
- **Profundidad.** Se mantiene el límite actual de 256 niveles de anidamiento de
  listas en posición de expresión → `expresion_demasiado_profunda`.
- **Sin llamadas.** El AST no tiene `Expr::Llamada`; `(f x)` es
  `forma_desconocida`. Las invocaciones no existen en el lenguaje todavía.

---

## 5. Tipos

`TipoDato` tiene 13 variantes: 8 hojas + 4 compuestas + `Nominal`.

| # | `TipoDato` | Forma | Notas |
|---|---|---|---|
| 1 | `Entero64` | `i64` | |
| 2 | `Flotante64` | `f64` | |
| 3 | `Booleano` | `booleano` | |
| 4 | `Texto` | `texto` | |
| 5 | `CapacidadHardware` | `capacidad` | |
| 6 | `Void` | `void` | |
| 7 | `USD` | `USD` | mayúsculas, como en el lexer |
| 8 | `EUR` | `EUR` | mayúsculas, como en el lexer |
| 9 | `Lista(T)` | `(list T)` | aridad 2 |
| 10 | `Mapa(K,V)` | `(map K V)` | aridad 3, clave primero |
| 11 | `Conjunto(T)` | `(set T)` | aridad 2 |
| 12 | `Tupla(Vec<T>)` | `(tuple T*)` | `(tuple)` válido (tupla vacía) |
| 13 | `Nominal(String)` | `NOMBRE` | cualquier átomo que no sea 1..12 |

Anidamiento:

```lisp
(list i64)
(list (list texto))
(map texto i64)
(map texto (list (tuple i64 booleano)))
(set i64)
(tuple i64 texto)
(tuple)
Usuario            ; nominal
(list Usuario)     ; nominal anidado
```

Un átomo en posición de tipo que no esté en las filas 1..8 se interpreta como
`Nominal`, **no** como error: el parser actual hace lo mismo
(`Token::Ident(_) => TipoDato::Nominal`). Una lista en posición de tipo cuya
cabeza no sea `list`/`map`/`set`/`tuple` → `tipo_desconocido`. Un número o texto
en posición de tipo → `tipo_desconocido`.

---

## 6. Restricciones de hardware

```lisp
(restrictions
  (gestion_memoria arena)
  (tiempo_ejecucion constante)
  (latencia_maxima baja))
```

`Restriccion { clave: String, valor: String, span }`. Ambos campos son `String`,
no `Ident`: el lector no construye identificadores.

Reglas heredadas del parser actual (`palabra_restriccion` + `parse_restricciones`):

1. `clave` debe ser un **átomo no numérico**. Un entero, flotante o texto →
   `restriccion_no_atomica`. (Test existente: `latencia_maxima: 200` debe fallar.)
2. `valor` idem: átomo no numérico. Es la razón por la que `200us` no se acepta
   hoy; sigue sin aceptarse.
3. Aridad exacta 2 por restricción.
4. El bloque puede estar vacío: `(restrictions)`.
5. El lector **no** valida el vocabulario de claves ni de valores. En CNL
   `arena`, `atomica`, `asincrona`, `en_memoria`, `cero_pausas` eran keywords del
   lexer; en S-exp son átomos como cualquier otro, así que el conjunto aceptado
   es «cualquier átomo». La validación semántica de claves es del checker.

Claves y valores en uso en el repositorio (informativo, **no** cerrado):

| Clave | Valores vistos |
|---|---|
| `gestion_memoria` | `arena`, `en_memoria` |
| `tiempo_ejecucion` | `constante`, `cero_pausas` |
| `latencia_maxima` | `baja` |
| `concurrencia` | `atomica`, `asincrona` |

---

## 7. Reglas de spans

`Span = Range<usize>` con offsets de **byte** sobre el fuente UTF-8, extremo
final **exclusivo**. Solo los nodos que declaran `span` en `ast.rs` lo reciben.

| Nodo | Span asignado |
|---|---|
| `Ident` | el del átomo que lo produjo (sin espacios) |
| `Expr::BinOp` | el de la lista `(OP ...)` completa, de `(` a después de `)` |
| `Expr::AccesoProp` | el de la lista `(get ...)` completa |
| `Expr::Var`, `Expr::Lit` | no tienen span propio (viven en el `Ident` o son literales) |
| `Stmt::*` | el de la lista de la sentencia, `(`..`)`+1 |
| `Cuerpo` | no tiene span |
| `TareaDecl` | el de `(task ...)` |
| `EstructuraDecl` / `ModeloDecl` / `MemoriaPersistenteDecl` | el de `(struct|model|memory ...)` |
| `VarianteDecl` | el de `(variant ...)` |
| `ServicioDecl` | el de `(service ...)` |
| `ManejadorEvento` | el de `(handler ...)` |
| `CapacidadDecl` | el de `(capability ...)` |
| `ModuloDecl` | el de `(module ...)` |
| `MigracionDecl` | el de `(migration ...)` |
| `Restriccion` | el de su lista `(CLAVE VALOR)` |

Regla general: **cada lista lleva el span de su paréntesis de apertura hasta el
de cierre; cada átomo su span propio.** Los comentarios y el espacio en blanco
nunca forman parte de un span, salvo que estén *dentro* de una lista, cuyo span
los cubre por construcción.

Un span vacío (`start == end`) solo es válido en el documento vacío
(`Program { defs: [] }`). Los errores del lector llevan el span del sexp que
falló, y si el fallo es por falta de elementos, el span del `)` de cierre o del
fin de archivo (`len..len`).

---

## 8. Traducción completa de `examples/`

14 archivos. Tres son idénticos byte a byte: `sexp/sumar.sexp` =
`sexp/test_calc.sexp` = `sexp/calculadora_segura.sexp`. `sexp/fibra_basica.sexp`
y `sexp/scheduler_real.sexp` tienen el mismo cuerpo y solo cambia el nombre de
la tarea. Se traducen todos, uno por fila, porque el contrato es por archivo.

| # | Archivo | Formas de nivel superior |
|---|---|---|
| 1 | `sexp/sumar.sexp` | 1 `task` |
| 2 | `sexp/calculadora_segura.sexp` | 1 `task` |
| 3 | `sexp/test_calc.sexp` | 1 `task` |
| 4 | `sexp/descuentos.sexp` | 1 `task` |
| 5 | `sexp/factorial.sexp` | 4 `task` |
| 6 | `sexp/inventario.sexp` | 2 `task` |
| 7 | `sexp/procesar_pago.sexp` | 1 `task` |
| 8 | `sexp/programa_confianza.sexp` | 2 `task` |
| 9 | `sexp/validacion.sexp` | 2 `task` |
| 10 | `sexp/verificacion_z3.sexp` | 1 `task` |
| 11 | `sexp/verificacion_z3_compleja.sexp` | 1 `task` |
| 12 | `sexp/falla_smt.sexp` | 1 `task` |
| 13 | `sexp/fibra_basica.sexp` | 1 `task` |
| 14 | `sexp/scheduler_real.sexp` | 1 `task` |

### 8.1 `examples/sexp/sumar.sexp`

```sil
definir tarea sumar(x: Entero64, y: Entero64) -> Entero64:
    retornar x + y
```

```lisp
(task sumar
  (params (x i64) (y i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (return (+ x y))))
```

### 8.2 `examples/sexp/calculadora_segura.sexp`

Idéntico a 8.1.

```sil
definir tarea sumar(x: Entero64, y: Entero64) -> Entero64:
    retornar x + y
```

```lisp
(task sumar
  (params (x i64) (y i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (return (+ x y))))
```

### 8.3 `examples/sexp/test_calc.sexp`

Idéntico a 8.1.

```sil
definir tarea sumar(x: Entero64, y: Entero64) -> Entero64:
    retornar x + y
```

```lisp
(task sumar
  (params (x i64) (y i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (return (+ x y))))
```

### 8.4 `examples/sexp/descuentos.sexp`

```sil
definir tarea descuento_porcentaje(precio: Entero64, pct: Entero64) -> Entero64:
    asumir precio > 0
    asumir precio < 10000
    asumir pct >= 0
    asumir pct <= 50
    retornar precio * (100 - pct) / 100
```

```lisp
(task descuento_porcentaje
  (params (precio i64) (pct i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (assume (> precio 0))
    (assume (< precio 10000))
    (assume (>= pct 0))
    (assume (<= pct 50))
    (return (/ (* precio (- 100 pct)) 100))))
```

`precio * (100 - pct) / 100` es `((precio * (100 - pct)) / 100)` en CNL, por
asociatividad a la izquierda de `*` y `/` al mismo nivel.

### 8.5 `examples/sexp/factorial.sexp`

```sil
definir tarea factorial(n: Entero64) -> Entero64:
    asumir n >= 0
    asumir n <= 20
    asumir n == 0
    retornar 1

definir tarea factorial_1(n: Entero64) -> Entero64:
    asumir n >= 0
    asumir n <= 20
    asumir n == 1
    retornar 1

definir tarea factorial_2(n: Entero64) -> Entero64:
    asumir n >= 0
    asumir n <= 20
    asumir n == 2
    retornar 2

definir tarea factorial_3(n: Entero64) -> Entero64:
    asumir n >= 0
    asumir n <= 20
    asumir n == 3
    retornar 6
```

```lisp
(task factorial
  (params (n i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (assume (>= n 0))
    (assume (<= n 20))
    (assume (== n 0))
    (return 1)))

(task factorial_1
  (params (n i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (assume (>= n 0))
    (assume (<= n 20))
    (assume (== n 1))
    (return 1)))

(task factorial_2
  (params (n i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (assume (>= n 0))
    (assume (<= n 20))
    (assume (== n 2))
    (return 2)))

(task factorial_3
  (params (n i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (assume (>= n 0))
    (assume (<= n 20))
    (assume (== n 3))
    (return 6)))
```

### 8.6 `examples/sexp/inventario.sexp`

```sil
definir tarea valor_producto(precio: Entero64, stock: Entero64) -> Entero64:
    asumir precio > 0
    asumir precio < 1000000
    asumir stock >= 0
    asumir stock < 10000
    demostrar precio > 0
    demostrar stock >= 0
    retornar precio * stock

definir tarea agregar_stock(stock: Entero64, cantidad: Entero64) -> Entero64:
    asumir cantidad > 0
    asumir cantidad < 1000
    asumir stock >= 0
    asumir stock < 10000
    demostrar stock + cantidad > stock
    retornar stock + cantidad
```

```lisp
(task valor_producto
  (params (precio i64) (stock i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (assume (> precio 0))
    (assume (< precio 1000000))
    (assume (>= stock 0))
    (assume (< stock 10000))
    (prove (> precio 0))
    (prove (>= stock 0))
    (return (* precio stock))))

(task agregar_stock
  (params (stock i64) (cantidad i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (assume (> cantidad 0))
    (assume (< cantidad 1000))
    (assume (>= stock 0))
    (assume (< stock 10000))
    (prove (> (+ stock cantidad) stock))
    (return (+ stock cantidad))))
```

### 8.7 `examples/sexp/procesar_pago.sexp`

```sil
definir tarea procesar_pago(monto: Entero64) -> Entero64:
    asumir monto > 0
    demostrar monto > 0
    let impuesto: Entero64 = monto * 19
    retornar monto + impuesto
    bajo restricciones:
        gestion_memoria: arena
        tiempo_ejecucion: constante
```

```lisp
(task procesar_pago
  (params (monto i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (assume (> monto 0))
    (prove (> monto 0))
    (let impuesto i64 (* monto 19))
    (return (+ monto impuesto))
    (restrictions
      (gestion_memoria arena)
      (tiempo_ejecucion constante))))
```

### 8.8 `examples/sexp/programa_confianza.sexp`

```sil
definir tarea trabajo() -> Entero64:
    crear canal con capacidad 8 como c
    enviar 40 en c
    recibir de c como v
    retornar v + 2

definir tarea principal() -> Entero64:
    solicitar capacidad lectura_archivo para "/tmp/datos" como cap
    validar cap
    generar trabajo
    retornar 42
```

```lisp
(task trabajo
  (params)
  (returns i64)
  (requires)
  (ensures)
  (body
    (chan c (capacity 8))
    (send 40 c)
    (recv c v)
    (return (+ v 2))))

(task principal
  (params)
  (returns i64)
  (requires)
  (ensures)
  (body
    (request lectura_archivo "/tmp/datos" (as cap))
    (validate cap)
    (spawn trabajo)
    (return 42)))
```

### 8.9 `examples/sexp/validacion.sexp`

```sil
definir tarea validar_edad(edad: Entero64) -> Entero64:
    asumir edad >= 18
    asumir edad <= 150
    demostrar edad >= 18
    retornar 1

definir tarea es_mayor_18(edad: Entero64) -> Entero64:
    asumir edad >= 18
    asumir edad <= 150
    retornar 1
```

```lisp
(task validar_edad
  (params (edad i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (assume (>= edad 18))
    (assume (<= edad 150))
    (prove (>= edad 18))
    (return 1)))

(task es_mayor_18
  (params (edad i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (assume (>= edad 18))
    (assume (<= edad 150))
    (return 1)))
```

### 8.10 `examples/sexp/verificacion_z3.sexp`

```sil
definir tarea test_z3(x: Entero64) -> Booleano:
    asumir que x > 0
    demostrar que x * 2 > x
    retornar verdadero
```

```lisp
(task test_z3
  (params (x i64))
  (returns booleano)
  (requires)
  (ensures)
  (body
    (assume (> x 0))
    (prove (> (* x 2) x))
    (return verdadero)))
```

### 8.11 `examples/sexp/verificacion_z3_compleja.sexp`

```sil
definir tarea test_z3_complex(x: Entero64, y: Entero64) -> Booleano:
    asumir que x > 0
    asumir que y > 0
    demostrar que x + y > x
    demostrar que x + y > y
    demostrar que x * y > 0
    retornar verdadero
```

```lisp
(task test_z3_complex
  (params (x i64) (y i64))
  (returns booleano)
  (requires)
  (ensures)
  (body
    (assume (> x 0))
    (assume (> y 0))
    (prove (> (+ x y) x))
    (prove (> (+ x y) y))
    (prove (> (* x y) 0))
    (return verdadero)))
```

### 8.12 `examples/sexp/falla_smt.sexp`

```sil
definir tarea insegura(monto: Entero64) -> Entero64:
    demostrar monto > 1000000
    retornar monto
```

```lisp
(task insegura
  (params (monto i64))
  (returns i64)
  (requires)
  (ensures)
  (body
    (prove (> monto 1000000))
    (return monto)))
```

### 8.13 `examples/sexp/fibra_basica.sexp`

```sil
definir tarea test_fiber():
    asumir que verdadero
    demostrar que verdadero
    retornar verdadero
```

```lisp
(task test_fiber
  (params)
  (requires)
  (ensures)
  (body
    (assume verdadero)
    (prove verdadero)
    (return verdadero)))
```

`(returns ...)` se omite: la tarea no declara tipo de retorno, pero devuelve un
booleano. Es uno de los motivos por los que el archivo vive en `sexp/`; el
lector lo acepta, el checker lo rechaza (`retorno_con_valor_en_tarea_void`).

### 8.14 `examples/sexp/scheduler_real.sexp`

Idéntico a 8.13 salvo el nombre de la tarea (`test_fiber` → `test_scheduler_real`).

```sil
definir tarea test_scheduler_real():
    asumir que verdadero
    demostrar que verdadero
    retornar verdadero
```

```lisp
(task test_scheduler_real
  (params)
  (requires)
  (ensures)
  (body
    (assume verdadero)
    (prove verdadero)
    (return verdadero)))
```

---

## 9. Invariantes que el lector debe validar

El lector (reader) sustituye al parser: produce el mismo `Program` y debe
rechazar exactamente lo que el parser rechazaba, más lo que la nueva forma hace
verificable. Todo error lleva nombre, span y mensaje en español.

### 9.1 Errores léxicos

| Nombre | Cuándo |
|---|---|
| `texto_sin_cerrar` | `"` sin cierre antes de EOF |
| `numero_invalido` | entero fuera de `i64`, flotante mal formado (`1.`, `.5`, `1e9`) |
| `caracter_ilegal` | byte que no inicia átomo, texto, número, lista, comentario ni espacio |
| `parentesis_desbalanceado` | `)` sin `(` abierta |
| `lista_sin_cerrar` | `(` sin `)` al llegar a EOF |

### 9.2 Errores estructurales

| Nombre | Cuándo |
|---|---|
| `sexp_esperada` | se esperaba un sexp y llegó EOF |
| `lista_vacia` | `()` en cualquier posición (ninguna forma tiene aridad 0) |
| `cabeza_no_atomica` | lista cuya cabeza es número, texto o lista: `(42 x)`, `("f" x)`, `((a) b)` |
| `forma_desconocida` | cabeza de lista que no es una forma de este documento |
| `aridad_invalida` | número de hijos fuera del rango de la tabla de la forma |
| `forma_duplicada` | `(returns ...)`, `(net ...)` o `(restrictions ...)` repetidos |

### 9.3 Errores de nombre y tipo

| Nombre | Cuándo |
|---|---|
| `identificador_invalido` | se espera nombre y llega un operador (`+`, `==`) |
| `identificador_reservado` | `verdadero`/`falso` en posición de enlace |
| `tipo_desconocido` | lista de tipo con cabeza ∉ `{list, map, set, tuple}`; número o texto como tipo |
| `texto_esperado` | se espera `"..."`: `resource`, `bandwidth`, `field`, `RUTA`, `RECURSO`, `DESTINO` |
| `entero_esperado` | se espera número entero: `port`, `capacity`, `ttl` |
| `puerto_fuera_de_rango` | `port`/`ttl`/`capacity` negativo o > `u64::MAX` |
| `permiso_invalido` | `PERMISO` ∉ `{lectura_archivo, escritura_archivo, red, ejecucion}` |
| `clase_exportada_invalida` | `CLASE` ∉ `{service, task, struct, variant, capability}` |
| `restriccion_no_atomica` | clave o valor de restricción es número o texto |
| `restricciones_fuera_de_lugar` | `(restrictions ...)` no es el último hijo del `body` |

### 9.4 Errores de contexto (exigen la tabla de tareas ya leída)

| Nombre | Cuándo |
|---|---|
| `retorno_sin_valor_en_tarea_con_tipo` | `(return)` en tarea con `(returns T)`, `T != void` |
| `retorno_con_valor_en_tarea_void` | `(return EXPR)` en tarea sin `returns` o con `void` |
| `comparacion_encadenada` | un operador relacional como operando directo de otro |
| `expresion_demasiado_profunda` | más de 256 listas anidadas en una expresión |
| `tarea_inexistente_en_spawn` | `(spawn X)` con `X` sin `task` en el documento (aviso, no error duro: el backend actual tampoco lo exige) |
| `nombre_duplicado` | dos `task`/`struct`/… con el mismo nombre en el documento |

Los cuatro últimos son **post-lectura**: se calculan sobre el `Program` completo,
no durante el descenso. `retorno_*` y `nombre_duplicado` se documentan aquí
porque el contrato de aceptación es el mismo (el compilador los rechaza), pero el
lector puede emitirlos en una pasada de validación separada.

### 9.5 Invariantes que el lector NO valida (y no debe inventar)

- Vocabulario de claves/valores de restricción (§6).
- Vocabulario del `protocol` de red.
- Existencia de la capacidad nombrada en `(send ...)`, `(io-* ...)`.
- Tipos de los operandos: `(+ "a" 1)` es sintácticamente válido.
- Alcance de variables, uso antes de definición.
- Que `(spawn X)` apunte a una tarea sin parámetros (hoy lo dice el comentario
  del AST, no el parser).

---

## 10. Decisiones abiertas

Ninguna bloquea la implementación: cada una tiene una opción propuesta ya
aplicada en este documento. Se listan para que el revisor pueda vetarlas.

| # | Tema | Decisión aplicada | Alternativa descartada |
|---|---|---|---|
| 1 | Nombres de forma | inglés (`task`, `struct`, `if`, `let`) siguiendo el ejemplo del contrato | español (`tarea`, `estructura`, `si`), coherente con CNL pero mezcla dos vocabularios |
| 2 | `let` mutable | dos cabezas: `let` / `let-mut` | `(let mut x 1)` con `mut` contextual; ahorra una cabeza pero crea un átomo con significado posicional |
| 3 | `requires`/`ensures` vacíos | siempre presentes, pueden ir vacíos | omitirlos cuando están vacíos: menos ruido, más ramas de aridad |
| 4 | `(case Err)` sin datos | `(fields ...)` obligatorio, `(case Err (fields))` | `(case Err)` de aridad 2, con aridad variable |
| 5 | `OperarIO` | cuatro cabezas (`io-read`, `io-write`, `io-send`, `io-recv`) de aridad fija | `(io read ...)` con sub-operador; una cabeza, aridad variable por sub-operador |
| 6 | Tipos | `i64`, `f64` (como el ejemplo) + español para el resto (`texto`, `booleano`, `capacidad`) | todo inglés (`int`, `bool`, `string`) o todo español (`Entero64`, …) |
| 7 | `verdadero`/`falso` | átomos reservados en posición de enlace | literales con forma propia (`(true)`) o prefijo (`#t`) |
| 8 | Comentarios | solo `;` | aceptar `//` como alias de `;` para no romper ficheros CNL existentes |
| 9 | Operadores | símbolo S-exp (`+`, `==`) | nombre verbal (`add`, `eq`); más legible para `!=`/`>=`, más ruido |
| 10 | Acceso a propiedad | `(get base prop)` | `(. base prop)` o mantener `base.prop`, que reintroduce el punto en el léxico |
| 11 | `capability` | tres hijos nombrados (`resource`, `port`, `bandwidth`) | posición pura: `(capability NOMBRE "recurso" 443 "10Gbps")` |
| 12 | Restricciones | `(restrictions (clave valor) ...)` anidado dentro del `body` | lista plana `(restrictions clave valor clave valor)`; menos paréntesis, aridad par obligatoria |
| 13 | Escapes desconocidos | se conservan literales (`\q` → `\q`) | error `escape_invalido`; rompe rutas Windows |
| 14 | `spawn` de tarea inexistente | aviso, no error | error duro; el backend actual no lo exige |
| 15 | Unicidad de nombres | `nombre_duplicado` post-lectura | permitir y resolver en el checker |

Riesgo conocido que este documento **no** resuelve: el lexer actual acepta
`"C:\temp"` y lo convierte en `C:<TAB>emp` porque `\t` es escape válido. La
SPEC v2 conserva ese comportamiento (§1.4) para no cambiar el significado de
ningún literal existente; corregirlo exige una revisión de todos los textos del
repositorio y es una decisión de lenguaje, no del reader.

---

## Apéndice A — Gramática completa en EBNF

```ebnf
Documento      ::= { Forma } ;

Forma          ::= Task | Struct | Variant | Service | Model | Memory
                 | Capability | Module | Migration ;

Task           ::= "(" "task" Ident Params [ Returns ] Requires Ensures Body ")" ;
Params         ::= "(" "params" { "(" Ident Tipo ")" } ")" ;
Returns        ::= "(" "returns" Tipo ")" ;
Requires       ::= { "(" "requires" Expr ")" } ;
Ensures        ::= { "(" "ensures" Expr ")" } ;

Struct         ::= "(" "struct" Ident Fields ")" ;
Model          ::= "(" "model" Ident Fields ")" ;
Memory         ::= "(" "memory" Ident Fields ")" ;
Fields         ::= "(" "fields" { "(" Ident Tipo ")" } ")" ;

Variant        ::= "(" "variant" Ident { Case } ")" ;
Case           ::= "(" "case" Ident Fields ")" ;

Service        ::= "(" "service" Ident [ Net ] { Handler } { Task }
                    [ Restrictions ] ")" ;
Net            ::= "(" "net" Entero [ "(" "protocol" Ident ")" ] ")" ;
Handler        ::= "(" "handler" Ident Params Body ")" ;

Capability     ::= "(" "capability" Ident
                     "(" "resource" Texto ")"
                     "(" "port" Entero ")"
                     "(" "bandwidth" Texto ")" ")" ;

Module         ::= "(" "module" Ident { Export } ")" ;
Export         ::= "(" "export" Clase Ident ")" ;
Clase          ::= "service" | "task" | "struct" | "variant" | "capability" ;

Migration      ::= "(" "migration" Ident
                     "(" "from" Ident ")"
                     "(" "to" Ident ")"
                     "(" "field" Texto ")"
                     "(" "record" Ident ")"
                     "(" "default" Expr ")"
                     [ Restrictions ] ")" ;

Body           ::= "(" "body" { Stmt } [ Restrictions ] ")" ;
Restrictions   ::= "(" "restrictions" { Restriccion } ")" ;
Restriccion    ::= "(" Atomo Atomo ")" ;

Stmt           ::= If | Assert | Assume | Prove | Invariant | While | For
                 | Let | LetMut | Return | Request | Validate | Send | Recv
                 | Spawn | Chan | IoRead | IoWrite | IoSend | IoRecv ;

If             ::= "(" "if" Expr Body [ "(" "else" Body ")" ] ")" ;
Assert         ::= "(" "assert" Expr ")" ;
Assume         ::= "(" "assume" Expr ")" ;
Prove          ::= "(" "prove" Expr ")" ;
Invariant      ::= "(" "invariant" Expr ")" ;
While          ::= "(" "while" Expr Body ")" ;
For            ::= "(" "for" Ident Expr Body ")" ;
Let            ::= "(" "let" Ident [ Tipo ] Expr ")" ;
LetMut         ::= "(" "let-mut" Ident [ Tipo ] Expr ")" ;
Return         ::= "(" "return" [ Expr ] ")" ;
Request        ::= "(" "request" Permiso Texto
                     [ "(" "ttl" Entero ")" ] [ "(" "as" Ident ")" ] ")" ;
Validate       ::= "(" "validate" Ident ")" ;
Send           ::= "(" "send" Expr Ident ")" ;
Recv           ::= "(" "recv" Ident Ident ")" ;
Spawn          ::= "(" "spawn" Ident ")" ;
Chan           ::= "(" "chan" Ident [ "(" "capacity" Entero ")" ] ")" ;
IoRead         ::= "(" "io-read" Texto Ident Ident ")" ;
IoWrite        ::= "(" "io-write" Expr Texto Ident ")" ;
IoSend         ::= "(" "io-send" Expr Texto Ident ")" ;
IoRecv         ::= "(" "io-recv" Texto Ident Ident ")" ;

Permiso        ::= "lectura_archivo" | "escritura_archivo" | "red" | "ejecucion" ;

Expr           ::= Atomo | Numero | Texto | "verdadero" | "falso"
                 | "(" "get" Expr Ident ")"
                 | "(" OpBin Expr Expr ")" ;
OpBin          ::= "+" | "-" | "*" | "/" | ">" | "<" | "==" | "!=" | ">=" | "<=" ;

Tipo           ::= "i64" | "f64" | "booleano" | "texto" | "capacidad" | "void"
                 | "USD" | "EUR"
                 | "(" "list" Tipo ")"
                 | "(" "map" Tipo Tipo ")"
                 | "(" "set" Tipo ")"
                 | "(" "tuple" { Tipo } ")"
                 | Ident ;

Ident          ::= [A-Za-z_] [A-Za-z0-9_]* ;
Entero         ::= [0-9]+ ;
Texto          ::= '"' { ? cualquier carácter salvo '"' y NL, o escape ? } '"' ;
```
