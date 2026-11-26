/* sil_cap.c - Capacidades Zero-Trust con HMAC-SHA256 real.
 *
 * - sil_cap_solicitar(): firma HMAC-SHA256(magic|permisos|expiracion|recurso)
 *   con clave de 32 bytes en SIL_CAP_KEY (hex). Liga la capacidad al recurso.
 * - sil_cap_validar(): recalcula y compara en tiempo constante + permisos +
 *   expiracion estricta con reloj monotonico (vDSO/QPC, ~20ns).
 * - Sin malloc. La unica pieza de estado global es la clave fijada por el
 *   propio proceso (sil_cap_fijar_clave); se publica con una store release y
 *   se lee con acquire, y por contrato se fija una sola vez durante el
 *   arranque (antes de emitir capacidades).
 */

#define _POSIX_C_SOURCE 200809L
#include "sil_cap.h"
#include "sil_hmac.h"
#include <string.h>
#include <stdlib.h>
#include <time.h>
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#endif
#if defined(_MSC_VER)
#include <intrin.h>
#endif

#define SIL_CAP_MAGIC 0xC4A981E5u
#define SIL_CAP_MAX_RECURSO 1024u
#define SIL_CAP_TTL_DEFECTO_NS 500000000ull /* 500ms */
/* Bits de permiso definidos en sil_cap.h (LEER|ESCRIBIR|RED|EXEC). */
#define SIL_CAP_PERMISOS_VALIDOS 0x0Fu

static unsigned long long ahora_monotonico_ns(void) {
#if defined(_WIN32)
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

/* Clave fijada por el proceso huesped (prioritaria sobre SIL_CAP_KEY). */
static uint8_t clave_fija[32];
static int hay_clave_fija = 0;

SilCapError sil_cap_fijar_clave(const uint8_t clave[32]) {
    if (!clave) return SIL_CAP_INVALID_PARAM;
    memcpy(clave_fija, clave, 32);
#if defined(_MSC_VER)
    _ReadWriteBarrier();
    hay_clave_fija = 1;
#else
    __atomic_store_n(&hay_clave_fija, 1, __ATOMIC_RELEASE);
#endif
    return SIL_CAP_OK;
}

static int clave_fija_publicada(void) {
#if defined(_MSC_VER)
    return hay_clave_fija;
#else
    return __atomic_load_n(&hay_clave_fija, __ATOMIC_ACQUIRE);
#endif
}

static int leer_clave(uint8_t clave[32]) {
    if (clave_fija_publicada()) {
        memcpy(clave, clave_fija, 32);
        return 1;
    }
    const char *hex = getenv("SIL_CAP_KEY");
    if (!hex) return 0;
    if (strlen(hex) != 64) return 0;
    for (int i = 0; i < 32; i++) {
        unsigned v = 0;
        for (int j = 0; j < 2; j++) {
            char c = hex[i * 2 + j];
            v <<= 4;
            if (c >= '0' && c <= '9') v |= (unsigned)(c - '0');
            else if (c >= 'a' && c <= 'f') v |= (unsigned)(c - 'a' + 10);
            else if (c >= 'A' && c <= 'F') v |= (unsigned)(c - 'A' + 10);
            else return 0;
        }
        clave[i] = (uint8_t)v;
    }
    return 1;
}

static void le64(uint8_t out[8], uint64_t v) {
    for (int i = 0; i < 8; i++) out[i] = (uint8_t)(v >> (i * 8));
}

static void le32(uint8_t out[4], uint32_t v) {
    for (int i = 0; i < 4; i++) out[i] = (uint8_t)(v >> (i * 8));
}

/* Mensaje firmado: magic(8) | permisos(4) | expiracion(8) | recurso_hash(32). */
static void mensaje_firmar(uint64_t id, uint32_t permisos, uint64_t exp,
                           const uint8_t rh[32], const uint8_t clave[32],
                           uint8_t firma[32]) {
    uint8_t msg[8 + 4 + 8 + 32];
    le64(msg, id);
    le32(msg + 8, permisos);
    le64(msg + 12, exp);
    memcpy(msg + 20, rh, 32);
    sil_hmac_sha256(clave, 32, msg, sizeof(msg), firma);
    memset(msg, 0, sizeof(msg));
}

SilCapError sil_cap_solicitar(const char *recurso, uint32_t permisos, uint64_t ttl_ns, SilCapacidad *out_cap) {
    if (!out_cap || !recurso) return SIL_CAP_INVALID_PARAM;
    size_t lr = strlen(recurso);
    if (lr == 0 || lr > SIL_CAP_MAX_RECURSO) return SIL_CAP_INVALID_PARAM;
    /* Una capacidad sin permisos no autoriza nada: se rechaza en origen para
     * que nunca exista una capacidad "vacia" que valide por accidente. */
    if (permisos == 0) return SIL_CAP_INVALID_PARAM;
    if ((permisos & ~(uint32_t)SIL_CAP_PERMISOS_VALIDOS) != 0) return SIL_CAP_INVALID_PARAM;

    uint8_t clave[32];
    if (!leer_clave(clave)) {
        memset(out_cap, 0, sizeof(*out_cap));
        return SIL_CAP_SIN_CLAVE;
    }
    uint8_t rh[32];
    sil_sha256((const uint8_t *)recurso, lr, rh);

    uint64_t exp = ahora_monotonico_ns() + (ttl_ns ? ttl_ns : SIL_CAP_TTL_DEFECTO_NS);
    uint8_t firma[32];
    mensaje_firmar(SIL_CAP_MAGIC, permisos, exp, rh, clave, firma);

    memset(out_cap, 0, sizeof(*out_cap));
    out_cap->id = SIL_CAP_MAGIC;
    out_cap->permisos = permisos;
    out_cap->expiracion_ns = exp;
    memcpy(out_cap->recurso_hash, rh, 32);
    memcpy(out_cap->firma, firma, 32);
    out_cap->valida = true;

    memset(clave, 0, sizeof(clave));
    memset(rh, 0, sizeof(rh));
    memset(firma, 0, sizeof(firma));
    return SIL_CAP_OK;
}

SilCapError sil_cap_validar(const SilCapacidad *cap, const char *recurso, SilPermiso requerido, uint64_t ahora_ns) {
    if (!cap || !recurso) return SIL_CAP_INVALID_PARAM;
    if (!cap->valida) return SIL_CAP_ERROR;
    if (cap->id != SIL_CAP_MAGIC) return SIL_CAP_ERROR;
    if ((unsigned)requerido == 0u) return SIL_CAP_INVALID_PARAM;
    if (((unsigned)requerido & ~(unsigned)SIL_CAP_PERMISOS_VALIDOS) != 0)
        return SIL_CAP_INVALID_PARAM;
    if (((unsigned)cap->permisos & (unsigned)requerido) != (unsigned)requerido) return SIL_CAP_ERROR;
    size_t lr = strlen(recurso);
    if (lr == 0 || lr > SIL_CAP_MAX_RECURSO) return SIL_CAP_INVALID_PARAM;

    uint64_t ahora = ahora_ns ? ahora_ns : ahora_monotonico_ns();
    if (ahora > cap->expiracion_ns) return SIL_CAP_ERROR;

    uint8_t clave[32];
    if (!leer_clave(clave)) return SIL_CAP_SIN_CLAVE;
    uint8_t rh[32];
    sil_sha256((const uint8_t *)recurso, lr, rh);
    if (!sil_ct_igual(rh, cap->recurso_hash, 32)) {
        memset(clave, 0, sizeof(clave));
        memset(rh, 0, sizeof(rh));
        return SIL_CAP_ERROR;
    }
    uint8_t esperada[32];
    mensaje_firmar(cap->id, cap->permisos, cap->expiracion_ns, rh, clave, esperada);
    int ok = sil_ct_igual(esperada, cap->firma, 32);
    memset(clave, 0, sizeof(clave));
    memset(rh, 0, sizeof(rh));
    memset(esperada, 0, sizeof(esperada));
    return ok ? SIL_CAP_OK : SIL_CAP_ERROR;
}
