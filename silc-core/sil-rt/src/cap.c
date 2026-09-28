/* sil_cap.c - Capacidades Zero-Trust. M0: validación userspace (eBPF/TPM en Fase 1+). */
#include "sil_cap.h"
#include <string.h>
#include <time.h>

static uint64_t ahora_monotonico_ns(void) {
#if defined(_WIN32)
    /* M0 Windows: stub (Fase 1: QueryPerformanceCounter). */
    return 0;
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
#endif
}

bool sil_cap_validar(const SilCapacidad *cap, SilPermiso requerido, uint64_t ahora_ns) {
    if (!cap || !cap->valida) return false;
    if ((cap->permisos & (uint32_t)requerido) != (uint32_t)requerido) return false;
    if (ahora_ns > cap->expiracion_ns) return false;
    return true;
}

SilCapacidad sil_cap_solicitar(const char *recurso, uint32_t permisos, uint64_t ttl_ns) {
    /* M0: token autofirmado SOLO para desarrollo. NO SEGURO. Fase 1: TPM 2.0. */
    (void)recurso;
    SilCapacidad c;
    memset(&c, 0, sizeof(c));
    c.id = 0xDEADBEEF;
    c.permisos = permisos;
    c.expiracion_ns = ahora_monotonico_ns() + ttl_ns;
    c.valida = true;
    return c;
}
