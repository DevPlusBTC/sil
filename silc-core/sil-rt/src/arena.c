/* sil_arena.c - Subsistema de Arenas Causales O(1) del runtime nativo SIL.
 *
 * Invariantes que este fichero garantiza (verificadas en sil-rt/tests):
 *  1. Toda asignacion devuelve un puntero alineado a `alineacion` o NULL.
 *  2. Ninguna peticion (tamanio/alineacion arbitrarios) desborda un bloque ni
 *     provoca comportamiento indefinido: si no se puede servir, se devuelve
 *     NULL sin tocar la arena.
 *  3. `sil_arena_asignar` nunca asume bloques preexistentes: si la arena esta
 *     vacia (recien creada sin memoria, o tras `sil_arena_destruir`) crea el
 *     bloque perezosamente.
 *  4. Los ciclos son O(1) en bytes (restaurar el offset de la marca) y O(n) en
 *     los bloques creados dentro del ciclo (se liberan).
 *  5. `sil_arena_promover` nunca forma ciclos en la lista de bloques.
 */

#include "sil_arena.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* Alineacion por defecto si el llamador pasa 0 (alineacion natural de
 * max_align_t en las plataformas soportadas). */
#define SIL_ARENA_ALINEACION_DEFECTO 8u

static int es_potencia_de_dos(size_t v) { return v != 0 && (v & (v - 1)) == 0; }

/* Redondea `v` hacia arriba al multiplo de `a` (potencia de dos).
 * Devuelve 0 si la operacion desbordaria uintptr_t. */
static int alinear_arriba(uintptr_t v, size_t a, uintptr_t *salida) {
    uintptr_t resto = v & (uintptr_t)(a - 1);
    if (resto == 0) {
        *salida = v;
        return 1;
    }
    {
        uintptr_t delta = (uintptr_t)a - resto;
        if (v > UINTPTR_MAX - delta) return 0;
        *salida = v + delta;
    }
    return 1;
}

static SilArenaBloque *bloque_nuevo(size_t capacidad) {
    SilArenaBloque *b = (SilArenaBloque *)calloc(1, sizeof(SilArenaBloque));
    if (!b) return NULL;
    b->memoria = (uint8_t *)malloc(capacidad);
    if (!b->memoria) {
        free(b);
        return NULL;
    }
    b->capacidad = capacidad;
    b->usado = 0;
    b->siguiente = NULL;
    return b;
}

static void bloques_liberar(SilArenaBloque *primero) {
    SilArenaBloque *c = primero;
    while (c) {
        SilArenaBloque *siguiente = c->siguiente;
        free(c->memoria);
        free(c);
        c = siguiente;
    }
}

SilArena sil_arena_crear(size_t capacidad_inicial) {
    SilArena arena;
    size_t cap = capacidad_inicial > 0 ? capacidad_inicial : (size_t)SIL_ARENA_PAGE_SIZE;
    SilArenaBloque *b = bloque_nuevo(cap);

    memset(&arena, 0, sizeof(arena));
    if (!b) {
        /* Arena valida pero sin memoria: la primera asignacion reintenta. */
        return arena;
    }
    arena.inicio = b;
    arena.actual = b;
    arena.asignado_total = cap;
    return arena;
}

void *sil_arena_asignar(SilArena *arena, size_t tamanio, size_t alineacion) {
    if (!arena) return NULL;
    if (alineacion == 0) alineacion = SIL_ARENA_ALINEACION_DEFECTO;
    if (!es_potencia_de_dos(alineacion)) return NULL;
    /* Una peticion de 0 bytes devuelve un puntero unico: no se puede
     * "asignar nada" y devolver el mismo puntero dos veces. */
    if (tamanio == 0) tamanio = 1;
    /* Evita el desbordamiento de `tamanio + alineacion` al dimensionar. */
    if (tamanio > SIZE_MAX - alineacion) return NULL;

    for (;;) {
        SilArenaBloque *b = arena->actual;
        uintptr_t base, fin_bloque, inicio;

        if (!b) {
            size_t cap = tamanio + alineacion;
            if (cap < (size_t)SIL_ARENA_PAGE_SIZE) cap = (size_t)SIL_ARENA_PAGE_SIZE;
            b = bloque_nuevo(cap);
            if (!b) return NULL;
            arena->inicio = b;
            arena->actual = b;
            arena->asignado_total += cap;
        }

        base = (uintptr_t)b->memoria;
        if (b->capacidad > UINTPTR_MAX - base) return NULL;
        fin_bloque = base + b->capacidad;

        if (!alinear_arriba(base + b->usado, alineacion, &inicio)) return NULL;
        if (inicio >= base + b->usado && inicio <= fin_bloque &&
            tamanio <= (size_t)(fin_bloque - inicio)) {
            size_t usado_anterior = b->usado;
            size_t desplazamiento = (size_t)(inicio - base);
            b->usado = desplazamiento + tamanio;
            /* Contabilidad del ciclo abierto mas interno. */
            if (arena->ciclos_abiertos > 0) {
                arena->ciclos[arena->ciclos_abiertos - 1].bytes += b->usado - usado_anterior;
            }
            return (void *)inicio;
        }

        /* No cabe: encadenar un bloque nuevo dimensionado a la peticion. */
        {
            size_t cap = tamanio + alineacion;
            SilArenaBloque *n;
            if (cap < (size_t)SIL_ARENA_PAGE_SIZE) cap = (size_t)SIL_ARENA_PAGE_SIZE;
            n = bloque_nuevo(cap);
            if (!n) return NULL;
            b->siguiente = n;
            arena->actual = n;
            arena->asignado_total += cap;
        }
    }
}

void sil_arena_destruir(SilArena *arena) {
    if (!arena) return;
    bloques_liberar(arena->inicio);
    arena->inicio = NULL;
    arena->actual = NULL;
    arena->asignado_total = 0;
    arena->ciclo_actual = 0;
    arena->ciclos_abiertos = 0;
    memset(arena->ciclos, 0, sizeof(arena->ciclos));
}

void sil_arena_reset(SilArena *arena) {
    /* O(n bloques) con n acotado por los bloques vivos: se conservan las
     * paginas (no se devuelven al SO) y se reinician los offsets. */
    SilArenaBloque *b;
    if (!arena) return;
    for (b = arena->inicio; b; b = b->siguiente) b->usado = 0;
    arena->actual = arena->inicio;
    arena->ciclo_actual = 0;
    arena->ciclos_abiertos = 0;
    memset(arena->ciclos, 0, sizeof(arena->ciclos));
}

void sil_arena_promover(SilArena *origen, SilArena *destino, void *ptr, size_t tamanio) {
    /* Transferencia zero-copy de los bloques del origen al destino.
     * `ptr`/`tamanio` se conservan en la firma por compatibilidad ABI: la
     * promocion es a nivel de arena (todos los bloques), no de objeto. */
    (void)ptr;
    (void)tamanio;
    if (!origen || !destino || origen == destino) return; /* nunca auto-enlazar */
    if (!origen->inicio) return;

    /* Los ciclos abiertos del origen dejarian marcas apuntando a bloques que
     * ya no le pertenecen: se descartan (la memoria vive en el destino). */
    origen->ciclo_actual = 0;
    origen->ciclos_abiertos = 0;
    memset(origen->ciclos, 0, sizeof(origen->ciclos));

    if (destino->actual) {
        destino->actual->siguiente = origen->inicio;
    } else {
        destino->inicio = origen->inicio;
    }
    destino->actual = origen->actual;
    destino->asignado_total += origen->asignado_total;

    origen->inicio = NULL;
    origen->actual = NULL;
    origen->asignado_total = 0;
}

/* Migracion zero-cost: alias documentado de la promocion de bloques. */
void sil_arena_migrar_cero_costo(SilArena *origen, SilArena *destino) {
    sil_arena_promover(origen, destino, NULL, 0);
}

/* ===========================================================================
 * Sub-arenas ciclicas
 *
 * Un ciclo marca la posicion actual (bloque + offset). Al cerrarlo se libera
 * todo lo asignado desde la marca: O(1) en bytes, O(n bloques) en el numero de
 * bloques creados dentro del ciclo. Se anidan hasta SIL_ARENA_MAX_CICLOS.
 * =========================================================================*/

uint32_t sil_arena_ciclo_iniciar(SilArena *arena) {
    SilArenaCiclo *c;
    if (!arena || arena->ciclos_abiertos >= SIL_ARENA_MAX_CICLOS) return 0;
    c = &arena->ciclos[arena->ciclos_abiertos];
    c->bloque = arena->actual;
    c->usado = arena->actual ? arena->actual->usado : 0;
    c->id = ++arena->ciclo_contador;
    c->bytes = 0;
    arena->ciclos_abiertos++;
    arena->ciclo_actual = c->id;
    return c->id;
}

void sil_arena_ciclo_finalizar(SilArena *arena) {
    SilArenaCiclo *c;
    if (!arena || arena->ciclos_abiertos == 0) return;
    c = &arena->ciclos[arena->ciclos_abiertos - 1];

    if (c->bloque) {
        SilArenaBloque *b = c->bloque->siguiente;
        while (b) {
            SilArenaBloque *siguiente = b->siguiente;
            if (arena->asignado_total >= b->capacidad) {
                arena->asignado_total -= b->capacidad;
            } else {
                arena->asignado_total = 0;
            }
            free(b->memoria);
            free(b);
            b = siguiente;
        }
        c->bloque->siguiente = NULL;
        c->bloque->usado = c->usado;
        arena->actual = c->bloque;
    } else {
        /* El ciclo se abrio con la arena vacia: todo lo creado dentro le
         * pertenece y se libera. */
        bloques_liberar(arena->inicio);
        arena->inicio = NULL;
        arena->actual = NULL;
        arena->asignado_total = 0;
    }

    memset(c, 0, sizeof(*c));
    arena->ciclos_abiertos--;
    arena->ciclo_actual = arena->ciclos_abiertos > 0
                              ? arena->ciclos[arena->ciclos_abiertos - 1].id
                              : 0;
}

uint32_t sil_arena_ciclo_actual(SilArena *arena) {
    return arena ? arena->ciclo_actual : 0;
}

size_t sil_arena_ciclo_bytes(SilArena *arena, uint32_t ciclo) {
    uint32_t i;
    if (!arena || ciclo == 0) return 0;
    for (i = 0; i < arena->ciclos_abiertos; i++) {
        if (arena->ciclos[i].id == ciclo) return arena->ciclos[i].bytes;
    }
    return 0;
}

void sil_arena_ciclo_estadisticas(SilArena *arena, uint32_t *ciclos_abiertos,
                                  size_t *bytes_ciclo_actual) {
    if (ciclos_abiertos) *ciclos_abiertos = arena ? arena->ciclos_abiertos : 0;
    if (bytes_ciclo_actual) {
        *bytes_ciclo_actual = (arena && arena->ciclos_abiertos > 0)
                                  ? arena->ciclos[arena->ciclos_abiertos - 1].bytes
                                  : 0;
    }
}
