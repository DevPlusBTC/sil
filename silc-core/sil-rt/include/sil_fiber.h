#ifndef SIL_FIBER_H
#define SIL_FIBER_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

/* Fibra stackless: 64 bytes exactos (1 línea caché L1). */
#ifdef _MSC_VER
__declspec(align(64))
#endif
typedef struct
#ifdef __GNUC__
__attribute__((aligned(64)))
#endif
{
    void *ip;
    void *sp_arena;
    uint64_t estado;
    uint8_t cap_token[32];
    uint8_t _pad[8];
} SilFibra;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L && !defined(_MSC_VER)
_Static_assert(sizeof(SilFibra) == 64, "Fibra debe ser exactamente 64 bytes");
#endif

typedef struct SilScheduler SilScheduler;
SilScheduler* sil_sched_crear(unsigned n_hilos_os);
void sil_sched_destruir(SilScheduler *s);
bool sil_sched_spawn(SilScheduler *s, void (*entrada)(void*), void *arg);
void sil_sched_ejecutar(SilScheduler *s);
void sil_fibra_yield(void);

#endif
