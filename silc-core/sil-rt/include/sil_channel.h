#ifndef SIL_CHANNEL_H
#define SIL_CHANNEL_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "sil_arena.h"

// Lock-free single-producer, single-consumer channel
// Michael & Scott non-blocking queue algorithm (simplified for SIL)

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

SilChannel *sil_channel_crear(SilArena *arena, size_t capacidad_inicial);
void sil_channel_destruir(SilChannel *canal);
bool sil_channel_enviar(SilChannel *canal, void *mensaje);
bool sil_channel_recibir(SilChannel *canal, void **mensaje);
void sil_channel_cerrar(SilChannel *canal);
size_t sil_channel_contar(SilChannel *canal);
bool sil_channel_esta_vacio(SilChannel *canal);
bool sil_channel_esta_cerrado(SilChannel *canal);

#endif // SIL_CHANNEL_H