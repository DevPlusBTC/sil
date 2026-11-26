#ifndef SIL_CHANNEL_H
#define SIL_CHANNEL_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "sil_arena.h"

/* Canal tipado de capacidad acotada, libre de bloqueos (algoritmo de anillo
 * con numero de secuencia por ranura, Vyukov).
 *
 * Propiedades (verificadas en sil-rt/tests/test_channel.c):
 *  - Memoria O(capacidad) y CERO malloc por mensaje: las ranuras se reservan
 *    una vez en la arena del canal.
 *  - Sin ABA: no hay reutilizacion de nodos con punteros, el progreso lo marca
 *    una secuencia monotona por ranura.
 *  - Un solo consumidor (SPSC y MPSC lo respetan) y N productores (MPSC).
 *  - `capacidad == 0` usa SIL_CANAL_CAPACIDAD_DEFECTO.
 *  - La capacidad se redondea a la potencia de dos superior.
 *  - `enviar` devuelve false si el canal esta lleno o cerrado (contrapresion);
 *    `recibir` sigue drenando lo pendiente despues de `cerrar`.
 */

#define SIL_CANAL_CAPACIDAD_DEFECTO 1024u
/* Cota superior defensiva: 2^24 mensajes * 16 B = 256 MiB. */
#define SIL_CANAL_CAPACIDAD_MAXIMA (1u << 24)

typedef struct SilCanalRanura {
    /* Secuencia: i al inicializar; pos+1 al publicar; pos+mask+1 al consumir. */
    volatile size_t secuencia;
    void *dato;
} SilCanalRanura;

typedef struct SilCanalNucleo {
    SilCanalRanura *ranuras; /* reservadas en la arena del canal */
    size_t capacidad;        /* potencia de dos */
    size_t mascara;          /* capacidad - 1 */
    volatile size_t cabeza;  /* indice del consumidor (monotono) */
    volatile size_t cola;    /* indice de los productores (monotono) */
    volatile size_t cerrado; /* 0 = abierto, 1 = cerrado (acceso atomico) */
} SilCanalNucleo;

/* Canal SPSC (un productor, un consumidor). */
typedef struct {
    SilCanalNucleo nucleo;
} SilChannel;

/* Canal MPSC (N productores, un consumidor). */
typedef struct {
    SilCanalNucleo nucleo;
} SilChannelMPSC;

SilChannel *sil_channel_crear(SilArena *arena, size_t capacidad);
void sil_channel_destruir(SilChannel *canal);
bool sil_channel_enviar(SilChannel *canal, void *mensaje);
bool sil_channel_recibir(SilChannel *canal, void **mensaje);
void sil_channel_cerrar(SilChannel *canal);
size_t sil_channel_contar(SilChannel *canal);
size_t sil_channel_capacidad(SilChannel *canal);
bool sil_channel_esta_vacio(SilChannel *canal);
bool sil_channel_esta_cerrado(SilChannel *canal);

/* Variantes MPSC: mismo anillo, la publicacion de la cola usa CAS. */
SilChannelMPSC *sil_channel_mpsc_crear(SilArena *arena, size_t capacidad);
void sil_channel_mpsc_destruir(SilChannelMPSC *canal);
bool sil_channel_mpsc_enviar(SilChannelMPSC *canal, void *mensaje);
bool sil_channel_mpsc_recibir(SilChannelMPSC *canal, void **mensaje);
void sil_channel_mpsc_cerrar(SilChannelMPSC *canal);
size_t sil_channel_mpsc_contar(SilChannelMPSC *canal);
size_t sil_channel_mpsc_capacidad(SilChannelMPSC *canal);
bool sil_channel_mpsc_esta_vacio(SilChannelMPSC *canal);
bool sil_channel_mpsc_esta_cerrado(SilChannelMPSC *canal);

#endif /* SIL_CHANNEL_H */
