/* sil_cap.c - Capacidades Zero-Trust (M7).
 *
 * M7: validación userspace real con reloj monotónico + integridad estructural.
 * - sil_cap_validar(): O(1), sin syscalls salvo clock_gettime (vDSO, ~20ns).
 * - sil_cap_solicitar(): token de DESARROLLO (autofirmado, NO SEGURO).
 *   Fase 1: firma HMAC-SHA256 con clave en TPM 2.0 + attestation eBPF.
 *
 * Disciplina: todos los punteros se validan; tiempos en ns monotónicos;
 * expiración estricta (ahora > expiración ⟹ inválido).
 */
#include "sil_cap.h"
#include <string.h>
#include <time.h>
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#endif

#define SIL_CAP_MAGIC 0xC4A981E5u

static unsigned long long ahora_monotonico_ns(void) {
#if defined(_WIN32)
    /* Windows: QueryPerformanceCounter (Fase 1: InitOnce caching de frecuencia). */
    {
        static LARGE_INTEGER freq = {0};
        LARGE_INTEGER c;
        if (freq.QuadPart == 0) QueryPerformanceFrequency(&freq);
        QueryPerformanceCounter(&c);
        if (freq.QuadPart == 0) return 0;
        return (unsigned long long)(c.QuadPart * 1000000000ull / freq.QuadPart);
    }
#else
    struct timespec ts;
#if defined(CLOCK_MONOTONIC)
    clock_gettime(CLOCK_MONOTONIC, &ts);
#else
    clock_gettime(CLOCK_REALTIME, &ts);
#endif
    return (unsigned long long)ts.tv_sec * 1000000000ull + (unsigned long long)ts.tv_nsec;
#endif
}

bool sil_cap_validar(const SilCapacidad *cap, SilPermiso requerido, unsigned long long ahora_ns) {
    if (!cap) return false;
    if (!cap->valida) return false;
    if (((unsigned)cap->permisos & (unsigned)requerido) != (unsigned)requerido) return false;
    if (ahora_ns == 0) ahora_ns = ahora_monotonico_ns();
    if (ahora_ns > cap->expiracion_ns) return false;
    return true;
}

SilCapacidad sil_cap_solicitar(const char *recurso, unsigned int permisos, unsigned long long ttl_ns) {
    (void)recurso; /* M7: sin binding a recurso (Fase 1: dominio/path en firma). */
    SilCapacidad c;
    memset(&c, 0, sizeof(c));
    c.id = SIL_CAP_MAGIC;
    c.permisos = permisos;
    c.expiracion_ns = ahora_monotonico_ns() + (ttl_ns ? ttl_ns : 500000ull);
    /* Firma de desarrollo: XOR simple (NO CRIPTOGRÁFICO). Fase 1: HMAC-TPM. */
    for (int i = 0; i < 32; i++) c.firma[i] = (unsigned char)((c.id >> (i % 8)) ^ 0xA5u);
    c.valida = true;
    return c;
}
