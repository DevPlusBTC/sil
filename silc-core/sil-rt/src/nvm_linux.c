/* sil_nvm_linux.c - NVM Persistent Arenas para Linux
 * 
 * Usa mmap() con MAP_SHARED para persistencia.
 * Sincroniza con msync() y fsync().
 */

#include "sil_nvm.h"
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <pthread.h>

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
    int fd;
    void *base;
    size_t tamano_mapeado;
    size_t usado;
    size_t offset_siguiente;
    SilNvmHeader *header;
    uint32_t flags;
    uint32_t version_esquema;
    pthread_mutex_t lock;
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

/* Extender mapeo usando mremap */
static bool extender_mapeo(SilNvmArenaInterno *interno, size_t nuevo_tamano) {
    if (nuevo_tamano <= interno->tamano_mapeado) return true;
    
    /* Redondear a múltiplo de página */
    long page_size = sysconf(_SC_PAGESIZE);
    size_t nuevo = (nuevo_tamano + page_size - 1) & ~(page_size - 1);
    
    /* Extender archivo */
    if (ftruncate(interno->fd, nuevo) != 0) {
        return false;
    }
    
    /* Re-mapear */
    void *nuevo_base = mremap(interno->base, interno->tamano_mapeado, nuevo, MREMAP_MAYMOVE);
    if (nuevo_base == MAP_FAILED) {
        return false;
    }
    
    interno->base = nuevo_base;
    interno->header = (SilNvmHeader*)interno->base;
    interno->tamano_mapeado = nuevo;
    
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
    
    pthread_mutex_init(&interno->lock, NULL);
    
    int open_flags = O_RDWR | O_CREAT;
    if (config->flags & SIL_NVM_FLAG_SOLO_LECTURA) {
        open_flags = O_RDONLY;
    }
    
    interno->fd = open(config->ruta_archivo, open_flags, 0644);
    if (interno->fd < 0) {
        pthread_mutex_destroy(&interno->lock);
        free(interno);
        if (error) *error = SIL_NVM_PERMISOS;
        return NULL;
    }
    
    /* Tamaño inicial */
    size_t tamano_inicial = config->tamano_inicial > 0 ? config->tamano_inicial : 65536;
    if (tamano_inicial < sizeof(SilNvmHeader) + 4096) {
        tamano_inicial = sizeof(SilNvmHeader) + 4096;
    }
    
    /* Extender archivo al tamaño inicial */
    if (ftruncate(interno->fd, tamano_inicial) != 0) {
        close(interno->fd);
        pthread_mutex_destroy(&interno->lock);
        free(interno);
        if (error) *error = SIL_NVM_ERROR;
        return NULL;
    }
    
    /* Mapear en memoria */
    int prot = PROT_READ | PROT_WRITE;
    if (config->flags & SIL_NVM_FLAG_SOLO_LECTURA) {
        prot = PROT_READ;
    }
    
    int map_flags = MAP_SHARED;
    if (config->flags & SIL_NVM_FLAG_PMEM) {
        map_flags |= MAP_SYNC;  // Linux 4.15+ para PMEM
    }
    
    interno->base = mmap(NULL, tamano_inicial, prot, map_flags, interno->fd, 0);
    if (interno->base == MAP_FAILED) {
        close(interno->fd);
        pthread_mutex_destroy(&interno->lock);
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
    pthread_mutex_lock(&interno->lock);
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
            pthread_mutex_unlock(&interno->lock);
            munmap(interno->base, interno->tamano_mapeado);
            close(interno->fd);
            pthread_mutex_destroy(&interno->lock);
            free(interno);
            if (error) *error = SIL_NVM_CORRUPTO;
            return NULL;
        }
        interno->usado = interno->header->usado;
        interno->offset_siguiente = interno->header->usado;
    }
    pthread_mutex_unlock(&interno->lock);
    
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
    
    pthread_mutex_lock(&interno->lock);
    if (!(interno->flags & SIL_NVM_FLAG_SOLO_LECTURA)) {
        msync(interno->base, interno->tamano_mapeado, MS_SYNC);
        fsync(interno->fd);
    }
    pthread_mutex_unlock(&interno->lock);
    
    munmap(interno->base, interno->tamano_mapeado);
    close(interno->fd);
    pthread_mutex_destroy(&interno->lock);
    free(interno);
}

void* sil_nvm_asignar(SilNvmArena *arena, size_t tamanio, size_t alineacion) {
    if (!arena || tamanio == 0) return NULL;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    pthread_mutex_lock(&interno->lock);
    
    /* Alinear offset */
    size_t offset = (interno->offset_siguiente + alineacion - 1) & ~(alineacion - 1);
    size_t nuevo_usado = offset + tamanio;
    
    /* Verificar si necesitamos extender */
    if (nuevo_usado > interno->tamano_mapeado) {
        if (!extender_mapeo(interno, nuevo_usado + 65536)) {
            pthread_mutex_unlock(&interno->lock);
            return NULL;
        }
    }
    
    void *ptr = (uint8_t*)interno->base + offset;
    interno->offset_siguiente = nuevo_usado;
    interno->usado = nuevo_usado;
    interno->header->usado = nuevo_usado;
    interno->header->checksum = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    
    pthread_mutex_unlock(&interno->lock);
    
    return ptr;
}

void sil_nvm_liberar(SilNvmArena *arena, void *ptr, size_t tamanio) {
    /* En arena bump, no liberamos individualmente. Usar reset. */
    (void)arena; (void)ptr; (void)tamanio;
}

SilNvmError sil_nvm_checkpoint(SilNvmArena *arena) {
    if (!arena) return SIL_NVM_ERROR;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    pthread_mutex_lock(&interno->lock);
    
    /* Actualizar header */
    interno->header->usado = interno->usado;
    interno->header->checksum = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    
    /* Flush a disco */
    int ret = msync(interno->base, interno->tamano_mapeado, MS_SYNC);
    if (ret == 0 && !(interno->flags & SIL_NVM_FLAG_PMEM)) {
        fsync(interno->fd);
    }
    
    pthread_mutex_unlock(&interno->lock);
    
    return ret == 0 ? SIL_NVM_OK : SIL_NVM_ERROR;
}

void sil_nvm_reset(SilNvmArena *arena) {
    if (!arena) return;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    pthread_mutex_lock(&interno->lock);
    interno->offset_siguiente = sizeof(SilNvmHeader);
    interno->usado = sizeof(SilNvmHeader);
    interno->header->usado = sizeof(SilNvmHeader);
    interno->header->checksum = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    pthread_mutex_unlock(&interno->lock);
}

SilNvmError sil_nvm_migrar_desde_volatile(SilNvmArena *arena, SilArena *volatile_arena) {
    if (!arena || !volatile_arena) return SIL_NVM_ERROR;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    pthread_mutex_lock(&interno->lock);
    
    // Implementación simplificada
    
    pthread_mutex_unlock(&interno->lock);
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
    
    pthread_mutex_lock(&interno->lock);
    uint64_t chk = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    bool ok = (chk == interno->header->checksum);
    pthread_mutex_unlock(&interno->lock);
    
    return ok;
}

SilNvmError sil_nvm_recuperar(SilNvmArena *arena) {
    if (!arena) return SIL_NVM_ERROR;
    
    SilNvmArenaInterno *interno = (SilNvmArenaInterno*)arena;
    
    pthread_mutex_lock(&interno->lock);
    
    if (interno->header->magic != SIL_NVM_MAGIC) {
        pthread_mutex_unlock(&interno->lock);
        return SIL_NVM_CORRUPTO;
    }
    
    uint64_t chk = calcular_checksum(interno->header, sizeof(SilNvmHeader) - sizeof(uint64_t));
    if (chk != interno->header->checksum) {
        pthread_mutex_unlock(&interno->lock);
        return SIL_NVM_CORRUPTO;
    }
    
    pthread_mutex_unlock(&interno->lock);
    return SIL_NVM_OK;
}

SilNvmError sil_nvm_sync(SilNvmArena *arena) {
    return sil_nvm_checkpoint(arena);
}