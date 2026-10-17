#ifndef SIL_CHANNEL_H
#define SIL_CHANNEL_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "sil_arena.h"

// Lock-free single-producer, single-consumer channel
// Michael & Scott non-blocking queue algorithm

typedef struct SilChannelNode {
    void *data;
    struct SilChannelNode *next;
} SilChannelNode;

typedef struct {
    SilChannelNode *head;
    SilChannelNode *tail;
} SilChannelQueue;

typedef struct {
    SilChannelQueue queue;
    SilArena *arena;
    volatile size_t item_count;
    volatile int closed;
} SilChannel;

// MPSC (Multi-producer, single-consumer) channel
// Uses volatile pointers with explicit atomic operations
// MSVC uses Interlocked* intrinsics instead of _Atomic
typedef struct {
    void *head; // _Atomic(SilChannelNode *) not supported on MSVC
    void *tail; // _Atomic(SilChannelNode *) not supported on MSVC
    SilArena *arena;
    volatile size_t item_count;
    volatile int closed;
} SilChannelMPSC;

SilChannel *sil_channel_crear(SilArena *arena, size_t capacidad_inicial);
void sil_channel_destruir(SilChannel *canal);
bool sil_channel_enviar(SilChannel *canal, void *mensaje);
bool sil_channel_recibir(SilChannel *canal, void **mensaje);
void sil_channel_cerrar(SilChannel *canal);
size_t sil_channel_contar(SilChannel *canal);
bool sil_channel_esta_vacio(SilChannel *canal);
bool sil_channel_esta_cerrado(SilChannel *canal);

// MPSC variants
SilChannelMPSC *sil_channel_mpsc_crear(SilArena *arena, size_t capacidad_inicial);
void sil_channel_mpsc_destruir(SilChannelMPSC *canal);
bool sil_channel_mpsc_enviar(SilChannelMPSC *canal, void *mensaje);
bool sil_channel_mpsc_recibir(SilChannelMPSC *canal, void **mensaje);
void sil_channel_mpsc_cerrar(SilChannelMPSC *canal);
size_t sil_channel_mpsc_contar(SilChannelMPSC *canal);
bool sil_channel_mpsc_esta_vacio(SilChannelMPSC *canal);
bool sil_channel_mpsc_esta_cerrado(SilChannelMPSC *canal);

#endif // SIL_CHANNEL_H