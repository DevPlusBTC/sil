/* test_runtime_emitido.c - Pruebas sobre el runtime C que silc EMITE.
 *
 * El backend C99 embebe su propio runtime como texto (RUNTIME_ARENA,
 * RUNTIME_TIPOS, RUNTIME_CONFIANZA). Este test se compila contra la extraccion
 * literal de silc-backend/src/c99.rs, de modo que lo que se valida es
 * exactamente el codigo que recibe el usuario que ejecuta `silc build`.
 *
 * Requiere que scripts/run_c_tests.sh haya generado `runtime_emitido.h`.
 */
#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "runtime_emitido.h"
#include "sil_test.h"

static void arena_emitida_alinea(void) {
    static const size_t alineaciones[] = {1, 2, 4, 8, 16, 32, 64};
    static const size_t tamanios[] = {1, 3, 8, 15, 16, 33, 100, 4096};
    size_t i, j;
    SilArena_ a = sil_arena_crear_(64);
    for (i = 0; i < sizeof(alineaciones) / sizeof(alineaciones[0]); i++) {
        for (j = 0; j < sizeof(tamanios) / sizeof(tamanios[0]); j++) {
            void *p = sil_arena_asignar_(&a, tamanios[j], alineaciones[i]);
            ST_CHECK(p != NULL);
            if (!p) continue;
            ST_CHECK_EQ((uintptr_t)p % alineaciones[i], 0);
            memset(p, 0x7F, tamanios[j]); /* ASan: nada fuera del bloque */
        }
    }
    sil_arena_destruir_(&a);
}

static void arena_emitida_alineacion_cero(void) {
    SilArena_ a = sil_arena_crear_(1024);
    void *p = sil_arena_asignar_(&a, 8, 0);
    void *q = sil_arena_asignar_(&a, 8, 0);
    /* El runtime anterior con `al = 0` calculaba `a = 0` y devolvia siempre el
     * mismo puntero sin reservar memoria. */
    ST_CHECK(p != NULL);
    ST_CHECK(q != NULL);
    ST_CHECK(p != q);
    ST_CHECK_EQ((uintptr_t)p % SIL_ARENA_ALINEACION_DEFECTO_, 0);
    sil_arena_destruir_(&a);

    ST_CHECK(sil_arena_asignar_(&a, 8, 3) == NULL); /* potencia de dos */
    ST_CHECK(sil_arena_asignar_(&a, (size_t)-1, 8) == NULL); /* desbordamiento */
    p = sil_arena_asignar_(&a, 32, 8); /* arena destruida: crea bloque */
    ST_CHECK(p != NULL);
    memset(p, 0, 32);
    sil_arena_destruir_(&a);
}

static void arena_emitida_peticion_enorme(void) {
    SilArena_ a = sil_arena_crear_(1024);
    size_t grande = (size_t)SIL_ARENA_PAGE_ * 2 + 5;
    void *p = sil_arena_asignar_(&a, grande, 64);
    ST_CHECK(p != NULL);
    ST_CHECK_EQ((uintptr_t)p % 64, 0);
    memset(p, 0xAB, grande);
    ST_CHECK(a.asignado_total >= grande);
    sil_arena_destruir_(&a);
}

static void canales_emitidos_contrato(void) {
    long long h, v = 0;
    int i;

    /* Capacidad 0 -> valor por defecto (64). */
    h = sil_canal_crear_(0);
    ST_CHECK(h >= 0);
    ST_CHECK(sil_canal_enviar_(h, 99));
    ST_CHECK(sil_canal_recibir_(h, &v));
    ST_CHECK_EQ(v, 99);
    ST_CHECK(!sil_canal_recibir_(h, &v)); /* vacio */
    sil_canal_cerrar_(h);
    ST_CHECK(!sil_canal_enviar_(h, 1)); /* cerrado */
    sil_canal_destruir_(h);

    /* Capacidad 4: contrapresion y orden FIFO. */
    h = sil_canal_crear_(4);
    ST_CHECK(h >= 0);
    for (i = 0; i < 4; i++) ST_CHECK(sil_canal_enviar_(h, i + 1));
    ST_CHECK(!sil_canal_enviar_(h, 5)); /* lleno */
    for (i = 0; i < 4; i++) {
        ST_CHECK(sil_canal_recibir_(h, &v));
        ST_CHECK_EQ(v, i + 1);
    }
    ST_CHECK(!sil_canal_recibir_(h, &v));
    sil_canal_destruir_(h);

    /* Capacidad absurda: rechazo sin desbordar `sizeof(long long) * cap`. */
    ST_CHECK_EQ(sil_canal_crear_((1UL << 40)), -1);

    /* El pool se recicla: crear/destruir 100 canales no lo agota. */
    for (i = 0; i < 100; i++) {
        h = sil_canal_crear_(2);
        ST_CHECK(h >= 0);
        if (h < 0) break;
        sil_canal_destruir_(h);
    }
}

static void hmac_emitido(void) {
    static const char esperado[] =
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
    unsigned char clave[20];
    unsigned char mac[32];
    char hex[65];
    static const char digitos[] = "0123456789abcdef";
    int i;

    memset(clave, 0x0b, sizeof clave);
    sil_hmac_(clave, sizeof clave, (const unsigned char *)"Hi There", 8, mac);
    for (i = 0; i < 32; i++) {
        hex[i * 2] = digitos[mac[i] >> 4];
        hex[i * 2 + 1] = digitos[mac[i] & 0x0Fu];
    }
    hex[64] = '\0';
    ST_CHECK_STR(hex, esperado);
}

static void capacidades_emitidas(void) {
    SilCap_ cap;
    ST_CHECK_EQ(setenv("SIL_CAP_KEY",
                       "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
                       1),
                0);
    cap = sil_cap_solicitar_("/pagos", SIL_PERM_LEER_, 5000000000ull);
    ST_CHECK(cap.valida);
    ST_CHECK(sil_cap_validar_(&cap, "/pagos", SIL_PERM_LEER_));
    ST_CHECK(!sil_cap_validar_(&cap, "/pagos", SIL_PERM_ESCRIBIR_));
    ST_CHECK(!sil_cap_validar_(&cap, "/admin", SIL_PERM_LEER_));

    cap.firma[0] ^= 0xFF;
    ST_CHECK(!sil_cap_validar_(&cap, "/pagos", SIL_PERM_LEER_));

    cap = sil_cap_solicitar_("/pagos", SIL_PERM_LEER_, 5000000000ull);
    cap.expiracion_ns = 0; /* ya expirada */
    ST_CHECK(!sil_cap_validar_(&cap, "/pagos", SIL_PERM_LEER_));

    /* Sin clave no se emiten capacidades. */
    unsetenv("SIL_CAP_KEY");
    cap = sil_cap_solicitar_("/pagos", SIL_PERM_LEER_, 1);
    ST_CHECK(!cap.valida);
    ST_CHECK(!sil_cap_validar_(&cap, "/pagos", SIL_PERM_LEER_));
}

static int ejecutadas;
static void fibra_emitida(void *ctx) {
    if (ctx) (*(int *)ctx)++;
    ejecutadas++;
}

static void scheduler_emitido(void) {
    SilSched_ s;
    int ctx_a = 0, ctx_b = 0;
    unsigned long i;
    memset(&s, 0, sizeof s);
    ejecutadas = 0;
    ST_CHECK(sil_sched_spawn_(&s, fibra_emitida, &ctx_a));
    ST_CHECK(sil_sched_spawn_(&s, fibra_emitida, &ctx_b));
    ST_CHECK(!sil_sched_spawn_(&s, NULL, 0));
    sil_sched_ejecutar_(&s);
    ST_CHECK_EQ(ctx_a, 1);
    ST_CHECK_EQ(ctx_b, 1);
    ST_CHECK_EQ(ejecutadas, 2);
    /* Ejecutar de nuevo no repite trabajo ya terminado. */
    sil_sched_ejecutar_(&s);
    ST_CHECK_EQ(ejecutadas, 2);
    /* Limite del pool. */
    memset(&s, 0, sizeof s);
    for (i = 0; i < SIL_FIBRAS_MAX_; i++) {
        if (!sil_sched_spawn_(&s, fibra_emitida, 0)) {
            ST_FALLA("spawn fallo antes del limite del pool");
            break;
        }
    }
    ST_CHECK(!sil_sched_spawn_(&s, fibra_emitida, 0));
    ejecutadas = 0;
    sil_sched_ejecutar_(&s);
    ST_CHECK_EQ(ejecutadas, (int)SIL_FIBRAS_MAX_);
}

int main(void) {
    ST_CASO(arena_emitida_alinea);
    ST_CASO(arena_emitida_alineacion_cero);
    ST_CASO(arena_emitida_peticion_enorme);
    ST_CASO(canales_emitidos_contrato);
    ST_CASO(hmac_emitido);
    ST_CASO(capacidades_emitidas);
    ST_CASO(scheduler_emitido);
    ST_FIN();
}
