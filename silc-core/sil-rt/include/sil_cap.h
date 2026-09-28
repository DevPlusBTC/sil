#ifndef SIL_CAP_H
#define SIL_CAP_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

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

bool sil_cap_validar(const SilCapacidad *cap, SilPermiso requerido, uint64_t ahora_ns);
SilCapacidad sil_cap_solicitar(const char *recurso, uint32_t permisos, uint64_t ttl_ns);

#endif
