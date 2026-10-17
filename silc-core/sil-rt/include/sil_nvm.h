#ifndef SIL_NVM_H
#define SIL_NVM_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "sil_arena.h"

/* NVM (Non-Volatile Memory) Persistent Arenas
 * 
 * Características:
 * - mmap-based persistent memory mapping
 * - Lazy migration de arenas volátiles a persistentes
 * - Checkpointing automático con versionado
 * - Recuperación tras crash (replay de WAL)
 * - Compatibilidad PMEM (Intel Optane) y archivos regulares
 */

typedef enum {
    SIL_NVM_OK = 0,
    SIL_NVM_ERROR = -1,
    SIL_NVM_NO_MEM = -2,
    SIL_NVM_CORRUPTO = -3,
    SIL_NVM_VERSION = -4,
    SIL_NVM_PERMISOS = -5,
} SilNvmError;

typedef struct SilNvmArena SilNvmArena;
typedef struct SilNvmConfig SilNvmConfig;

/* Configuración de arena NVM */
struct SilNvmConfig {
    const char *ruta_archivo;     // Ruta del archivo persistente
    size_t tamano_inicial;        // Tamaño inicial en bytes
    size_t tamano_maximo;         // Tamaño máximo (0 = sin límite)
    uint32_t flags;               // Flags de comportamiento
    uint32_t version_esquema;     // Versión del esquema de datos
};

/* Flags para SilNvmConfig */
#define SIL_NVM_FLAG_CREAR        0x0001  // Crear si no existe
#define SIL_NVM_FLAG_SOLO_LECTURA 0x0002  // Abrir solo lectura
#define SIL_NVM_FLAG_SYNC         0x0004  // fsync en cada checkpoint
#define SIL_NVM_FLAG_PMEM         0x0008  // Usar PMEM directo (si disponible)
#define SIL_NVM_FLAG_WAL          0x0010  // Write-ahead logging

/* Crear/abrir arena NVM persistente */
SilNvmArena* sil_nvm_arena_crear(const SilNvmConfig *config, SilNvmError *error);

/* Abrir arena NVM existente (solo lectura) */
SilNvmArena* sil_nvm_arena_abrir(const char *ruta, SilNvmError *error);

/* Cerrar arena NVM (flush automático) */
void sil_nvm_arena_cerrar(SilNvmArena *arena);

/* Asignar memoria persistente */
void* sil_nvm_asignar(SilNvmArena *arena, size_t tamanio, size_t alineacion);

/* Liberar memoria persistente (no-op en arena bump, usar reset) */
void sil_nvm_liberar(SilNvmArena *arena, void *ptr, size_t tamanio);

/* Checkpoint: persistir cambios a disco */
SilNvmError sil_nvm_checkpoint(SilNvmArena *arena);

/* Reset arena al último checkpoint */
void sil_nvm_reset(SilNvmArena *arena);

/* Migrar arena volátil a persistente (lazy migration) */
SilNvmError sil_nvm_migrar_desde_volatile(SilNvmArena *arena, SilArena *volatile_arena);

/* Obtener puntero base de la arena */
void* sil_nvm_base(SilNvmArena *arena);

/* Obtener tamaño usado */
size_t sil_nvm_usado(SilNvmArena *arena);

/* Obtener tamaño total mapeado */
size_t sil_nvm_tamano(SilNvmArena *arena);

/* Verificar integridad (checksums) */
bool sil_nvm_verificar_integridad(SilNvmArena *arena);

/* Recuperación tras crash: replay WAL */
SilNvmError sil_nvm_recuperar(SilNvmArena *arena);

/* Forzar sync a disco */
SilNvmError sil_nvm_sync(SilNvmArena *arena);

#endif // SIL_NVM_H