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
    return ptr;
}

void sil_arena_destruir(SilArena *arena) {
    SilArenaBloque *c = arena->inicio;
    while (c) { SilArenaBloque *s = c->siguiente; free(c->memoria); free(c); c = s; }
    arena->inicio = NULL; arena->actual = NULL; arena->asignado_total = 0;
}

void sil_arena_reset(SilArena *arena) {
    /* O(1): conserva bloques, resetea offsets. */
    for (SilArenaBloque *b = arena->inicio; b; b = b->siguiente) b->usado = 0;
    arena->actual = arena->inicio;
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
