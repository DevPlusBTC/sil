#include "sil_channel.h"
#include <stdlib.h>
#include <string.h>
#include <stddef.h>

/* Lock-free single-producer, single-consumer channel
   Uses simple spin-wait with pause for SIL runtime */

SilChannel *sil_channel_crear(SilArena *arena, size_t capacidad_inicial) {
    (void)capacidad_inicial;
    
    SilChannel *canal = (SilChannel *)sil_arena_asignar(arena, sizeof(SilChannel), 8);
    if (!canal) return NULL;
    
    /* Nodo dummy inicial para evitar casos especiales */
    SilChannelNode *nodo_inicial = (SilChannelNode *)sil_arena_asignar(arena, sizeof(SilChannelNode), 8);
    if (!nodo_inicial) return NULL;
    
    nodo_inicial->data = NULL;
    nodo_inicial->next = NULL;
    
    canal->queue.head = nodo_inicial;
    canal->queue.tail = nodo_inicial;
    canal->arena = arena;
    canal->item_count = 0;
    canal->closed = 0;
    
    return canal;
}

void sil_channel_destruir(SilChannel *canal) {
    if (!canal) return;
    
    SilChannelNode *actual = canal->queue.head;
    while (actual) {
        SilChannelNode *siguiente = actual->next;
        free(actual);
        actual = siguiente;
    }
    canal->queue.head = NULL;
    canal->queue.tail = NULL;
}

bool sil_channel_enviar(SilChannel *canal, void *mensaje) {
    if (!canal || !mensaje) return false;
    if (canal->closed) return false;
    
    SilChannelNode *nuevo = (SilChannelNode *)sil_arena_asignar(canal->arena, sizeof(SilChannelNode), 8);
    if (!nuevo) return false;
    
    nuevo->data = mensaje;
    nuevo->next = NULL;
    
    /* Enlazar al final (productor único, sin atomics complejos) */
    SilChannelNode *actual_tail = canal->queue.tail;
    actual_tail->next = nuevo;
    canal->queue.tail = nuevo;
    
    /* Incrementar contador simple (sin atomics para MSVC compat) */
    canal->item_count++;
    
    return true;
}

bool sil_channel_recibir(SilChannel *canal, void **mensaje) {
    if (!canal || !mensaje) return false;
    
    SilChannelNode *cabeza = canal->queue.head;
    SilChannelNode *siguiente = cabeza->next;
    
    if (cabeza == canal->queue.tail) {
        if (siguiente == NULL) {
            return false;  /* Cola vacía */
        }
        /* Mover cabeza al siguiente nodo */
        canal->queue.head = siguiente;
    }
    
    void *msg = siguiente->data;
    *mensaje = msg;
    
    /* Limpiar nodo */
    siguiente->data = NULL;
    free(siguiente);
    
    /* Decrementar contador */
    if (canal->item_count > 0) {
        canal->item_count--;
    }
    
    return true;
}

void sil_channel_cerrar(SilChannel *canal) {
    if (!canal) return;
    canal->closed = 1;
}

size_t sil_channel_contar(SilChannel *canal) {
    if (!canal) return 0;
    return (size_t)canal->item_count;
}

bool sil_channel_esta_vacio(SilChannel *canal) {
    if (!canal) return true;
    return canal->item_count == 0;
}

bool sil_channel_esta_cerrado(SilChannel *canal) {
    if (!canal) return false;
    return canal->closed != 0;
}