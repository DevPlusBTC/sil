/* bench_canales.c - Medidas reproducibles de los canales y la arena de sil-rt.
 *
 * No es un test (no falla): imprime ns/operacion y millones de mensajes por
 * segundo para que cualquier comparacion futura se haga sobre numeros medidos.
 * Se ejecuta con `scripts/run_c_tests.sh --bench`.
 */
#define _POSIX_C_SOURCE 200809L
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#include "sil_arena.h"
#include "sil_channel.h"

static double ahora_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec * 1e9 + (double)ts.tv_nsec;
}

#define MENSAJES 5000000u

static void *productor(void *p) {
    SilChannel *c = (SilChannel *)p;
    unsigned i;
    for (i = 0; i < MENSAJES; i++) {
        while (!sil_channel_enviar(c, (void *)(uintptr_t)(i + 1u))) { }
    }
    return NULL;
}

static void *consumidor(void *p) {
    SilChannel *c = (SilChannel *)p;
    unsigned i;
    void *out = NULL;
    for (i = 0; i < MENSAJES; i++) {
        while (!sil_channel_recibir(c, &out)) { }
    }
    return NULL;
}

#define PRODUCTORES 4u
#define POR_PRODUCTOR 1000000u

static void *productor_mpsc(void *p) {
    SilChannelMPSC *c = (SilChannelMPSC *)p;
    unsigned i;
    for (i = 0; i < POR_PRODUCTOR; i++) {
        while (!sil_channel_mpsc_enviar(c, (void *)(uintptr_t)(i + 1u))) { }
    }
    return NULL;
}

int main(void) {
    SilArena arena = sil_arena_crear(1 << 16);
    double t0, t1;

    /* 1. Arena: coste por asignacion de 32 B. */
    {
        const unsigned n = 2000000u;
        unsigned i;
        t0 = ahora_ns();
        for (i = 0; i < n; i++) {
            if (!sil_arena_asignar(&arena, 32, 8)) break;
        }
        t1 = ahora_ns();
        printf("arena/asignar 32B      : %7.2f ns/op  (%u op)\n", (t1 - t0) / n, n);
        sil_arena_reset(&arena);
    }

    /* 2. Anillo en un solo hilo: coste puro de enviar+recibir (sin planificar
     *    hilos, que en maquinas con pocos nucleos domina la medida anterior). */
    {
        SilChannel *c = sil_channel_crear(&arena, 1024);
        const unsigned n = 2000000u;
        unsigned i;
        void *out = NULL;
        t0 = ahora_ns();
        for (i = 0; i < n; i++) {
            sil_channel_enviar(c, (void *)(uintptr_t)(i + 1u));
            sil_channel_recibir(c, &out);
        }
        t1 = ahora_ns();
        printf("anillo enviar+recibir  : %7.2f ns/par (%6.2f M par/s)\n",
               (t1 - t0) / n, n / ((t1 - t0) / 1e9) / 1e6);
        sil_arena_reset(&arena);
    }

    /* 3. SPSC: un productor, un consumidor, canal de 1024. */
    {
        SilChannel *c = sil_channel_crear(&arena, 1024);
        pthread_t p, q;
        t0 = ahora_ns();
        pthread_create(&p, NULL, productor, c);
        pthread_create(&q, NULL, consumidor, c);
        pthread_join(p, NULL);
        pthread_join(q, NULL);
        t1 = ahora_ns();
        printf("canal SPSC (1P/1C)     : %7.2f ns/msg (%6.2f M msg/s)\n",
               (t1 - t0) / MENSAJES, MENSAJES / ((t1 - t0) / 1e9) / 1e6);
        sil_arena_reset(&arena);
    }

    /* 4. MPSC: 4 productores, 1 consumidor. */
    {
        SilChannelMPSC *c = sil_channel_mpsc_crear(&arena, 4096);
        pthread_t h[PRODUCTORES];
        const unsigned total = PRODUCTORES * POR_PRODUCTOR;
        unsigned recibidos = 0;
        void *out = NULL;
        unsigned i;
        t0 = ahora_ns();
        for (i = 0; i < PRODUCTORES; i++) pthread_create(&h[i], NULL, productor_mpsc, c);
        while (recibidos < total) {
            if (sil_channel_mpsc_recibir(c, &out)) recibidos++;
        }
        for (i = 0; i < PRODUCTORES; i++) pthread_join(h[i], NULL);
        t1 = ahora_ns();
        printf("canal MPSC (4P/1C)     : %7.2f ns/msg (%6.2f M msg/s)\n",
               (t1 - t0) / total, total / ((t1 - t0) / 1e9) / 1e6);
    }

    sil_arena_destruir(&arena);
    return 0;
}
