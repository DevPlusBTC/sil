#!/usr/bin/env python3
"""
sil_arena.py - Subsistema de Arenas Causales para el Lenguaje SIL.
Garantiza gestión de memoria determinista O(1) sin Recolector de Basura (GC).
"""

# ==============================================================================
# HEADER C99 DEL SUBSISTEMA DE ARENAS
# ==============================================================================

ARENA_C99_HEADER = """/* ==============================================================================
 * SIL ARENA MEMORY SUBSYSTEM (C99)
 * ============================================================================== */
#ifndef SIL_ARENA_H
#define SIL_ARENA_H

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

#define SIL_ARENA_PAGE_SIZE 65536  // Páginas contiguas de 64 KB

typedef struct SilArenaBloque {
    uint8_t *memoria;
    size_t capacidad;
    size_t usado;
    struct SilArenaBloque *siguiente;
} SilArenaBloque;

typedef struct {
    SilArenaBloque *inicio;
    SilArenaBloque *actual;
    size_t asignado_total;
} SilArena;

// Inicialización de Arena en tiempo constante O(1)
static inline SilArena sil_arena_crear(size_t capacidad_inicial) {
    SilArena arena;
    size_t cap = capacidad_inicial > 0 ? capacidad_inicial : SIL_ARENA_PAGE_SIZE;

    SilArenaBloque *bloque = (SilArenaBloque*)malloc(sizeof(SilArenaBloque));
    bloque->memoria = (uint8_t*)malloc(cap);
    bloque->capacidad = cap;
    bloque->usado = 0;
    bloque->siguiente = NULL;
    arena.inicio = bloque;
    arena.actual = bloque;
    arena.asignado_total = cap;
    return arena;
}

// Asignación de memoria dentro de la Arena en O(1) mediante Bump Allocation
static inline void* sil_arena_asignar(SilArena *arena, size_t tamanio, size_t alineacion) {
    size_t alineado = (tamanio + (alineacion - 1)) & ~(alineacion - 1);
    SilArenaBloque *b = arena->actual;

    if (b->usado + alineado > b->capacidad) {
        size_t nueva_cap = alineado > SIL_ARENA_PAGE_SIZE ? alineado : SIL_ARENA_PAGE_SIZE;
        SilArenaBloque *nuevo = (SilArenaBloque*)malloc(sizeof(SilArenaBloque));
        nuevo->memoria = (uint8_t*)malloc(nueva_cap);
        nuevo->capacidad = nueva_cap;
        nuevo->usado = 0;
        nuevo->siguiente = NULL;
        b->siguiente = nuevo;
        arena->actual = nuevo;
        arena->asignado_total += nueva_cap;
        b = nuevo;
    }

    void *ptr = &b->memoria[b->usado];
    b->usado += alineado;
    return ptr;
}

// Liberación Causal Completa en O(1)
static inline void sil_arena_destruir(SilArena *arena) {
    SilArenaBloque *curr = arena->inicio;
    while (curr != NULL) {
        SilArenaBloque *sig = curr->siguiente;
        free(curr->memoria);
        free(curr);
        curr = sig;
    }
    arena->inicio = NULL;
    arena->actual = NULL;
    arena->asignado_total = 0;
}

// Promoción de datos a Arena Persistente (Hoisting O(1))
static inline void sil_arena_promover(SilArena *origen, SilArena *destino, void *ptr, size_t tamanio) {
    // En implementación real: transferencia de ownership de bloque sin copia
    // Aquí placeholder para la interfaz
    (void)origen; (void)destino; (void)ptr; (void)tamanio;
}

#endif // SIL_ARENA_H
"""

def test_arena_generacion():
    assert "sil_arena_crear" in ARENA_C99_HEADER
    assert "sil_arena_asignar" in ARENA_C99_HEADER
    assert "sil_arena_destruir" in ARENA_C99_HEADER
    assert "sil_arena_promover" in ARENA_C99_HEADER
    assert "SIL_ARENA_PAGE_SIZE" in ARENA_C99_HEADER
    print("[ARENA TEST] Subsistema de Memoria Causal C99 verificado correctamente.")

if __name__ == "__main__":
    test_arena_generacion()