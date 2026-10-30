/* test_channel.c - Contrato de los canales de capacidad acotada.
 *
 * Cubre el camino monohilo (orden FIFO, contrapresion, cierre, argumentos
 * invalidos), los dos canales concurrentes (SPSC y MPSC) y la propiedad de
 * memoria acotada: 1.000.000 de mensajes no aumentan la arena del canal.
 *
 * Los tests concurrentes se ejecutan tambien bajo ThreadSanitizer.
 */
#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "sil_arena.h"
#include "sil_channel.h"
#include "sil_test.h"

#define MENSAJES_SPSC 200000u
#define PRODUCTORES_MPSC 4
#define MENSAJES_POR_PRODUCTOR 25000u

static void crear_valida_parametros(void) {
    SilArena a = sil_arena_crear(1 << 16);
    SilChannel *c;
    ST_CHECK(sil_channel_crear(NULL, 8) == NULL);
    ST_CHECK(sil_channel_crear(&a, SIL_CANAL_CAPACIDAD_MAXIMA + 1) == NULL);
    c = sil_channel_crear(&a, 0);
    ST_CHECK(c != NULL);
    ST_CHECK_EQ(sil_channel_capacidad(c), SIL_CANAL_CAPACIDAD_DEFECTO);
    sil_channel_destruir(c);
    c = sil_channel_crear(&a, 100); /* se redondea a 128 */
    ST_CHECK(c != NULL);
    ST_CHECK_EQ(sil_channel_capacidad(c), 128);
    ST_CHECK(sil_channel_esta_vacio(c));
    ST_CHECK(!sil_channel_esta_cerrado(c));
    ST_CHECK_EQ(sil_channel_contar(c), 0);
    ST_CHECK(!sil_channel_enviar(c, NULL));
    ST_CHECK(!sil_channel_recibir(c, NULL));
    {
        void *out = NULL;
        ST_CHECK(!sil_channel_recibir(c, &out));
        ST_CHECK(out == NULL);
    }
    sil_arena_destruir(&a);
}

static void fifo_y_contrapresion(void) {
    SilArena a = sil_arena_crear(1 << 16);
    SilChannel *c = sil_channel_crear(&a, 4);
    long valores[6];
    void *out = NULL;
    int i;

    ST_CHECK(c != NULL);
    for (i = 0; i < 4; i++) {
        valores[i] = i + 1;
        ST_CHECK(sil_channel_enviar(c, &valores[i]));
    }
    ST_CHECK_EQ(sil_channel_contar(c), 4);
    ST_CHECK(!sil_channel_enviar(c, &valores[4])); /* lleno: contrapresion */
    ST_CHECK_EQ(sil_channel_contar(c), 4);
    for (i = 0; i < 4; i++) {
        ST_CHECK(sil_channel_recibir(c, &out));
        ST_CHECK_EQ(out, &valores[i]); /* orden FIFO estricto */
    }
    ST_CHECK(!sil_channel_recibir(c, &out)); /* vacio */
    ST_CHECK(sil_channel_esta_vacio(c));
    /* Reutilizacion de ranuras: tras drenar caben otra vez 4. */
    for (i = 0; i < 4; i++) ST_CHECK(sil_channel_enviar(c, &valores[i]));
    ST_CHECK_EQ(sil_channel_contar(c), 4);
    sil_channel_destruir(c);
    sil_arena_destruir(&a);
}

static void cierre_permite_drenar(void) {
    SilArena a = sil_arena_crear(1 << 16);
    SilChannel *c = sil_channel_crear(&a, 8);
    long v1 = 1, v2 = 2;
    void *out = NULL;
    ST_CHECK(c != NULL);
    ST_CHECK(sil_channel_enviar(c, &v1));
    ST_CHECK(sil_channel_enviar(c, &v2));
    sil_channel_cerrar(c);
    ST_CHECK(sil_channel_esta_cerrado(c));
    ST_CHECK(!sil_channel_enviar(c, &v1)); /* cerrado no acepta */
    ST_CHECK(sil_channel_recibir(c, &out) && out == &v1); /* pero drena */
    ST_CHECK(sil_channel_recibir(c, &out) && out == &v2);
    ST_CHECK(!sil_channel_recibir(c, &out));
    sil_channel_destruir(c);
    sil_arena_destruir(&a);
}

static void memoria_acotada(void) {
    SilArena a = sil_arena_crear(1 << 16);
    SilChannel *c = sil_channel_crear(&a, 64);
    size_t arena_tras_crear;
    unsigned i;
    void *out = NULL;
    long valor = 7;
    ST_CHECK(c != NULL);
    arena_tras_crear = a.asignado_total;
    for (i = 0; i < 200000u; i++) {
        while (!sil_channel_enviar(c, &valor)) (void)sil_channel_recibir(c, &out);
        while (sil_channel_recibir(c, &out)) { }
    }
    /* Cero malloc por mensaje: la arena no crece. */
    ST_CHECK_EQ(a.asignado_total, arena_tras_crear);
    sil_arena_destruir(&a);
}

/* --- SPSC concurrente ---------------------------------------------------- */

/* Banderas atomicas: `volatile` no basta (TSan lo marca como carrera). */
static int atomo_cargar(const int *p) { return __atomic_load_n(p, __ATOMIC_ACQUIRE); }
static void atomo_guardar(int *p, int v) { __atomic_store_n(p, v, __ATOMIC_RELEASE); }

typedef struct {
    SilChannel *canal;
    unsigned mensajes;
    int abortado;
} ArgSpsc;

static void *productor_spsc(void *p) {
    ArgSpsc *arg = (ArgSpsc *)p;
    unsigned i;
    for (i = 1; i <= arg->mensajes; i++) {
        while (!sil_channel_enviar(arg->canal, (void *)(uintptr_t)i)) {
            if (atomo_cargar(&arg->abortado)) return NULL;
        }
    }
    return NULL;
}

static void *consumidor_spsc(void *p) {
    ArgSpsc *arg = (ArgSpsc *)p;
    unsigned esperado = 1;
    void *out = NULL;
    while (esperado <= arg->mensajes) {
        if (!sil_channel_recibir(arg->canal, &out)) continue;
        if ((uintptr_t)out != (uintptr_t)esperado) {
            fprintf(stderr, "FALLO spsc: esperado %u, recibido %llu\n", esperado,
                    (unsigned long long)(uintptr_t)out);
            atomo_guardar(&arg->abortado, 1);
            return NULL;
        }
        esperado++;
    }
    return NULL;
}

static void spsc_concurrente(void) {
    SilArena a = sil_arena_crear(1 << 16);
    SilChannel *c = sil_channel_crear(&a, 128);
    ArgSpsc arg;
    pthread_t prod, cons;
    ST_CHECK(c != NULL);
    arg.canal = c;
    arg.mensajes = MENSAJES_SPSC;
    arg.abortado = 0;
    ST_CHECK_EQ(pthread_create(&prod, NULL, productor_spsc, &arg), 0);
    ST_CHECK_EQ(pthread_create(&cons, NULL, consumidor_spsc, &arg), 0);
    pthread_join(prod, NULL);
    pthread_join(cons, NULL);
    ST_CHECK_EQ(atomo_cargar(&arg.abortado), 0);
    ST_CHECK(sil_channel_esta_vacio(c));
    ST_CHECK_EQ(sil_channel_contar(c), 0);
    sil_arena_destruir(&a);
}

/* --- MPSC concurrente ---------------------------------------------------- */

typedef struct {
    SilChannelMPSC *canal;
    unsigned id;
    unsigned mensajes;
} ArgMpsc;

static void *productor_mpsc(void *p) {
    ArgMpsc *arg = (ArgMpsc *)p;
    unsigned i;
    for (i = 1; i <= arg->mensajes; i++) {
        /* Codifica productor y secuencia en un valor unico. */
        uintptr_t valor = ((uintptr_t)arg->id << 32) | (uintptr_t)i;
        while (!sil_channel_mpsc_enviar(arg->canal, (void *)valor)) { }
    }
    return NULL;
}

static void mpsc_concurrente(void) {
    SilArena a = sil_arena_crear(1 << 20);
    SilChannelMPSC *c = sil_channel_mpsc_crear(&a, 512);
    ArgMpsc args[PRODUCTORES_MPSC];
    pthread_t hilos[PRODUCTORES_MPSC];
    unsigned char *vistos;
    const unsigned total = PRODUCTORES_MPSC * MENSAJES_POR_PRODUCTOR;
    unsigned recibidos = 0;
    unsigned i;
    int duplicados = 0;
    unsigned vacios = 0;

    ST_CHECK(c != NULL);
    vistos = (unsigned char *)calloc(total, 1);
    ST_CHECK(vistos != NULL);
    if (!vistos) return;

    for (i = 0; i < PRODUCTORES_MPSC; i++) {
        args[i].canal = c;
        args[i].id = i + 1;
        args[i].mensajes = MENSAJES_POR_PRODUCTOR;
        ST_CHECK_EQ(pthread_create(&hilos[i], NULL, productor_mpsc, &args[i]), 0);
    }

    while (recibidos < total) {
        void *out = NULL;
        unsigned id, seq;
        if (!sil_channel_mpsc_recibir(c, &out)) {
            /* Cota de reintentos: evita colgar el test si se pierde un mensaje. */
            if (++vacios > (1u << 26)) {
                ST_FALLA("mpsc: mensajes perdidos (canal vacio antes de tiempo)");
                break;
            }
            continue;
        }
        vacios = 0;
        id = (unsigned)((uintptr_t)out >> 32);
        seq = (unsigned)((uintptr_t)out & 0xFFFFFFFFu);
        if (id < 1 || id > PRODUCTORES_MPSC || seq < 1 ||
            seq > MENSAJES_POR_PRODUCTOR) {
            duplicados++;
            recibidos++;
            continue;
        }
        {
            size_t indice = (size_t)(id - 1) * MENSAJES_POR_PRODUCTOR + (seq - 1);
            if (vistos[indice]) duplicados++;
            vistos[indice] = 1;
        }
        recibidos++;
    }

    for (i = 0; i < PRODUCTORES_MPSC; i++) pthread_join(hilos[i], NULL);
    ST_CHECK_EQ(duplicados, 0);
    ST_CHECK_EQ(recibidos, total);
    ST_CHECK(sil_channel_mpsc_esta_vacio(c));
    ST_CHECK_EQ(sil_channel_mpsc_contar(c), 0);
    ST_CHECK(!sil_channel_mpsc_enviar(c, NULL));
    sil_channel_mpsc_cerrar(c);
    ST_CHECK(sil_channel_mpsc_esta_cerrado(c));
    free(vistos);
    sil_arena_destruir(&a);
}

int main(void) {
    ST_CASO(crear_valida_parametros);
    ST_CASO(fifo_y_contrapresion);
    ST_CASO(cierre_permite_drenar);
    ST_CASO(memoria_acotada);
    ST_CASO(spsc_concurrente);
    ST_CASO(mpsc_concurrente);
    ST_FIN();
}
