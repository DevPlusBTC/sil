/* sil_arena_nvm.c - Persistencia NVM para arenas con integridad criptográfica.
 *
 * Implementación funcional (M9) de:
 *   - sil_arena_persistir
 *   - sil_arena_recuperar
 *   - sil_arena_destruir_persistida
 *   - sil_arena_verificar_nvm
 *   - sil_arena_info_nvm
 *
 * Formato de archivo en disco:
 *   [header]  magic(4) | version(4) | crc64(8) | hmac(32) |
 *             bloque_count(4) | total_size(8) | timestamp(8) |
 *             ciclo_contador(4) | ciclos_abiertos(4) | reserved(40)
 *   [bloques] por bloque: capacidad(8) | usado(8) | bytes(usado)
 *   [ciclos ] por ciclo:  id(4) | usado(8) | bytes(8) | bloque_idx(4)
 *
 * Integridad: CRC64-ECMA sobre el cuerpo; HMAC-SHA256 sobre (header0 || cuerpo).
 * Atomicidad: escritura a <path>.tmp + fsync + verificación + rename.
 */

#include "sil_arena.h"
#include "sil_hmac.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#ifdef _WIN32
#include <io.h>
#include <errno.h>
#define SIL_FSYNC(fd) _commit(fd)
#else
#include <unistd.h>
#include <errno.h>
#define SIL_FSYNC(fd) fsync(fd)
#endif

static const uint8_t SIL_NVM_KEY[] = "SIL-Arena-NVM-v1";
#define SIL_NVM_KEY_LEN (sizeof(SIL_NVM_KEY) - 1)

#define NVM_OK          0
#define NVM_ERR_ARG     (-1)
#define NVM_ERR_IO      (-2)
#define NVM_ERR_MAGIC   (-3)
#define NVM_ERR_VERSION (-4)
#define NVM_ERR_CRC     (-5)
#define NVM_ERR_HMAC    (-6)
#define NVM_ERR_LAYOUT  (-7)

static uint64_t sil_crc64(const uint8_t *data, size_t len) {
    uint64_t crc = 0xFFFFFFFFFFFFFFFFULL;
    for (size_t i = 0; i < len; i++) {
        crc ^= (uint64_t)data[i] << 56;
        for (int j = 0; j < 8; j++) {
            if (crc & 0x8000000000000000ULL)
                crc = (crc << 1) ^ 0x42F0E1EBA9EA3693ULL;
            else
                crc <<= 1;
        }
    }
    return crc ^ 0xFFFFFFFFFFFFFFFFULL;
}

typedef struct {
    uint8_t *buf;
    size_t   cap;
    size_t   len;
} NvmW;

static int w_push(NvmW *w, const void *src, size_t n) {
    if (w->len + n > w->cap) return NVM_ERR_LAYOUT;
    memcpy(w->buf + w->len, src, n);
    w->len += n;
    return NVM_OK;
}

static int w_u32(NvmW *w, uint32_t v) { return w_push(w, &v, sizeof(v)); }
static int w_u64(NvmW *w, uint64_t v) { return w_push(w, &v, sizeof(v)); }

static int build_body(const SilArena *a, NvmW *w) {
    for (SilArenaBloque *b = a->inicio; b; b = b->siguiente) {
        if (w_u64(w, (uint64_t)b->capacidad) != NVM_OK) return NVM_ERR_LAYOUT;
        if (w_u64(w, (uint64_t)b->usado) != NVM_OK) return NVM_ERR_LAYOUT;
        if (b->usado && w_push(w, b->memoria, b->usado) != NVM_OK)
            return NVM_ERR_LAYOUT;
    }
    for (uint32_t i = 0; i < a->ciclos_abiertos; i++) {
        const SilArenaCiclo *c = &a->ciclos[i];
        uint32_t bloque_index = 0;
        for (SilArenaBloque *b = a->inicio; b && b != c->bloque; b = b->siguiente)
            bloque_index++;
        if (w_u32(w, c->id) != NVM_OK) return NVM_ERR_LAYOUT;
        if (w_u64(w, (uint64_t)c->usado) != NVM_OK) return NVM_ERR_LAYOUT;
        if (w_u64(w, (uint64_t)c->bytes) != NVM_OK) return NVM_ERR_LAYOUT;
        if (w_u32(w, bloque_index) != NVM_OK) return NVM_ERR_LAYOUT;
    }
    return NVM_OK;
}

typedef struct {
    uint32_t magic;
    uint32_t version;
    uint64_t crc64;
    uint8_t  hmac[SIL_ARENA_NVM_HMAC_SIZE];
    uint32_t bloque_count;
    uint64_t total_size;
    uint64_t timestamp;
    uint32_t ciclo_contador;
    uint32_t ciclos_abiertos;
    uint8_t  reserved[40];
} NvmHeader;

static NvmHeader make_header(const SilArena *a, uint32_t bloque_count,
                             uint64_t crc64, uint64_t ts) {
    NvmHeader h;
    memset(&h, 0, sizeof(h));
    h.magic = SIL_ARENA_NVM_MAGIC;
    h.version = SIL_ARENA_NVM_VERSION;
    h.crc64 = crc64;
    h.bloque_count = bloque_count;
    h.total_size = (uint64_t)a->asignado_total;
    h.timestamp = ts;
    h.ciclo_contador = a->ciclo_contador;
    h.ciclos_abiertos = a->ciclos_abiertos;
    return h;
}

static void header_hmac(const NvmHeader *h, const uint8_t *body, size_t body_len,
                        uint8_t mac[SIL_ARENA_NVM_HMAC_SIZE]) {
    NvmHeader tmp = *h;
    memset(tmp.hmac, 0, sizeof(tmp.hmac));
    uint8_t hbuf[sizeof(tmp)];
    memcpy(hbuf, &tmp, sizeof(hbuf));

    uint8_t *all = (uint8_t *)malloc(sizeof(hbuf) + body_len);
    if (all) {
        memcpy(all, hbuf, sizeof(hbuf));
        if (body_len) memcpy(all + sizeof(hbuf), body, body_len);
        sil_hmac_sha256(SIL_NVM_KEY, SIL_NVM_KEY_LEN, all, sizeof(hbuf) + body_len, mac);
        free(all);
    } else {
        memset(mac, 0, SIL_ARENA_NVM_HMAC_SIZE);
    }
}

int sil_arena_persistir(const SilArena *a, const char *path) {
    if (!a || !path || path[0] == '\0') return NVM_ERR_ARG;

    uint32_t bloque_count = 0;
    for (SilArenaBloque *b = a->inicio; b; b = b->siguiente) bloque_count++;

    size_t body_cap = 0;
    for (SilArenaBloque *b = a->inicio; b; b = b->siguiente)
        body_cap += 16 + b->usado;
    body_cap += (size_t)a->ciclos_abiertos * 24;

    NvmW w = { NULL, body_cap + 1, 0 };
    w.buf = (uint8_t *)malloc(w.cap ? w.cap : 1);
    if (!w.buf) return NVM_ERR_IO;

    int rc = build_body(a, &w);
    if (rc != NVM_OK) { free(w.buf); return rc; }

    uint64_t body_crc = sil_crc64(w.buf, w.len);
    uint64_t ts = (uint64_t)time(NULL);
    NvmHeader h = make_header(a, bloque_count, body_crc, ts);
    header_hmac(&h, w.buf, w.len, h.hmac);

    size_t plen = strlen(path);
    char *tmp = (char *)malloc(plen + 8);
    if (!tmp) { free(w.buf); return NVM_ERR_IO; }
    memcpy(tmp, path, plen);
    memcpy(tmp + plen, ".tmp", 5);

    FILE *f = fopen(tmp, "wb");
    if (!f) { free(tmp); free(w.buf); return NVM_ERR_IO; }

    size_t wtot = 0;
    wtot += fwrite(&h, 1, sizeof(NvmHeader), f);
    wtot += fwrite(w.buf, 1, w.len, f);

    int io = (wtot == sizeof(NvmHeader) + w.len) ? 0 : 1;
    if (fflush(f) != 0) io = 1;
    if (!io) io = SIL_FSYNC(fileno(f)) == 0 ? 0 : 1;

    if (io) {
        fclose(f);
        remove(tmp);
        free(tmp); free(w.buf);
        return NVM_ERR_IO;
    }
    fclose(f);

    /* Verificación post-escritura */
    {
        FILE *rf = fopen(tmp, "rb");
        if (rf) {
            NvmHeader rh;
            uint8_t *rb = (uint8_t *)malloc(w.len ? w.len : 1);
            int ok = 0;
            if (rb) {
                ok = (fread(&rh, 1, sizeof(rh), rf) == sizeof(rh)) &&
                     (fread(rb, 1, w.len, rf) == w.len);
                if (ok) {
                    uint64_t crc = sil_crc64(rb, w.len);
                    uint8_t mac[SIL_ARENA_NVM_HMAC_SIZE];
                    header_hmac(&rh, rb, w.len, mac);
                    ok = (rh.magic == SIL_ARENA_NVM_MAGIC) &&
                         (rh.version == SIL_ARENA_NVM_VERSION) &&
                         (crc == rh.crc64) &&
                         sil_ct_igual(mac, rh.hmac, SIL_ARENA_NVM_HMAC_SIZE);
                }
                free(rb);
            }
            fclose(rf);
            if (!ok) {
                remove(tmp);
                free(tmp); free(w.buf);
                return NVM_ERR_CRC;
            }
        } else {
            free(tmp); free(w.buf);
            return NVM_ERR_IO;
        }
    }

    /* Windows no sobreescribe en rename(); eliminar destino previo si existe */
    remove(path);
    if (rename(tmp, path) != 0) {
        remove(tmp);
        free(tmp); free(w.buf);
        return NVM_ERR_IO;
    }
    free(tmp); free(w.buf);
    return NVM_OK;
}

int sil_arena_recuperar(SilArena *a, const char *path) {
    if (!a || !path || path[0] == '\0') return NVM_ERR_ARG;

    FILE *f = fopen(path, "rb");
    if (!f) return NVM_ERR_IO;

    NvmHeader h;
    if (fread(&h, 1, sizeof(h), f) != sizeof(h)) { fclose(f); return NVM_ERR_IO; }
    if (h.magic != SIL_ARENA_NVM_MAGIC) { fclose(f); return NVM_ERR_MAGIC; }
    if (h.version != SIL_ARENA_NVM_VERSION) { fclose(f); return NVM_ERR_VERSION; }

    fseek(f, 0, SEEK_END);
    long total = ftell(f);
    fseek(f, sizeof(h), SEEK_SET);
    size_t body_len = (total > (long)sizeof(h)) ? (size_t)(total - sizeof(h)) : 0;

    uint8_t *body = (uint8_t *)malloc(body_len ? body_len : 1);
    if (!body) { fclose(f); return NVM_ERR_IO; }
    if (fread(body, 1, body_len, f) != body_len) {
        fclose(f); free(body); return NVM_ERR_IO; }
    fclose(f);

    uint64_t crc = sil_crc64(body, body_len);
    if (crc != h.crc64) { free(body); return NVM_ERR_CRC; }
    uint8_t mac[SIL_ARENA_NVM_HMAC_SIZE];
    header_hmac(&h, body, body_len, mac);
    if (!sil_ct_igual(mac, h.hmac, SIL_ARENA_NVM_HMAC_SIZE)) {
        free(body); return NVM_ERR_HMAC;
    }

    if (a->inicio) sil_arena_destruir(a);
    memset(a, 0, sizeof(*a));

    size_t pos = 0;
    SilArenaBloque *prev = NULL;
    SilArenaBloque **slots = NULL;
    uint32_t nslots = 0, capslots = 0;

    for (uint32_t i = 0; i < h.bloque_count; i++) {
        if (pos + 16 > body_len) { free(body); return NVM_ERR_LAYOUT; }
        uint64_t capacidad, usado;
        memcpy(&capacidad, body + pos, 8); pos += 8;
        memcpy(&usado, body + pos, 8); pos += 8;
        if (pos + usado > body_len) { free(body); return NVM_ERR_LAYOUT; }

        SilArenaBloque *b = (SilArenaBloque *)malloc(sizeof(SilArenaBloque));
        b->memoria = (uint8_t *)malloc(capacidad ? (size_t)capacidad : 1);
        b->capacidad = (size_t)capacidad;
        b->usado = (size_t)usado;
        b->siguiente = NULL;
        if (!b || !b->memoria) { free(body); return NVM_ERR_IO; }
        if (usado) memcpy(b->memoria, body + pos, usado);
        pos += usado;

        if (!a->inicio) { a->inicio = b; }
        else { prev->siguiente = b; }
        prev = b;
        a->asignado_total += b->capacidad;

        if (nslots == capslots) {
            capslots = capslots ? capslots * 2 : 8;
            slots = (SilArenaBloque **)realloc(slots, capslots * sizeof(*slots));
            if (!slots) { free(body); return NVM_ERR_IO; }
        }
        slots[nslots++] = b;
    }
    a->actual = prev ? prev : a->inicio;

    for (uint32_t i = 0; i < h.ciclos_abiertos && i < SIL_ARENA_MAX_CICLOS; i++) {
        if (pos + 24 > body_len) { free(slots); free(body); return NVM_ERR_LAYOUT; }
        uint32_t id;
        memcpy(&id, body + pos, 4); pos += 4;
        uint64_t usado, bytes;
        memcpy(&usado, body + pos, 8); pos += 8;
        memcpy(&bytes, body + pos, 8); pos += 8;
        uint32_t bidx;
        memcpy(&bidx, body + pos, 4); pos += 4;

        SilArenaCiclo *c = &a->ciclos[i];
        c->id = id;
        c->usado = (size_t)usado;
        c->bytes = (size_t)bytes;
        c->bloque = (bidx < nslots) ? slots[bidx] : NULL;
    }
    a->ciclo_contador = h.ciclo_contador;
    a->ciclos_abiertos = h.ciclos_abiertos;
    a->ciclo_actual = h.ciclos_abiertos > 0
                          ? a->ciclos[h.ciclos_abiertos - 1].id
                          : 0;

    free(slots);
    free(body);
    return NVM_OK;
}

int sil_arena_verificar_nvm(const char *path) {
    if (!path || path[0] == '\0') return NVM_ERR_ARG;
    FILE *f = fopen(path, "rb");
    if (!f) return NVM_ERR_IO;
    NvmHeader h;
    if (fread(&h, 1, sizeof(h), f) != sizeof(h)) { fclose(f); return NVM_ERR_IO; }
    if (h.magic != SIL_ARENA_NVM_MAGIC) { fclose(f); return NVM_ERR_MAGIC; }
    if (h.version != SIL_ARENA_NVM_VERSION) { fclose(f); return NVM_ERR_VERSION; }

    fseek(f, 0, SEEK_END);
    long end = ftell(f);
    fseek(f, sizeof(h), SEEK_SET);
    size_t body_len = (end > (long)sizeof(h)) ? (size_t)(end - sizeof(h)) : 0;

    uint8_t *buf = (uint8_t *)malloc(body_len ? body_len : 1);
    if (!buf) { fclose(f); return NVM_ERR_IO; }
    if (fread(buf, 1, body_len, f) != body_len) {
        free(buf); fclose(f); return NVM_ERR_IO;
    }
    fclose(f);

    uint64_t crc = sil_crc64(buf, body_len);
    uint8_t mac[SIL_ARENA_NVM_HMAC_SIZE];
    header_hmac(&h, buf, body_len, mac);
    int ok = (crc == h.crc64) &&
             sil_ct_igual(mac, h.hmac, SIL_ARENA_NVM_HMAC_SIZE);
    free(buf);
    return ok ? NVM_OK : NVM_ERR_CRC;
}

int sil_arena_info_nvm(const char *path, SilArenaNvmInfo *info) {
    if (!info || !path || path[0] == '\0') return NVM_ERR_ARG;
    FILE *f = fopen(path, "rb");
    if (!f) return NVM_ERR_IO;
    NvmHeader h;
    int ok = fread(&h, 1, sizeof(h), f) == sizeof(h);
    fclose(f);
    if (!ok) return NVM_ERR_IO;
    if (h.magic != SIL_ARENA_NVM_MAGIC) return NVM_ERR_MAGIC;
    info->version = h.version;
    info->total_size = (size_t)h.total_size;
    info->bloque_count = h.bloque_count;
    info->ciclo_contador = h.ciclo_contador;
    info->timestamp = h.timestamp;
    return NVM_OK;
}

int sil_arena_destruir_persistida(const char *path) {
    if (!path || path[0] == '\0') return NVM_ERR_ARG;
    /* Remove es idempotente: si el archivo ya no existe, ENOENT se trata como
     * éxito (noop). Esto evita errores en llamadas repetidas. */
    int rc = remove(path);
    if (rc != 0) {
#ifdef _WIN32
        if (errno == ENOENT) return NVM_OK;
#else
        if (errno == ENOENT) return NVM_OK;
#endif
    }
    return rc == 0 ? NVM_OK : NVM_ERR_IO;
}
