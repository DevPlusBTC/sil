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
    /* M0: copia (Fase 1: transferencia zero-copy de bloques). */
    (void)origen;
    void *nuevo = sil_arena_asignar(destino, tamanio, 8);
    memcpy(nuevo, ptr, tamanio);
}
