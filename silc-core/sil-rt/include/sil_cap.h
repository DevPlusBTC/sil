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
} SilCapError;

typedef enum {
    SIL_CAP_NINGUNO = 0x00,
    SIL_CAP_LEER_ARCHIVO = 0x01,
    SIL_CAP_ESCRIBIR_ARCHIVO = 0x02,
    SIL_CAP_RED = 0x04,
    SIL_CAP_EXEC = 0x08,
} SilPermiso;

typedef struct {
    uint64_t id;
    uint32_t permisos;
    uint64_t expiracion_ns;
    uint8_t firma[32];
    bool valida;
} SilCapacidad;

/* Valida una capacidad existente (O(1), sin syscalls salvo clock_gettime).
 * Requiere capacidad válida, permisos correctos y no expirada. */
SilCapError sil_cap_validar(const SilCapacidad *cap, SilPermiso requerido, uint64_t ahora_ns);

/* Solicita una nueva capacidad.
 * NOTA: Implementación de desarrollo (DEV-ONLY). La firma es XOR simple (NO CRIPTOGRÁFICA).
 * El parámetro `recurso` se ignora en esta versión (sin resource binding).
 * Para producción: requiere HMAC-SHA256 con clave en TPM 2.0 + attestation.
 * Retorna SIL_CAP_NOT_IMPLEMENTED hasta que se integre backend criptográfico real. */
SilCapError sil_cap_solicitar(const char *recurso, uint32_t permisos, uint64_t ttl_ns, SilCapacidad *out_cap);

#endif