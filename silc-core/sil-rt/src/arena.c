/* sil_arena.c - Subsistema Arenas Causales O(1). M0: implementación funcional básica. */
#include "sil_arena.h"
#include <stdlib.h>
#include <string.h>

SilArena sil_arena_crear(size_t capacidad_inicial) {
    SilArena arena;
    size_t cap = capacidad_inicial > 0 ? capacidad_inicial : SIL_ARENA_PAGE_SIZE;
    SilArenaBloque *b = (SilArenaBloque*)malloc(sizeof(SilArenaBloque));
    b->memoria = (uint8_t*)malloc(cap);
    b->capacidad = cap;
    b->usado = 0;
    b->siguiente = NULL;
    arena.inicio = b;
    arena.actual = b;
    arena.asignado_total = cap;
    arena.ciclo_actual = 0;
    arena.ciclo_contador = 0;
    arena.ciclos_abiertos = 0;
    memset(arena.ciclos, 0, sizeof(arena.ciclos));
    return arena;
}

void* sil_arena_asignar(SilArena *arena, size_t tamanio, size_t alineacion) {
    if (alineacion == 0) alineacion = 8;
    size_t alineado = (tamanio + (alineacion - 1)) & ~(alineacion - 1);
    SilArenaBloque *b = arena->actual;
    if (b->usado + alineado > b->capacidad) {
        size_t nueva = alineado > SIL_ARENA_PAGE_SIZE ? alineado : SIL_ARENA_PAGE_SIZE;
        SilArenaBloque *n = (SilArenaBloque*)malloc(sizeof(SilArenaBloque));
        n->memoria = (uint8_t*)malloc(nueva);
        n->capacidad = nueva;
        n->usado = 0;
        n->siguiente = NULL;
        b->siguiente = n;
        arena->actual = n;
        arena->asignado_total += nueva;
        b = n;
    }
    void *ptr = &b->memoria[b->usado];
    b->usado += alineado;
    /* Contabilidad del ciclo abierto mas interno. */
    if (arena->ciclos_abiertos > 0) {
        arena->ciclos[arena->ciclos_abiertos - 1].bytes += alineado;
    }
    return ptr;
}

void sil_arena_destruir(SilArena *arena) {
    SilArenaBloque *c = arena->inicio;
    while (c) { SilArenaBloque *s = c->siguiente; free(c->memoria); free(c); c = s; }
    arena->inicio = NULL; arena->actual = NULL; arena->asignado_total = 0;
    arena->ciclo_actual = 0;
    arena->ciclos_abiertos = 0;
    memset(arena->ciclos, 0, sizeof(arena->ciclos));
}

void sil_arena_reset(SilArena *arena) {
    /* O(1): conserva bloques, resetea offsets. */
    for (SilArenaBloque *b = arena->inicio; b; b = b->siguiente) b->usado = 0;
    arena->actual = arena->inicio;
    arena->ciclo_actual = 0;
    arena->ciclos_abiertos = 0;
    memset(arena->ciclos, 0, sizeof(arena->ciclos));
}

void sil_arena_promover(SilArena *origen, SilArena *destino, void *ptr, size_t tamanio) {
    /* M1: zero-copy block transfer (Fase 1: reencadenamiento de bloques). */
    (void)ptr;
    (void)tamanio;
    /* Transferir todos los bloques de origen a destino enlazando listas. */
    if (origen->inicio) {
        if (destino->actual) {
            destino->actual->siguiente = origen->inicio;
        } else {
            destino->inicio = origen->inicio;
        }
        destino->actual = origen->actual;
        destino->asignado_total += origen->asignado_total;
    }
    /* Reset origen a estado vacío. */
    origen->inicio = NULL;
    origen->actual = NULL;
    origen->asignado_total = 0;
    origen->ciclo_actual = 0;
    origen->ciclos_abiertos = 0;
    memset(origen->ciclos, 0, sizeof(origen->ciclos));
}

/* Funciones de callbak para integración con Rust */
typedef void (*sil_arena_assign_fn)(SilArena *arena, size_t tamanio, size_t alineacion);
typedef void (*sil_arena_reset_fn)(SilArena *arena);
typedef void (*sil_arena_promote_fn)(SilArena *origen, SilArena *destino, void *ptr, size_t tamanio);

void sil_arena_register_callbacks(sil_arena_assign_fn assign_fn, sil_arena_reset_fn reset_fn, sil_arena_promote_fn promote_fn) {
    (void)assign_fn;
    (void)reset_fn;
    (void)promote_fn;
}

void sil_arena_migrar_cero_costo(SilArena *origen, SilArena *destino) {
    /* M1: migración zero-cost de esquemas (Fase 1: transferencia de bloques). */
    /* Transferir todos los bloques del origen al destino sin copiar elementos. */
    sil_arena_promover(origen, destino, NULL, 0);
}

/* ===========================================================================
 * Sub-arenas ciclicas
 *
 * Un ciclo marca la posicion actual (bloque + offset). Al cerrarlo se libera
 * todo lo asignado desde la marca: O(1) en bytes, O(n bloques) en el número de
 * bloques creados dentro del ciclo. Se anidan hasta SIL_ARENA_MAX_CICLOS.
 * =========================================================================*/

uint32_t sil_arena_ciclo_iniciar(SilArena *arena) {
    if (!arena || arena->ciclos_abiertos >= SIL_ARENA_MAX_CICLOS) return 0;
    SilArenaCiclo *c = &arena->ciclos[arena->ciclos_abiertos];
    c->bloque = arena->actual;
    c->usado = arena->actual ? arena->actual->usado : 0;
    c->id = ++arena->ciclo_contador;
    c->bytes = 0;
    arena->ciclos_abiertos++;
    arena->ciclo_actual = c->id;
    return c->id;
}

void sil_arena_ciclo_finalizar(SilArena *arena) {
    if (!arena || arena->ciclos_abiertos == 0) return;
    SilArenaCiclo *c = &arena->ciclos[arena->ciclos_abiertos - 1];
    if (c->bloque) {
        SilArenaBloque *b = c->bloque->siguiente;
        while (b != NULL) {
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
    }
    arena->ciclos_abiertos--;
    arena->ciclo_actual = arena->ciclos_abiertos > 0
                              ? arena->ciclos[arena->ciclos_abiertos - 1].id
                              : 0;
}

uint32_t sil_arena_ciclo_actual(SilArena *arena) {
    return arena ? arena->ciclo_actual : 0;
}

size_t sil_arena_ciclo_bytes(SilArena *arena, uint32_t ciclo) {
    if (!arena) return 0;
    for (uint32_t i = 0; i < arena->ciclos_abiertos; i++) {
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
