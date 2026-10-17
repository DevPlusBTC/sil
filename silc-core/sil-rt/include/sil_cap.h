#ifndef SIL_CAP_H
#define SIL_CAP_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

typedef enum {
    SIL_CAP_OK = 0,
    SIL_CAP_ERROR = -1,
    SIL_CAP_NOT_IMPLEMENTED = -2,
    SIL_CAP_INVALID_PARAM = -3,
    SIL_CAP_SIN_CLAVE = -4,
} SilCapError;

typedef enum {
    SIL_CAP_NINGUNO = 0x00,
    SIL_CAP_LEER_ARCHIVO = 0x01,
    SIL_CAP_ESCRIBIR_ARCHIVO = 0x02,
    SIL_CAP_RED = 0x04,
    SIL_CAP_EXEC = 0x08,
} SilPermiso;

/* Capacidad Zero-Trust con firma HMAC-SHA256.
 * La firma cubre (magic, permisos, expiracion, recurso). El recurso
 * queda ligado por hash: una capacidad no se puede reutilizar en otra ruta. */
typedef struct {
    uint64_t id;
    uint32_t permisos;
    uint64_t expiracion_ns;
    uint8_t recurso_hash[32];
    uint8_t firma[32];
    bool valida;
} SilCapacidad;

/* Clave fija desde el proceso huesped (ej. wrapper Rust que ya leyo el entorno).
 * Tiene prioridad sobre SIL_CAP_KEY. Se limpia al reemplazar. */
SilCapError sil_cap_fijar_clave(const uint8_t clave[32]);

/* Solicita una capacidad firmada para `recurso` (max 1024 bytes).
 * Clave: variable de entorno SIL_CAP_KEY con 64 hex chars (32 bytes).
 * Sin clave retorna SIL_CAP_SIN_CLAVE. ttl_ns == 0 usa 500ms por defecto. */
SilCapError sil_cap_solicitar(const char *recurso, uint32_t permisos, uint64_t ttl_ns, SilCapacidad *out_cap);

/* Valida firma, permisos, recurso y expiracion. O(1).
 * ahora_ns == 0 usa reloj monotonico interno. */
SilCapError sil_cap_validar(const SilCapacidad *cap, const char *recurso, SilPermiso requerido, uint64_t ahora_ns);

#endif
