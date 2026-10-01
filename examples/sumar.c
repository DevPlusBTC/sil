#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

/* --- SIL Runtime: Arenas O(1) (embebido, autocontenido) --- */
typedef struct SilArenaBloque_ {
    unsigned char *memoria;
    unsigned long capacidad;
    unsigned long usado;
    struct SilArenaBloque_ *siguiente;
} SilArenaBloque_;

typedef struct {
    SilArenaBloque_ *inicio;
    SilArenaBloque_ *actual;
    unsigned long asignado_total;
} SilArena_;

#define SIL_ARENA_PAGE_ 65536UL

static SilArena_ sil_arena_crear_(unsigned long cap_inicial) {
    SilArena_ a;
    unsigned long cap = cap_inicial > 0 ? cap_inicial : SIL_ARENA_PAGE_;
    SilArenaBloque_ *b = (SilArenaBloque_*)malloc(sizeof(SilArenaBloque_));
    b->memoria = (unsigned char*)malloc(cap);
    b->capacidad = cap;
    b->usado = 0;
    b->siguiente = 0;
    a.inicio = b;
    a.actual = b;
    a.asignado_total = cap;
    return a;
}

static void *sil_arena_asignar_(SilArena_ *arena, unsigned long n, unsigned long al) {
    unsigned long a = (n + (al - 1)) & ~(al - 1);
    SilArenaBloque_ *b = arena->actual;
    if (b->usado + a > b->capacidad) {
        unsigned long nc = a > SIL_ARENA_PAGE_ ? a : SIL_ARENA_PAGE_;
        SilArenaBloque_ *nn = (SilArenaBloque_*)malloc(sizeof(SilArenaBloque_));
        nn->memoria = (unsigned char*)malloc(nc);
        nn->capacidad = nc;
        nn->usado = 0;
        nn->siguiente = 0;
        b->siguiente = nn;
        arena->actual = nn;
        arena->asignado_total += nc;
        b = nn;
    }
    void *p = &b->memoria[b->usado];
    b->usado += a;
    return p;
}

static void sil_arena_destruir_(SilArena_ *arena) {
    SilArenaBloque_ *c = arena->inicio;
    while (c) { SilArenaBloque_ *s = c->siguiente; free(c->memoria); free(c); c = s; }
    arena->inicio = 0; arena->actual = 0; arena->asignado_total = 0;
}

/* --- SIL Tipos --- */
typedef struct { const char *ptr; long len; } SilTexto_;
typedef struct { unsigned long long id; unsigned int permisos; unsigned long long expiracion_ns; unsigned char firma[32]; int valida; } SilCap_;

/* Tarea SIL: sumar */
long long sumar(SilArena_ *__arena, long long x, long long y) {
(void)__arena;
    long long v2;
    v2 = x + y; /* checked: SMT */
    return v2;
 }
