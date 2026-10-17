#ifndef SIL_EBPF_H
#define SIL_EBPF_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

/* eBPF Kernel Integration
 * 
 * Características:
 * - Cargar/desprogramar programas eBPF
 * - Maps eBPF (hash, array, perf event, ringbuf)
 * - Attach a tracepoints, kprobes, uprobes, XDP, TC
 * - Comunicación userspace-kernel via ringbuf/perf_event
 * - Verificación automática de programas
 * 
 * Linux: libbpf + kernel 5.10+
 * Windows: eBPF for Windows (experimental)
 */

typedef enum {
    SIL_EBPF_OK = 0,
    SIL_EBPF_ERROR = -1,
    SIL_EBPF_NO_MEM = -2,
    SIL_EBPF_VERIFICACION = -3,
    SIL_EBPF_CARGA = -4,
    SIL_EBPF_ADJUNTO = -5,
    SIL_EBPF_MAPA = -6,
    SIL_EBPF_PERMISOS = -7,
    SIL_EBPF_NO_SOPORTADO = -8,
} SilEbpfError;

typedef enum {
    SIL_EBPF_PROG_SOCKET_FILTER = 0,
    SIL_EBPF_PROG_KPROBE = 1,
    SIL_EBPF_PROG_UPROBE = 2,
    SIL_EBPF_PROG_TRACEPOINT = 3,
    SIL_EBPF_PROG_XDP = 4,
    SIL_EBPF_PROG_TC = 5,
    SIL_EBPF_PROG_CGROUP_SKB = 6,
    SIL_EBPF_PROG_LSM = 7,
    SIL_EBPF_PROG_STRUCT_OPS = 8,
} SilEbpfProgType;

typedef enum {
    SIL_EBPF_MAP_HASH = 0,
    SIL_EBPF_MAP_ARRAY = 1,
    SIL_EBPF_MAP_PROG_ARRAY = 2,
    SIL_EBPF_MAP_PERF_EVENT_ARRAY = 3,
    SIL_EBPF_MAP_RINGBUF = 4,
    SIL_EBPF_MAP_STACK_TRACE = 5,
    SIL_EBPF_MAP_LRU_HASH = 6,
    SIL_EBPF_MAP_LRU_PERCPU_HASH = 7,
} SilEbpfMapType;

typedef struct SilEbpfProgram SilEbpfProgram;
typedef struct SilEbpfMap SilEbpfMap;
typedef struct SilEbpfLink SilEbpfLink;
typedef struct SilEbpfContext SilEbpfContext;

/* Configuración de programa eBPF */
typedef struct {
    const char *nombre;
    SilEbpfProgType tipo;
    const void *bytecode;      // Bytecode eBPF (ELF)
    size_t bytecode_len;
    const char *license;       // Licencia (GPL, MIT, etc.)
    uint32_t kern_version;     // Versión kernel objetivo
} SilEbpfProgramConfig;

/* Configuración de mapa eBPF */
typedef struct {
    const char *nombre;
    SilEbpfMapType tipo;
    uint32_t key_size;
    uint32_t value_size;
    uint32_t max_entries;
    uint32_t flags;
} SilEbpfMapConfig;

/* Crear contexto eBPF */
SilEbpfContext* sil_ebpf_ctx_crear(void);

/* Destruir contexto */
void sil_ebpf_ctx_destruir(SilEbpfContext *ctx);

/* Cargar programa eBPF desde bytecode ELF */
SilEbpfProgram* sil_ebpf_programa_cargar(SilEbpfContext *ctx, const SilEbpfProgramConfig *config, SilEbpfError *error);

/* Desprogramar programa */
void sil_ebpf_programa_destruir(SilEbpfProgram *prog);

/* Adjuntar programa a punto de anclaje */
SilEbpfLink* sil_ebpf_adjuntar(SilEbpfProgram *prog, const char *punto_anclaje, SilEbpfError *error);

/* Desadjuntar programa */
void sil_ebpf_desadjuntar(SilEbpfLink *link);

/* Crear mapa eBPF */
SilEbpfMap* sil_ebpf_mapa_crear(SilEbpfContext *ctx, const SilEbpfMapConfig *config, SilEbpfError *error);

/* Destruir mapa */
void sil_ebpf_mapa_destruir(SilEbpfMap *mapa);

/* Operaciones de mapa */
SilEbpfError sil_ebpf_mapa_actualizar(SilEbpfMap *mapa, const void *key, const void *value, uint64_t flags);
SilEbpfError sil_ebpf_mapa_buscar(SilEbpfMap *mapa, const void *key, void *value);
SilEbpfError sil_ebpf_mapa_eliminar(SilEbpfMap *mapa, const void *key);
SilEbpfError sil_ebpf_mapa_iterar(SilEbpfMap *mapa, void *key, void *value, bool *mas);

/* Obtener FD de mapa (para usar en bytecode) */
int sil_ebpf_mapa_fd(SilEbpfMap *mapa);

/* Poll/leer eventos de ringbuf/perf_event */
int sil_ebpf_eventos_leer(SilEbpfMap *mapa, void *buffer, size_t tamano, int timeout_ms);

/* Verificar soporte eBPF en sistema */
bool sil_ebpf_soportado(void);

/* Obtener versión kernel eBPF */
uint32_t sil_ebpf_version_kernel(void);

#endif // SIL_EBPF_H