#ifndef SIL_ARENA_H
#define SIL_ARENA_H

#include <stdint.h>
#include <stddef.h>

#define SIL_ARENA_PAGE_SIZE 65536
/* Pila máxima de ciclos anidados por arena (sub-arenas cíclicas). */
#define SIL_ARENA_MAX_CICLOS 16

/* Magic number para validación de archivos NVM persistidos */
#define SIL_ARENA_NVM_MAGIC 0x53494C41  /* "SILA" */
#define SIL_ARENA_NVM_VERSION 1
#define SIL_ARENA_NVM_HMAC_SIZE 32  /* HMAC-SHA256 */

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

/* ============================================================================
 * Persistencia NVM (Non-Volatile Memory)
 * ============================================================================
 *
 * Formato de archivo:
 * 1. Header (ArenaNvmHeader) - verifica integridad con CRC64 + HMAC-SHA256
 * 2. Bloques consecutivos - cada uno con CRC64 individual
 *
 * Formato de escritura: archivo temporal + fsync + rename atómico
 * Formato de lectura: validación completa (magic + CRC64 + HMAC + versión)
 */

/* Header de archivo NVM persistido */
typedef struct {
    uint32_t magic;           /* SIL_ARENA_NVM_MAGIC */
    uint32_t version;         /* SIL_ARENA_NVM_VERSION */
    uint64_t crc64;           /* CRC64-ECMA del contenido (bloques + ciclos) */
    uint8_t hmac[32];         /* HMAC-SHA256 del contenido + header sin hmac */
    uint32_t bloque_count;    /* Número de bloques persistidos */
    size_t total_size;        /* Tamaño total asignado */
    uint64_t timestamp;       /* Timestamp Unix para detectar stale */
    uint32_t ciclo_contador;  /* Último ID de ciclo usado */
    uint32_t ciclos_abiertos; /* Número de ciclos abiertos persistidos */
    uint8_t _reserved[40];    /* Reserva para futuras extensiones */
} SilArenaNvmHeader;

/* Bloque persistido en disco */
typedef struct {
    size_t capacidad;
    size_t usado;
    uint64_t crc64;           /* CRC64 del contenido de memoria */
    uint8_t hmac[32];         /* HMAC-SHA256 del contenido + header */
} SilArenaBloqueNvm;

/* Ciclo persistido en disco */
typedef struct {
    uint32_t id;
    size_t usado;
    size_t bytes;
    uint32_t bloque_index;    /* Índice del bloque en el array persistido */
    size_t bloque_usado;      /* used del bloque al iniciar ciclo */
} SilArenaCicloNvm;

/* ============================================================================
 * Funciones de persistencia NVM
 * ============================================================================
 *
 * sil_arena_persistir:
 *   Guarda el estado completo de la arena a un archivo NVM.
 *   - Atomicidad: escribe a archivo temporal + fsync + rename atómico
 *   - Integridad: CRC64 + HMAC-SHA256 en header y cada bloque
 *   - Concurrencia: file locking + double-check
 *
 * sil_arena_recuperar:
 *   Recupera una arena desde archivo NVM.
 *   - Validación completa: magic + CRC64 + HMAC + versión + timestamp
 *   - Reconstrucción exacta de bloques, ciclos y contadores
 *
 * sil_arena_destruir_persistida:
 *   Elimina archivo persistido + lock file
 */

int sil_arena_persistir(const SilArena *arena, const char *path);
int sil_arena_recuperar(SilArena *arena, const char *path);
int sil_arena_destruir_persistida(const char *path);

/* Función auxiliar: verifica si un archivo NVM es válido sin recuperarlo */
int sil_arena_verificar_nvm(const char *path);

/* Función auxiliar: obtiene información del archivo NVM sin recuperarlo */
typedef struct {
    uint32_t version;
    size_t total_size;
    uint32_t bloque_count;
    uint32_t ciclo_contador;
    uint64_t timestamp;
} SilArenaNvmInfo;

int sil_arena_info_nvm(const char *path, SilArenaNvmInfo *info);

#endif
