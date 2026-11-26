/* sil_channel.c - Canales tipados de capacidad acotada, libres de bloqueos.
 *
 * Implementacion: anillo de ranuras con numero de secuencia por ranura
 * (D. Vyukov, "Bounded MPMC queue"). Se evita el patron anterior de lista
 * enlazada (que ademas hacia CAS con el operando esperado equivocado, lo que
 * provocaba un SEGV en el primer envio) y se elimina el malloc por mensaje:
 * todas las ranuras viven en la arena del canal.
 *
 * Modelo de memoria (C11 / builtins __atomic de GCC y Clang):
 *   el productor escribe `dato` y publica con una store release de `secuencia`;
 *   el consumidor hace load acquire de `secuencia` antes de leer `dato`. El
 *   avance de `cola` (productores) y `cabeza` (consumidor) usa release; en el
 *   caso MPSC la reserva de posicion usa CAS acq_rel.
 *
 * Un unico consumidor en ambos canales es parte del contrato: es lo que
 * permite consumir sin CAS.
 */

#include "sil_channel.h"
#include <stdint.h>
#include <string.h>

#if defined(_MSC_VER)
#include <intrin.h>

static size_t carga_size(volatile size_t *p) {
#if defined(_WIN64)
    return (size_t)_InterlockedCompareExchange64((volatile __int64 *)p, 0, 0);
#else
    return (size_t)_InterlockedCompareExchange((volatile long *)p, 0, 0);
#endif
}
static void almacena_size(volatile size_t *p, size_t v) {
#if defined(_WIN64)
    _InterlockedExchange64((volatile __int64 *)p, (__int64)v);
#else
    _InterlockedExchange((volatile long *)p, (long)v);
#endif
}
static int cas_size(volatile size_t *p, size_t *esperado, size_t nuevo) {
#if defined(_WIN64)
    size_t visto = (size_t)_InterlockedCompareExchange64(
        (volatile __int64 *)p, (__int64)nuevo, (__int64)*esperado);
#else
    size_t visto = (size_t)_InterlockedCompareExchange(
        (volatile long *)p, (long)nuevo, (long)*esperado);
#endif
    if (visto == *esperado) return 1;
    *esperado = visto;
    return 0;
}
#else
/* GCC/Clang: builtins __atomic, disponibles tambien con -std=c99 (sin
 * depender de <stdatomic.h> ni del calificador _Atomic). */
static size_t carga_size(volatile size_t *p) {
    return __atomic_load_n((size_t *)p, __ATOMIC_ACQUIRE);
}
static void almacena_size(volatile size_t *p, size_t v) {
    __atomic_store_n((size_t *)p, v, __ATOMIC_RELEASE);
}
static int cas_size(volatile size_t *p, size_t *esperado, size_t nuevo) {
    return __atomic_compare_exchange_n((size_t *)p, esperado, nuevo, 1,
                                       __ATOMIC_ACQ_REL, __ATOMIC_ACQUIRE);
}
#endif

static size_t redondear_potencia_dos(size_t v) {
    size_t p = 1;
    while (p < v) p <<= 1;
    return p;
}

/* Reserva el nucleo y las ranuras en la arena del canal. */
static int canal_inicializar(SilCanalNucleo *c, SilArena *arena, size_t capacidad) {
    size_t i;
    SilCanalRanura *ranuras;
    size_t bytes;

    if (!c || !arena) return 0;
    if (capacidad == 0) capacidad = SIL_CANAL_CAPACIDAD_DEFECTO;
    if (capacidad > SIL_CANAL_CAPACIDAD_MAXIMA) return 0;
    capacidad = redondear_potencia_dos(capacidad);
    /* capacidad <= 2^24 -> el producto no desborda size_t en 32/64 bits. */
    bytes = capacidad * sizeof(SilCanalRanura);
    if (bytes / sizeof(SilCanalRanura) != capacidad) return 0;

    ranuras = (SilCanalRanura *)sil_arena_asignar(arena, bytes, 64);
    if (!ranuras) return 0;
    if (((uintptr_t)ranuras & 63u) != 0) return 0; /* alineacion de linea de cache */

    for (i = 0; i < capacidad; i++) {
        ranuras[i].secuencia = i;
        ranuras[i].dato = NULL;
    }
    c->ranuras = ranuras;
    c->capacidad = capacidad;
    c->mascara = capacidad - 1;
    c->cabeza = 0;
    c->cola = 0;
    c->cerrado = 0;
    return 1;
}

/* Productor unico: no necesita CAS para reservar posicion. */
static int canal_encolar_spsc(SilCanalNucleo *c, void *dato) {
    size_t pos = carga_size(&c->cola);
    SilCanalRanura *r = &c->ranuras[pos & c->mascara];
    if ((ptrdiff_t)(carga_size(&r->secuencia) - pos) != 0) return 0; /* lleno */
    r->dato = dato;
    almacena_size(&r->secuencia, pos + 1);
    almacena_size(&c->cola, pos + 1);
    return 1;
}

/* N productores: CAS sobre `cola` para reservar la ranura. */
static int canal_encolar_mpsc(SilCanalNucleo *c, void *dato) {
    size_t pos = carga_size(&c->cola);
    for (;;) {
        SilCanalRanura *r = &c->ranuras[pos & c->mascara];
        ptrdiff_t dif = (ptrdiff_t)(carga_size(&r->secuencia) - pos);
        if (dif == 0) {
            if (cas_size(&c->cola, &pos, pos + 1)) {
                r->dato = dato;
                almacena_size(&r->secuencia, pos + 1);
                return 1;
            }
            /* cas_size deja en `pos` el valor observado: reintentar. */
        } else if (dif < 0) {
            return 0; /* lleno */
        } else {
            pos = carga_size(&c->cola);
        }
    }
}

/* Consumidor unico: reserva la posicion sin CAS. */
static int canal_desencolar(SilCanalNucleo *c, void **dato) {
    size_t pos = carga_size(&c->cabeza);
    SilCanalRanura *r = &c->ranuras[pos & c->mascara];
    if ((ptrdiff_t)(carga_size(&r->secuencia) - (pos + 1)) != 0) return 0; /* vacio */
    *dato = r->dato;
    almacena_size(&r->secuencia, pos + c->mascara + 1);
    almacena_size(&c->cabeza, pos + 1);
    return 1;
}

static size_t canal_contar(const SilCanalNucleo *c) {
    size_t cabeza, cola;
    if (!c) return 0;
    cabeza = carga_size((volatile size_t *)&c->cabeza);
    cola = carga_size((volatile size_t *)&c->cola);
    return cola > cabeza ? cola - cabeza : 0;
}

static void canal_cerrar(SilCanalNucleo *c) {
    if (c) almacena_size(&c->cerrado, 1);
}

static int canal_esta_cerrado(const SilCanalNucleo *c) {
    return c ? carga_size((volatile size_t *)&c->cerrado) != 0 : 0;
}

/* ------------------------------------------------------------------ SPSC --- */

SilChannel *sil_channel_crear(SilArena *arena, size_t capacidad) {
    SilChannel *canal = arena ? (SilChannel *)sil_arena_asignar(arena, sizeof(SilChannel), 64) : NULL;
    if (!canal) return NULL;
    if (!canal_inicializar(&canal->nucleo, arena, capacidad)) return NULL;
    return canal;
}

void sil_channel_destruir(SilChannel *canal) {
    /* La memoria es de la arena: basta con cerrar el canal (drenable) para que
     * ningun envio nuevo lo use. */
    if (!canal) return;
    canal_cerrar(&canal->nucleo);
}

bool sil_channel_enviar(SilChannel *canal, void *mensaje) {
    if (!canal || !mensaje) return false;
    if (canal_esta_cerrado(&canal->nucleo)) return false;
    return canal_encolar_spsc(&canal->nucleo, mensaje) != 0;
}

bool sil_channel_recibir(SilChannel *canal, void **mensaje) {
    if (!canal || !mensaje) return false;
    return canal_desencolar(&canal->nucleo, mensaje) != 0;
}

void sil_channel_cerrar(SilChannel *canal) { if (canal) canal_cerrar(&canal->nucleo); }

size_t sil_channel_contar(SilChannel *canal) { return canal ? canal_contar(&canal->nucleo) : 0; }

size_t sil_channel_capacidad(SilChannel *canal) { return canal ? canal->nucleo.capacidad : 0; }

bool sil_channel_esta_vacio(SilChannel *canal) { return canal ? canal_contar(&canal->nucleo) == 0 : true; }

bool sil_channel_esta_cerrado(SilChannel *canal) { return canal ? canal_esta_cerrado(&canal->nucleo) != 0 : false; }

/* ------------------------------------------------------------------ MPSC --- */

SilChannelMPSC *sil_channel_mpsc_crear(SilArena *arena, size_t capacidad) {
    SilChannelMPSC *canal =
        arena ? (SilChannelMPSC *)sil_arena_asignar(arena, sizeof(SilChannelMPSC), 64) : NULL;
    if (!canal) return NULL;
    if (!canal_inicializar(&canal->nucleo, arena, capacidad)) return NULL;
    return canal;
}

void sil_channel_mpsc_destruir(SilChannelMPSC *canal) {
    if (!canal) return;
    canal_cerrar(&canal->nucleo);
}

bool sil_channel_mpsc_enviar(SilChannelMPSC *canal, void *mensaje) {
    if (!canal || !mensaje) return false;
    if (canal_esta_cerrado(&canal->nucleo)) return false;
    return canal_encolar_mpsc(&canal->nucleo, mensaje) != 0;
}

bool sil_channel_mpsc_recibir(SilChannelMPSC *canal, void **mensaje) {
    if (!canal || !mensaje) return false;
    return canal_desencolar(&canal->nucleo, mensaje) != 0;
}

void sil_channel_mpsc_cerrar(SilChannelMPSC *canal) { if (canal) canal_cerrar(&canal->nucleo); }

size_t sil_channel_mpsc_contar(SilChannelMPSC *canal) {
    return canal ? canal_contar(&canal->nucleo) : 0;
}

size_t sil_channel_mpsc_capacidad(SilChannelMPSC *canal) {
    return canal ? canal->nucleo.capacidad : 0;
}

bool sil_channel_mpsc_esta_vacio(SilChannelMPSC *canal) {
    return canal ? canal_contar(&canal->nucleo) == 0 : true;
}

bool sil_channel_mpsc_esta_cerrado(SilChannelMPSC *canal) {
    return canal ? canal_esta_cerrado(&canal->nucleo) != 0 : false;
}
