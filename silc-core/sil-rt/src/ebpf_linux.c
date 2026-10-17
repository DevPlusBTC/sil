/* sil_ebpf_linux.c - eBPF para Linux usando libbpf
 * 
 * Requiere: libbpf-dev, kernel 5.10+, headers kernel
 * Compilar con: -lbpf -lelf -lz
 */

#include "sil_ebpf.h"
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <stdint.h>
#include <errno.h>

#if defined(__linux__)
#include <unistd.h>
#include <sys/resource.h>
#include <sys/utsname.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <bpf/libbpf.h>
#include <bpf/bpf.h>

typedef struct {
 struct bpf_object *obj;
 struct bpf_program *prog;
 struct bpf_link *link;
} SilEbpfProgramInterno;

typedef struct {
 struct bpf_map *map;
 int fd;
} SilEbpfMapInterno;

typedef struct {
 struct bpf_link *link;
} SilEbpfLinkInterno;

typedef struct {
 int dummy;
} SilEbpfContextInterno;

static bool aumentar_limite_memlock(void) {
 struct rlimit rlim = {RLIM_INFINITY, RLIM_INFINITY};
 return setrlimit(RLIMIT_MEMLOCK, &rlim) == 0;
}

SilEbpfContext* sil_ebpf_ctx_crear(void) {
 aumentar_limite_memlock();
 SilEbpfContextInterno *interno = (SilEbpfContextInterno*)calloc(1, sizeof(SilEbpfContextInterno));
 if (!interno) return NULL;
 libbpf_set_strict_mode(LIBBPF_STRICT_ALL);
 libbpf_set_print(NULL);
 return (SilEbpfContext*)interno;
}

void sil_ebpf_ctx_destruir(SilEbpfContext *ctx) {
 if (!ctx) return;
 free(ctx);
}

SilEbpfProgram* sil_ebpf_programa_cargar(SilEbpfContext *ctx, const SilEbpfProgramConfig *config, SilEbpfError *error) {
 if (!ctx || !config || !config->bytecode || config->bytecode_len == 0) {
 if (error) *error = SIL_EBPF_ERROR;
 return NULL;
 }
 SilEbpfProgramInterno *interno = (SilEbpfProgramInterno*)calloc(1, sizeof(SilEbpfProgramInterno));
 if (!interno) {
 if (error) *error = SIL_EBPF_NO_MEM;
 return NULL;
 }
 struct bpf_object_open_opts opts = { .sz = sizeof(opts), .object_name = config->nombre };
 interno->obj = bpf_object__open_mem(config->bytecode, config->bytecode_len, &opts);
 if (libbpf_get_error(interno->obj)) { free(interno); if (error) *error = SIL_EBPF_CARGA; return NULL; }
 struct bpf_program *prog = NULL;
 bpf_object__for_each_program(prog, interno->obj) {
 enum bpf_prog_type tipo = bpf_program__get_type(prog);
 if ((int)tipo == config->tipo) { interno->prog = prog; break; }
 }
 if (!interno->prog) { bpf_object__close(interno->obj); free(interno); if (error) *error = SIL_EBPF_CARGA; return NULL; }
 if (config->license) bpf_program__set_license(interno->prog, config->license);
 int err = bpf_object__load(interno->obj);
 if (err) { bpf_object__close(interno->obj); free(interno); if (error) *error = SIL_EBPF_VERIFICACION; return NULL; }
 if (error) *error = SIL_EBPF_OK;
 return (SilEbpfProgram*)interno;
}

void sil_ebpf_programa_destruir(SilEbpfProgram *prog) {
 if (!prog) return;
 SilEbpfProgramInterno *interno = (SilEbpfProgramInterno*)prog;
 if (interno->link) bpf_link__destroy(interno->link);
 if (interno->obj) bpf_object__close(interno->obj);
 free(interno);
}

SilEbpfLink* sil_ebpf_adjuntar(SilEbpfProgram *prog, const char *punto_anclaje, SilEbpfError *error) {
 if (!prog || !punto_anclaje) { if (error) *error = SIL_EBPF_ERROR; return NULL; }
 SilEbpfProgramInterno *interno = (SilEbpfProgramInterno*)prog;
 struct bpf_link *link = NULL;
 if (strncmp(punto_anclaje, "tracepoint/", 11) == 0) {
 char *categoria = strdup(punto_anclaje + 11);
 char *nombre = strchr(categoria, '/');
 if (nombre) { *nombre++ = '\0'; link = bpf_program__attach_tracepoint(interno->prog, categoria, nombre); }
 free(categoria);
 } else if (strncmp(punto_anclaje, "kprobe/", 7) == 0) {
 link = bpf_program__attach_kprobe(interno->prog, false, punto_anclaje + 7);
 } else if (strncmp(punto_anclaje, "kretprobe/", 10) == 0) {
 link = bpf_program__attach_kprobe(interno->prog, true, punto_anclaje + 10);
 } else if (strncmp(punto_anclaje, "uprobe:", 7) == 0) {
 char *bin_func = strdup(punto_anclaje + 7);
 char *func = strchr(bin_func, ':');
 if (func) { *func++ = '\0'; link = bpf_program__attach_uprobe(interno->prog, -1, bin_func, func); }
 free(bin_func);
 } else if (strncmp(punto_anclaje, "xdp/", 4) == 0) {
 int ifindex = atoi(punto_anclaje + 4); link = bpf_program__attach_xdp(interno->prog, ifindex);
 } else if (strncmp(punto_anclaje, "tc/", 3) == 0) {
 int ifindex = atoi(punto_anclaje + 3); link = bpf_program__attach_tc(interno->prog, ifindex);
 }
 if (!link || libbpf_get_error(link)) { if (error) *error = SIL_EBPF_ADJUNTO; return NULL; }
 SilEbpfLinkInterno *link_interno = (SilEbpfLinkInterno*)malloc(sizeof(SilEbpfLinkInterno));
 if (!link_interno) { bpf_link__destroy(link); if (error) *error = SIL_EBPF_NO_MEM; return NULL; }
 link_interno->link = link;
 if (error) *error = SIL_EBPF_OK;
 return (SilEbpfLink*)link_interno;
}

void sil_ebpf_desadjuntar(SilEbpfLink *link) { if (!link) return; SilEbpfLinkInterno *interno = (SilEbpfLinkInterno*)link; if (interno->link) bpf_link__destroy(interno->link); free(interno); }

SilEbpfMap* sil_ebpf_mapa_crear(SilEbpfContext *ctx, const SilEbpfMapConfig *config, SilEbpfError *error) {
 if (!ctx || !config) { if (error) *error = SIL_EBPF_ERROR; return NULL; }
 SilEbpfMapInterno *interno = (SilEbpfMapInterno*)calloc(1, sizeof(SilEbpfMapInterno));
 if (!interno) { if (error) *error = SIL_EBPF_NO_MEM; return NULL; }
 enum bpf_map_type tipo;
 switch (config->tipo) {
 case SIL_EBPF_MAP_HASH: tipo = BPF_MAP_TYPE_HASH; break;
 case SIL_EBPF_MAP_ARRAY: tipo = BPF_MAP_TYPE_ARRAY; break;
 case SIL_EBPF_MAP_PROG_ARRAY: tipo = BPF_MAP_TYPE_PROG_ARRAY; break;
 case SIL_EBPF_MAP_PERF_EVENT_ARRAY: tipo = BPF_MAP_TYPE_PERF_EVENT_ARRAY; break;
 case SIL_EBPF_MAP_RINGBUF: tipo = BPF_MAP_TYPE_RINGBUF; break;
 case SIL_EBPF_MAP_STACK_TRACE: tipo = BPF_MAP_TYPE_STACK_TRACE; break;
 case SIL_EBPF_MAP_LRU_HASH: tipo = BPF_MAP_TYPE_LRU_HASH; break;
 case SIL_EBPF_MAP_LRU_PERCPU_HASH: tipo = BPF_MAP_TYPE_LRU_PERCPU_HASH; break;
 default: free(interno); if (error) *error = SIL_EBPF_MAPA; return NULL;
 }
 struct bpf_map_create_opts opts = { .sz = sizeof(opts), .map_flags = config->flags };
 interno->map = bpf_map_create(tipo, config->nombre, config->key_size, config->value_size, config->max_entries, &opts);
 if (libbpf_get_error(interno->map)) { free(interno); if (error) *error = SIL_EBPF_MAPA; return NULL; }
 interno->fd = bpf_map__fd(interno->map);
 if (error) *error = SIL_EBPF_OK;
 return (SilEbpfMap*)interno;
}

void sil_ebpf_mapa_destruir(SilEbpfMap *mapa) { if (!mapa) return; SilEbpfMapInterno *interno = (SilEbpfMapInterno*)mapa; if (interno->map) bpf_map__destroy(interno->map); free(interno); }

SilEbpfError sil_ebpf_mapa_actualizar(SilEbpfMap *mapa, const void *key, const void *value, uint64_t flags) { if (!mapa || !key || !value) return SIL_EBPF_ERROR; SilEbpfMapInterno *interno = (SilEbpfMapInterno*)mapa; int err = bpf_map__update_elem(interno->map, key, value, flags); return err == 0 ? SIL_EBPF_OK : SIL_EBPF_MAPA; }

SilEbpfError sil_ebpf_mapa_buscar(SilEbpfMap *mapa, const void *key, void *value) { if (!mapa || !key || !value) return SIL_EBPF_ERROR; SilEbpfMapInterno *interno = (SilEbpfMapInterno*)mapa; int err = bpf_map__lookup_elem(interno->map, key, value); return err == 0 ? SIL_EBPF_OK : SIL_EBPF_MAPA; }

SilEbpfError sil_ebpf_mapa_eliminar(SilEbpfMap *mapa, const void *key) { if (!mapa || !key) return SIL_EBPF_ERROR; SilEbpfMapInterno *interno = (SilEbpfMapInterno*)mapa; int err = bpf_map__delete_elem(interno->map, key); return err == 0 ? SIL_EBPF_OK : SIL_EBPF_MAPA; }

SilEbpfError sil_ebpf_mapa_iterar(SilEbpfMap *mapa, void *key, void *value, bool *mas) {
 if (!mapa || !key || !value || !mas) return SIL_EBPF_ERROR;
 SilEbpfMapInterno *interno = (SilEbpfMapInterno*)mapa;
 __u32 ks = bpf_map__key_size(interno->map);
 if (ks == 0 || ks > 4096) return SIL_EBPF_MAPA;
 char cur[4096]; char nxt[4096];
 memcpy(cur, key, ks);
 int err = bpf_map__get_next_key(interno->map, cur, nxt);
 if (err) { *mas = false; return SIL_EBPF_OK; }
 err = bpf_map__lookup_elem(interno->map, nxt, value);
 if (err) return SIL_EBPF_MAPA;
 memcpy(key, nxt, ks);
 *mas = true; return SIL_EBPF_OK;
}

int sil_ebpf_mapa_fd(SilEbpfMap *mapa) { if (!mapa) return -1; SilEbpfMapInterno *interno = (SilEbpfMapInterno*)mapa; return interno->fd; }

typedef struct { void *buf; size_t cap; size_t n; } EbpfMuestra_;
static int ebpf_muestra_cb(void *ctx, void *data, size_t len) {
 EbpfMuestra_ *m = (EbpfMuestra_*)ctx;
 size_t c = len < m->cap ? len : m->cap;
 memcpy(m->buf, data, c); m->n = c; return 0;
}
int sil_ebpf_eventos_leer(SilEbpfMap *mapa, void *buffer, size_t tamano, int timeout_ms) {
 if (!mapa || !buffer || tamano == 0) return -1;
 SilEbpfMapInterno *interno = (SilEbpfMapInterno*)mapa;
 if (bpf_map__type(interno->map) == BPF_MAP_TYPE_RINGBUF) {
  EbpfMuestra_ m = { buffer, tamano, 0 };
  struct ring_buffer *rb = ring_buffer__new(interno->fd, ebpf_muestra_cb, &m, NULL);
  if (!rb) return -1;
  int ret = ring_buffer__poll(rb, timeout_ms);
  ring_buffer__free(rb);
  if (ret < 0) return -1;
  return (int)m.n;
 } else if (bpf_map__type(interno->map) == BPF_MAP_TYPE_PERF_EVENT_ARRAY) {
  ssize_t r = read(interno->fd, buffer, tamano);
  return r < 0 ? -1 : (int)r;
 }
 return -1;
}

bool sil_ebpf_soportado(void) {
 int fd = memfd_create("sil-probe", 0);
 if (fd >= 0) close(fd);
 return access("/sys/fs/bpf", W_OK | X_OK) == 0 || fd >= 0;
}
uint32_t sil_ebpf_version_kernel(void) {
 struct utsname u;
 if (uname(&u) != 0) return 0;
 unsigned may = 0, men = 0, par = 0;
 sscanf(u.release, "%u.%u.%u", &may, &men, &par);
 return (may << 16) | (men << 8) | (par & 0xFFu);
}

#else

SilEbpfContext* sil_ebpf_ctx_crear(void) { return NULL; }
void sil_ebpf_ctx_destruir(SilEbpfContext *ctx) { (void)ctx; }
SilEbpfProgram* sil_ebpf_programa_cargar(SilEbpfContext *ctx, const SilEbpfProgramConfig *config, SilEbpfError *error) { (void)ctx; (void)config; if (error) *error = SIL_EBPF_NO_SOPORTADO; return NULL; }
void sil_ebpf_programa_destruir(SilEbpfProgram *prog) { (void)prog; }
SilEbpfLink* sil_ebpf_adjuntar(SilEbpfProgram *prog, const char *punto_anclaje, SilEbpfError *error) { (void)prog; (void)punto_anclaje; if (error) *error = SIL_EBPF_NO_SOPORTADO; return NULL; }
void sil_ebpf_desadjuntar(SilEbpfLink *link) { (void)link; }
SilEbpfMap* sil_ebpf_mapa_crear(SilEbpfContext *ctx, const SilEbpfMapConfig *config, SilEbpfError *error) { (void)ctx; (void)config; if (error) *error = SIL_EBPF_NO_SOPORTADO; return NULL; }
void sil_ebpf_mapa_destruir(SilEbpfMap *mapa) { (void)mapa; }
SilEbpfError sil_ebpf_mapa_actualizar(SilEbpfMap *mapa, const void *key, const void *value, uint64_t flags) { (void)mapa; (void)key; (void)value; (void)flags; return SIL_EBPF_NO_SOPORTADO; }
SilEbpfError sil_ebpf_mapa_buscar(SilEbpfMap *mapa, const void *key, void *value) { (void)mapa; (void)key; (void)value; return SIL_EBPF_NO_SOPORTADO; }
SilEbpfError sil_ebpf_mapa_eliminar(SilEbpfMap *mapa, const void *key) { (void)mapa; (void)key; return SIL_EBPF_NO_SOPORTADO; }
SilEbpfError sil_ebpf_mapa_iterar(SilEbpfMap *mapa, void *key, void *value, bool *mas) { (void)mapa; (void)key; (void)value; (void)mas; return SIL_EBPF_NO_SOPORTADO; }
int sil_ebpf_mapa_fd(SilEbpfMap *mapa) { (void)mapa; return -1; }
int sil_ebpf_eventos_leer(SilEbpfMap *mapa, void *buffer, size_t tamano, int timeout_ms) { (void)mapa; (void)buffer; (void)tamano; (void)timeout_ms; return -1; }
bool sil_ebpf_soportado(void) { return false; }
uint32_t sil_ebpf_version_kernel(void) { return 0; }

#endif
