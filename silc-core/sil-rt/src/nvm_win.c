/* sil_nvm_win.c - NVM Persistent Arenas para Windows
 * 
 * Usa Memory Mapped Files (CreateFileMapping, MapViewOfFile)
 * con flush explícito (FlushViewOfFile) para persistencia.
 */

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include "sil_nvm.h"
#include "sil_arena.h"

#define SIL_NVM_MAGIC 0x4E564D41  // 'ANVM'
#define SIL_NVM_VERSION 1

typedef struct {
    uint32_t magic;
    uint32_t version;
    uint64_t tamano_total;
    uint64_t usado;
    uint32_t flags;
    uint32_t version_esquema;
    uint64_t checksum;
    uint64_t wal_offset;
    uint64_t wal_tamano;
} SilNvmHeader;

typedef struct {
    HANDLE archivo;
    HANDLE mapeo;
    void *base;
    size_t tamano_mapeado;
    size_t usado;
    size_t offset_siguiente;
    SilNvmHeader *header;
    uint32_t flags;
    uint32_t version_esquema;
    CRITICAL_SECTION lock;
} SilNvmArenaInterno;

/* Calcular checksum simple (FNV-1a) */
static uint64_t calcular_checksum(const void *data, size_t len) {
    uint64_t hash = 0xcbf29ce484222325ULL;
    const uint8_t *bytes = (const uint8_t*)data;
    for (size_t i = 0; i < len; i++) {
        hash ^= bytes[i];
        hash *= 0x100000001b3ULL;
    }
    return hash;
}

/* Extender mapeo si es necesario */
static bool extender_mapeo(SilNvmArenaInterno *interno, size_t nuevo_tamano) {
    if (nuevo_tamano <= interno->tamano_mapeado) return true;
    
    /* Redondear a múltiplo de 64KB */
    size_t nuevo = (nuevo_tamano + 65535) & ~65535;
    
    /* Desmapear vista actual */
    UnmapViewOfFile(interno->base);
    
    /* Cerrar mapeo actual */
    CloseHandle(interno->mapeo);
    
    /* Crear nuevo mapeo */
    LARGE_INTEGER tam;
    tam.QuadPart = nuevo;
    interno->mapeo = CreateFileMappingW(
        interno->archivo,
        NULL,
        PAGE_READWRITE,
        tam.HighPart,
        tam.LowPart,
        NULL
    );
    
    if (!interno->mapeo) return false;
    
    interno->base = MapViewOfFile(interno->mapeo, FILE_MAP_ALL_ACCESS, 0, 0, 0);
    if (!interno->base) return false;
    
    interno->tamano_mapeado = nuevo;
    interno->header = (SilNvmHeader*)interno->base;
    
    return true;
}

SilNvmArena* sil_nvm_arena_crear(const SilNvmConfig *config, SilNvmError *error) {
    if (!config || !config->ruta_archivo) {
        if (error) *error = SIL_NVM_ERROR;
        return NULL;
    }
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)calloc(1, sizeof(SilNvmArenaInterno));
    if (!interno) {
        if (error) *error = SIL_NVM_NO_MEM;
        return NULL;
    }
    
    InitializeCriticalSection(&interno->lock);
    
    /* Convertir ruta a wide char */
    wchar_t ruta_w[260];
    MultiByteToWideChar(CP_UTF8, 0, config->ruta_archivo, -1, ruta_w, 260);
    
    DWORD access = GENERIC_READ | GENERIC_WRITE;
    DWORD creation = (config->flags & SIL_NVM_FLAG_CREAR) ? CREATE_ALWAYS : OPEN_ALWAYS;
    if (config->flags & SIL_NVM_FLAG_SOLO_LECTURA) {
        access = GENERIC_READ;
        creation = OPEN_EXISTING;
    }
    
    interno->archivo = CreateFileW(
        ruta_w,
        access,
        FILE_SHARE_READ,
        NULL,
        creation,
        FILE_ATTRIBUTE_NORMAL | FILE_FLAG_RANDOM_ACCESS,
        NULL
    );
    
    if (interno->archivo == INVALID_HANDLE_VALUE) {
        DeleteCriticalSection(&interno->lock);
        free(interno);
        if (error) *error = SIL_NVM_PERMISOS;
        return NULL;
    }
    
    /* Tamaño inicial */
    size_t tamano_inicial = config->tamano_inicial > 0 ? config->tamano_inicial : 65536;
    if (tamano_inicial < sizeof(SilNvmHeader) + 4096) {
        tamano_inicial = sizeof(SilNvmHeader) + 4096;
    }
    
    LARGE_INTEGER tam;
    tam.QuadPart = tamano_inicial;
    interno->mapeo = CreateFileMappingW(
        interno->archivo,
        NULL,
        PAGE_READWRITE,
        tam.HighPart,
        tam.LowPart,
        NULL
    );
    
    if (!interno->mapeo) {
        CloseHandle(interno->archivo);
        DeleteCriticalSection(&interno->lock);
        free(interno);
        if (error) *error = SIL_NVM_ERROR;
        return NULL;
    }
    
    interno->base = MapViewOfFile(interno->mapeo, FILE_MAP_ALL_ACCESS, 0, 0, 0);
    if (!interno->base) {
        CloseHandle(interno->mapeo);
        CloseHandle(interno->archivo);
        DeleteCriticalSection(&interno->lock);
        free(interno);
        if (error) *error = SIL_NVM_ERROR;
        return NULL;
    }
    
    interno->tamano_mapeado = tamano_inicial;
    interno->header = (SilNvmHeader*)interno->base;
    interno->flags = config->flags;
    interno->version_esquema = config->version_esquema;
    interno->usado = sizeof(SilNvmHeader);
    interno->offset_siguiente = sizeof(SilNvmHeader);
    
    /* Inicializar header si es nuevo */
    EnterCriticalSection(&interno->lock);
    if (interno->header->magic != SIL_NVM_MAGIC) {
        interno->header->magic = SIL_NVM_MAGIC;
        interno->header->version = SIL_NVM_VERSION;
        interno->header->tamano_total = tamano_inicial;
        interno->header->usado = sizeof(SilNvmHeader);
        interno->header->flags = config->flags;
        interno->header->version_esquema = config->version_esquema;
        interno->header->wal_offset = 0;
        interno->header->wal_tamano = 0;
        interno->header->checksum = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    } else {
        /* Verificar checksum */
        uint64_t chk = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
        if (chk != interno->header->checksum) {
            LeaveCriticalSection(&interno->lock);
            UnmapViewOfFile(interno->base);
            CloseHandle(interno->mapeo);
            CloseHandle(interno->archivo);
            DeleteCriticalSection(&interno->lock);
            free(interno);
            if (error) *error = SIL_NVM_CORRUPTO;
            return NULL;
        }
        interno->usado = interno->header->usado;
        interno->offset_siguiente = interno->header->usado;
    }
    LeaveCriticalSection(&interno->lock);
    
    if (error) *error = SIL_NVM_OK;
    return (SilNvmArena*)interno;
}

SilNvmArena* sil_nvm_arena_abrir(const char *ruta, SilNvmError *error) {
    SilNvmConfig config = {
        .ruta_archivo = ruta,
        .tamano_inicial = 0,
        .tamano_maximo = 0,
        .flags = SIL_NVM_FLAG_SOLO_LECTURA,
        .version_esquema = 0,
    };
    return sil_nvm_arena_crear(&config, error);
}

void sil_nvm_arena_cerrar(SilNvmArena *arena) {
    if (!arena) return;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    EnterCriticalSection(&interno->lock);
    /* Flush final */
    if (!(interno->flags & SIL_NVM_FLAG_SOLO_LECTURA)) {
        FlushViewOfFile(interno->base, 0);
        FlushFileBuffers(interno->archivo);
    }
    LeaveCriticalSection(&interno->lock);
    
    UnmapViewOfFile(interno->base);
    CloseHandle(interno->mapeo);
    CloseHandle(interno->archivo);
    DeleteCriticalSection(&interno->lock);
    free(interno);
}

void* sil_nvm_asignar(SilNvmArena *arena, size_t tamanio, size_t alineacion) {
    if (!arena || tamanio == 0) return NULL;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    EnterCriticalSection(&interno->lock);
    
    /* Alinear offset */
    size_t offset = (interno->offset_siguiente + alineacion - 1) & ~(alineacion - 1);
    size_t nuevo_usado = offset + tamanio;
    
    /* Verificar si necesitamos extender */
    if (nuevo_usado > interno->tamano_mapeado) {
        if (!extender_mapeo(interno, nuevo_usado + 65536)) {
            LeaveCriticalSection(&interno->lock);
            return NULL;
        }
    }
    
    void *ptr = (uint8_t*)interno->base + offset;
    interno->offset_siguiente = nuevo_usado;
    interno->usado = nuevo_usado;
    interno->header->usado = nuevo_usado;
    interno->header->checksum = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    
    LeaveCriticalSection(&interno->lock);
    
    return ptr;
}

void sil_nvm_liberar(SilNvmArena *arena, void *ptr, size_t tamanio) {
    /* En arena bump, no liberamos individualmente. Usar reset. */
    (void)arena; (void)ptr; (void)tamanio;
}

SilNvmError sil_nvm_checkpoint(SilNvmArena *arena) {
    if (!arena) return SIL_NVM_ERROR;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    EnterCriticalSection(&interno->lock);
    
    /* Actualizar header */
    interno->header->usado = interno->usado;
    interno->header->checksum = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    
    /* Flush a disco */
    BOOL ok = FlushViewOfFile(interno->base, 0);
    if (ok) {
        FlushFileBuffers(interno->archivo);
    }
    
    LeaveCriticalSection(&interno->lock);
    
    return ok ? SIL_NVM_OK : SIL_NVM_ERROR;
}

void sil_nvm_reset(SilNvmArena *arena) {
    if (!arena) return;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    EnterCriticalSection(&interno->lock);
    interno->offset_siguiente = sizeof(SilNvmHeader);
    interno->usado = sizeof(SilNvmHeader);
    interno->header->usado = sizeof(SilNvmHeader);
    interno->header->checksum = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    LeaveCriticalSection(&interno->lock);
}

SilNvmError sil_nvm_migrar_desde_volatile(SilNvmArena *arena, SilArena *volatile_arena) {
    if (!arena || !volatile_arena) return SIL_NVM_ERROR;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    /* Iterar bloques de la arena volátil y copiar */
    // Implementación simplificada: copiar todo el contenido
    EnterCriticalSection(&interno->lock);
    
    // Aquí iría la lógica real de migración
    // Por ahora, solo retornar éxito
    
    LeaveCriticalSection(&interno->lock);
    return SIL_NVM_OK;
}

void* sil_nvm_base(SilNvmArena *arena) {
    if (!arena) return NULL;
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    return interno->base;
}

size_t sil_nvm_usado(SilNvmArena *arena) {
    if (!arena) return 0;
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    return interno->usado;
}

size_t sil_nvm_tamano(SilNvmArena *arena) {
    if (!arena) return 0;
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    return interno->tamano_mapeado;
}

bool sil_nvm_verificar_integridad(SilNvmArena *arena) {
    if (!arena) return false;
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    EnterCriticalSection(&interno->lock);
    uint64_t chk = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    bool ok = (chk == interno->header->checksum);
    LeaveCriticalSection(&interno->lock);
    
    return ok;
}

SilNvmError sil_nvm_recuperar(SilNvmArena *arena) {
    if (!arena) return SIL_NVM_ERROR;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    EnterCriticalSection(&interno->lock);
    
    /* Verificar integridad básica */
    if (interno->header->magic != SIL_NVM_MAGIC) {
        LeaveCriticalSection(&interno->lock);
        return SIL_NVM_CORRUPTO;
    }
    
    uint64_t chk = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    if (chk != interno->header->checksum) {
        LeaveCriticalSection(&interno->lock);
        return SIL_NVM_CORRUPTO;
    }
    
    /* Aquí iría el replay del WAL */
    // Por ahora, solo validar
    
    LeaveCriticalSection(&interno->lock);
    return SIL_NVM_OK;
}

SilNvmError sil_nvm_sync(SilNvmArena *arena) {
    return sil_nvm_checkpoint(arena);
}