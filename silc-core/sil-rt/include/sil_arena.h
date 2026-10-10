#ifndef SIL_ARENA_H
#define SIL_ARENA_H

#include <stdint.h>
#include <stddef.h>

#define SIL_ARENA_PAGE_SIZE 65536

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
    uint32_t ciclo_actual;
    uint32_t ciclo_contador;
} SilArena;

SilArena sil_arena_crear(size_t capacidad_inicial);
void* sil_arena_asignar(SilArena *arena, size_t tamanio, size_t alineacion);
void sil_arena_destruir(SilArena *arena);
void sil_arena_reset(SilArena *arena);
void sil_arena_promover(SilArena *origen, SilArena *destino, void *ptr, size_t tamanio);

uint32_t sil_arena_ciclo_iniciar(SilArena *arena);
void sil_arena_ciclo_finalizar(SilArena *arena);
uint32_t sil_arena_ciclo_actual(SilArena *arena);
size_t sil_arena_ciclo_bytes(SilArena *arena, uint32_t ciclo);
void sil_arena_ciclo_estadisticas(SilArena *arena, uint32_t *ciclos_activos, size_t *bytes_por_ciclo);

#endif
