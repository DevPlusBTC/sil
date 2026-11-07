#ifndef SIL_ARENA_NVM_H
#define SIL_ARENA_NVM_H

#include <stdint.h>
#include <stddef.h>

#define SIL_ARENA_NVM_MAGIC  0x53494C41U  /* "SILA" */
#define SIL_ARENA_NVM_VERSION 1U
#define SIL_ARENA_NVM_HMAC_SIZE 32U

typedef struct {
    uint32_t magic;          /* debe ser SIL_ARENA_NVM_MAGIC */
    uint32_t version;        /* debe ser SIL_ARENA_NVM_VERSION */
    uint64_t crc64;          /* CRC64-ECMA del contenido sin hmac */
    uint8_t  hmac[32];       /* HMAC-SHA256 sobre header + contenido sin hmac */
    uint32_t bloque_count;   /* numero de bloques persistidos */
    size_t  total_size;      /* tamaño total asignado (bytes)      */
    uint64_t timestamp;      /* epoch unix segundos       */
    uint32_t ciclo_contador; /* ultimo id de ciclo usado       */
    uint32_t ciclos_abiertos;/* numero de ciclos abiertos persistidos  */
    uint8_t  _reserved[40];  /* reserva para extensiones futuras   */
} SilArenaNvmHeader;

typedef struct {
    size_t  capacidad;   /* tamaño total asignado al bloque    */
    size_t  usado;         /* bytes usados dentro del bloque   */
    uint64_t crc64;          /* CRC64 del contenido de memoria   */
    uint8_t  hmac[32];       /* HMAC-SHA256 del contenido           */
} SilArenaBloqueNvm;

typedef struct {
    uint32_t id;            /* id monotónico de ciclo */
    size_t usado;           /* bytes asignados dentro del ciclo */
    size_t bytes;           /* bytes totales asignados dentro */
    uint32_t bloque_index;  /* indice en el array de bloques persistido  */
    size_t bloque_usado;    /* usado del bloque al iniciar ciclo   */
} SilArenaCicloNvm;

int sil_arena_persistir_nvm  (const SilArena *arena, const char *path);
int sil_arena_recuperar_nvm  (SilArena *arena, const char *path);
int sil_arena_verificar_nvm  (const char *path);
int sil_arena_info_nvm       (const char *path, int *version, size_t *total_size, uint32_t *bloque_count);

#endif
