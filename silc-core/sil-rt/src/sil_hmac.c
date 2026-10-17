/* sil_hmac.c - SHA-256 + HMAC-SHA256 autocontenido (dominio publico, estilo Brad Conte).
 *
 * Sin malloc, sin estado global, solo stack. Para firmar capacidades SIL.
 */

#include "sil_hmac.h"
#include <string.h>

#define ROTR(x, n) (((x) >> (n)) | ((x) << (32 - (n))))
#define CH(x, y, z) (((x) & (y)) ^ (~(x) & (z)))
#define MAJ(x, y, z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define EP0(x) (ROTR(x, 2) ^ ROTR(x, 13) ^ ROTR(x, 22))
#define EP1(x) (ROTR(x, 6) ^ ROTR(x, 11) ^ ROTR(x, 25))
#define SIG0(x) (ROTR(x, 7) ^ ROTR(x, 18) ^ ((x) >> 3))
#define SIG1(x) (ROTR(x, 17) ^ ROTR(x, 19) ^ ((x) >> 10))

static const uint32_t K[64] = {
    0x428a2f98u, 0x71374491u, 0xb5c0fbcfu, 0xe9b5dba5u,
    0x3956c25bu, 0x59f111f1u, 0x923f82a4u, 0xab1c5ed5u,
    0xd807aa98u, 0x12835b01u, 0x243185beu, 0x550c7dc3u,
    0x72be5d74u, 0x80deb1feu, 0x9bdc06a7u, 0xc19bf174u,
    0xe49b69c1u, 0xefbe4786u, 0x0fc19dc6u, 0x240ca1ccu,
    0x2de92c6fu, 0x4a7484aau, 0x5cb0a9dcu, 0x76f988dau,
    0x983e5152u, 0xa831c66du, 0xb00327c8u, 0xbf597fc7u,
    0xc6e00bf3u, 0xd5a79147u, 0x06ca6351u, 0x14292967u,
    0x27b70a85u, 0x2e1b2138u, 0x4d2c6dfcu, 0x53380d13u,
    0x650a7354u, 0x766a0abbu, 0x81c2c92eu, 0x92722c85u,
    0xa2bfe8a1u, 0xa81a664bu, 0xc24b8b70u, 0xc76c51a3u,
    0xd192e819u, 0xd6990624u, 0xf40e3585u, 0x106aa070u,
    0x19a4c116u, 0x1e376c08u, 0x2748774cu, 0x34b0bcb5u,
    0x391c0cb3u, 0x4ed8aa4au, 0x5b9cca4fu, 0x682e6ff3u,
    0x748f82eeu, 0x78a5636fu, 0x84c87814u, 0x8cc70208u,
    0x90befffau, 0xa4506cebu, 0xbef9a3f7u, 0xc67178f2u
};

typedef struct {
    uint8_t datos[64];
    uint32_t len_datos;
    uint64_t len_bits;
    uint32_t estado[8];
} Sha256Ctx_;

static void sha256_comprimir(Sha256Ctx_ *ctx, const uint8_t bloque[64]) {
    uint32_t w[64];
    for (int i = 0; i < 16; i++)
        w[i] = ((uint32_t)bloque[i * 4] << 24) | ((uint32_t)bloque[i * 4 + 1] << 16) |
               ((uint32_t)bloque[i * 4 + 2] << 8) | ((uint32_t)bloque[i * 4 + 3]);
    for (int i = 16; i < 64; i++)
        w[i] = SIG1(w[i - 2]) + w[i - 7] + SIG0(w[i - 15]) + w[i - 16];

    uint32_t a = ctx->estado[0], b = ctx->estado[1], c = ctx->estado[2], d = ctx->estado[3];
    uint32_t e = ctx->estado[4], f = ctx->estado[5], g = ctx->estado[6], h = ctx->estado[7];
    for (int i = 0; i < 64; i++) {
        uint32_t t1 = h + EP1(e) + CH(e, f, g) + K[i] + w[i];
        uint32_t t2 = EP0(a) + MAJ(a, b, c);
        h = g; g = f; f = e; e = d + t1;
        d = c; c = b; b = a; a = t1 + t2;
    }
    ctx->estado[0] += a; ctx->estado[1] += b; ctx->estado[2] += c; ctx->estado[3] += d;
    ctx->estado[4] += e; ctx->estado[5] += f; ctx->estado[6] += g; ctx->estado[7] += h;
}

static void sha256_init(Sha256Ctx_ *ctx) {
    ctx->len_datos = 0; ctx->len_bits = 0;
    ctx->estado[0] = 0x6a09e667u; ctx->estado[1] = 0xbb67ae85u;
    ctx->estado[2] = 0x3c6ef372u; ctx->estado[3] = 0xa54ff53au;
    ctx->estado[4] = 0x510e527fu; ctx->estado[5] = 0x9b05688cu;
    ctx->estado[6] = 0x1f83d9abu; ctx->estado[7] = 0x5be0cd19u;
}

static void sha256_update(Sha256Ctx_ *ctx, const uint8_t *msg, size_t len) {
    for (size_t i = 0; i < len; i++) {
        ctx->datos[ctx->len_datos++] = msg[i];
        if (ctx->len_datos == 64) {
            sha256_comprimir(ctx, ctx->datos);
            ctx->len_bits += 512;
            ctx->len_datos = 0;
        }
    }
}

static void sha256_final(Sha256Ctx_ *ctx, uint8_t digesto[32]) {
    uint64_t total = ctx->len_bits + (uint64_t)ctx->len_datos * 8u;
    ctx->datos[ctx->len_datos++] = 0x80;
    if (ctx->len_datos > 56) {
        while (ctx->len_datos < 64) ctx->datos[ctx->len_datos++] = 0;
        sha256_comprimir(ctx, ctx->datos);
        ctx->len_datos = 0;
    }
    while (ctx->len_datos < 56) ctx->datos[ctx->len_datos++] = 0;
    for (int i = 7; i >= 0; i--) ctx->datos[ctx->len_datos++] = (uint8_t)(total >> (i * 8));
    sha256_comprimir(ctx, ctx->datos);
    for (int i = 0; i < 8; i++) {
        digesto[i * 4] = (uint8_t)(ctx->estado[i] >> 24);
        digesto[i * 4 + 1] = (uint8_t)(ctx->estado[i] >> 16);
        digesto[i * 4 + 2] = (uint8_t)(ctx->estado[i] >> 8);
        digesto[i * 4 + 3] = (uint8_t)(ctx->estado[i]);
    }
}

void sil_sha256(const uint8_t *msg, size_t len, uint8_t digesto[SIL_SHA256_DIGESTO]) {
    Sha256Ctx_ ctx;
    sha256_init(&ctx);
    if (msg && len) sha256_update(&ctx, msg, len);
    else if (!msg && len) return;
    sha256_final(&ctx, digesto);
    memset(&ctx, 0, sizeof(ctx));
}

void sil_hmac_sha256(const uint8_t *clave, size_t clave_len,
                     const uint8_t *msg, size_t msg_len,
                     uint8_t mac[SIL_SHA256_DIGESTO]) {
    uint8_t k[SIL_SHA256_BLOQUE];
    memset(k, 0, sizeof(k));
    if (clave_len > sizeof(k)) {
        sil_sha256(clave, clave_len, k);
    } else if (clave && clave_len) {
        memcpy(k, clave, clave_len);
    }
    uint8_t ipad[SIL_SHA256_BLOQUE], opad[SIL_SHA256_BLOQUE];
    for (int i = 0; i < 64; i++) { ipad[i] = k[i] ^ 0x36u; opad[i] = k[i] ^ 0x5cu; }
    Sha256Ctx_ ctx;
    sha256_init(&ctx);
    sha256_update(&ctx, ipad, sizeof(ipad));
    if (msg && msg_len) sha256_update(&ctx, msg, msg_len);
    uint8_t interno[SIL_SHA256_DIGESTO];
    sha256_final(&ctx, interno);
    sha256_init(&ctx);
    sha256_update(&ctx, opad, sizeof(opad));
    sha256_update(&ctx, interno, sizeof(interno));
    sha256_final(&ctx, mac);
    memset(&ctx, 0, sizeof(ctx));
    memset(k, 0, sizeof(k));
    memset(ipad, 0, sizeof(ipad));
    memset(opad, 0, sizeof(opad));
    memset(interno, 0, sizeof(interno));
}

int sil_ct_igual(const uint8_t *a, const uint8_t *b, size_t len) {
    if (!a || !b) return 0;
    uint8_t diff = 0;
    for (size_t i = 0; i < len; i++) diff |= (uint8_t)(a[i] ^ b[i]);
    return diff == 0 ? 1 : 0;
}
