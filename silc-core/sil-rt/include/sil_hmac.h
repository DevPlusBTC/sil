#ifndef SIL_HMAC_H
#define SIL_HMAC_H

#include <stdint.h>
#include <stddef.h>

#define SIL_SHA256_DIGESTO 32u
#define SIL_SHA256_BLOQUE 64u

/* SHA-256 de un mensaje en un solo paso. Sin malloc, sin estado global. */
void sil_sha256(const uint8_t *msg, size_t len, uint8_t digesto[SIL_SHA256_DIGESTO]);

/* HMAC-SHA256 (RFC 2104). Acepta claves de cualquier longitud. */
void sil_hmac_sha256(const uint8_t *clave, size_t clave_len,
                     const uint8_t *msg, size_t msg_len,
                     uint8_t mac[SIL_SHA256_DIGESTO]);

/* Comparacion en tiempo constante. Retorna 1 si iguales, 0 si no. */
int sil_ct_igual(const uint8_t *a, const uint8_t *b, size_t len);

#endif
