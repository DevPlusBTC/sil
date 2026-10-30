#ifndef SIL_ARENA_H
#define SIL_ARENA_H

#include <stdint.h>
#include <stddef.h>

#define SIL_ARENA_PAGE_SIZE 65536
/* Pila máxima de ciclos anidados por arena (sub-arenas cíclicas). */
#define SIL_ARENA_MAX_CICLOS 16

typedef struct SilArenaBloque {
    uint8_t *memoria;
    size_t capacidad;
    size_t usado;
    struct SilArenaBloque *siguiente;
} SilArenaBloque;

/* Marca de un ciclo abierto: bloque y offset vigentes al abrirlo, más los
 * bytes asignados dentro del ciclo. Finalizar restaura la marca en O(1)
 * respecto al tamaño y O(n bloques) respecto a los bloques añadidos. */
typedef struct SilArenaCiclo {
    SilArenaBloque *bloque;
    size_t usado;
    uint32_t id;
    size_t bytes;
} SilArenaCiclo;

typedef struct {
    SilArenaBloque *inicio;
    SilArenaBloque *actual;
    size_t asignado_total;
    uint32_t ciclo_actual;    /* id del ciclo abierto más interno (0 = ninguno) */
    uint32_t ciclo_contador;  /* ids monótonos, nunca se reutilizan */
    SilArenaCiclo ciclos[SIL_ARENA_MAX_CICLOS];
    uint32_t ciclos_abiertos;
} SilArena;

/* Crea una arena con al menos `capacidad_inicial` bytes (0 = pagina de 64 KiB).
 * Nunca falla: si no hay memoria devuelve una arena vacia y la primera
 * asignacion reintenta crear el bloque. */
SilArena sil_arena_crear(size_t capacidad_inicial);

/* Reserva `tamanio` bytes alineados a `alineacion`.
 *
 * Contrato:
 *  - `alineacion` debe ser potencia de dos (0 = 8). Si no lo es, devuelve NULL.
 *  - Devuelve un puntero alineado de verdad: la alineacion se aplica sobre la
 *    direccion, no sobre el tamanio.
 *  - Si `tamanio + alineacion` desbordaria size_t, o si no hay memoria,
 *    devuelve NULL sin modificar la arena.
 *  - `tamanio == 0` reserva 1 byte (nunca se devuelve el mismo puntero dos
 *    veces).
 *  - Crea un bloque nuevo de forma perezosa si la arena esta vacia. */
void* sil_arena_asignar(SilArena *arena, size_t tamanio, size_t alineacion);

/* Libera todos los bloques. La arena queda reutilizable (creacion perezosa). */
void sil_arena_destruir(SilArena *arena);

/* Reinicia los offsets conservando las paginas: O(n bloques) con n acotado. */
void sil_arena_reset(SilArena *arena);

/* Transfiere TODOS los bloques del origen al destino sin copiar datos.
 * `ptr`/`tamanio` se mantienen por compatibilidad ABI (la promocion es a nivel
 * de arena). No hace nada si `origen == destino` (evita ciclos en la lista). */
void sil_arena_promover(SilArena *origen, SilArena *destino, void *ptr, size_t tamanio);

/* Sub-arenas ciclicas: `iniciar` abre un ciclo (id > 0, 0 = desbordamiento) y
 * `finalizar` reclama en O(1) en bytes todo lo asignado desde la marca. */
uint32_t sil_arena_ciclo_iniciar(SilArena *arena);
void sil_arena_ciclo_finalizar(SilArena *arena);
uint32_t sil_arena_ciclo_actual(SilArena *arena);
size_t sil_arena_ciclo_bytes(SilArena *arena, uint32_t ciclo);
void sil_arena_ciclo_estadisticas(SilArena *arena, uint32_t *ciclos_activos, size_t *bytes_por_ciclo);

#endif
