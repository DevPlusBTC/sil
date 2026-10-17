#include "sil_channel.h"
#include <stdlib.h>
#include <string.h>
#include <stddef.h>

#if defined(_MSC_VER)
#include <intrin.h>
#pragma intrinsic(_InterlockedCompareExchangePointer)
#pragma intrinsic(_InterlockedExchangePointer)
#pragma intrinsic(_InterlockedIncrement64)
#pragma intrinsic(_InterlockedDecrement64)
#pragma intrinsic(_InterlockedExchangeAdd64)
#pragma intrinsic(_InterlockedExchange)
#define ATOMIC_LOAD(ptr) (*(volatile void **)(ptr))
#define ATOMIC_STORE(ptr, val) (*(volatile void **)(ptr) = (val))
#define ATOMIC_CAS(ptr, old, new) (_InterlockedCompareExchangePointer((volatile void **)(ptr), (void *)(new), (void *)(old)) == (void *)(old))
#define ATOMIC_EXCHANGE(ptr, val) _InterlockedExchangePointer((volatile void **)(ptr), (void *)(val))
#define ATOMIC_FETCH_ADD(ptr, val) _InterlockedExchangeAdd64((volatile long long *)(ptr), (val))
#define ATOMIC_FETCH_SUB(ptr, val) _InterlockedExchangeAdd64((volatile long long *)(ptr), -(val))
#define ATOMIC_EXCHANGE_INT(ptr, val) _InterlockedExchange((volatile long *)(ptr), (long)(val))
#else
#include <stdatomic.h>
#define ATOMIC_LOAD(ptr) atomic_load_explicit((_Atomic(void *)*)(ptr), memory_order_acquire)
#define ATOMIC_STORE(ptr, val) atomic_store_explicit((_Atomic(void *)*)(ptr), (val), memory_order_release)
#define ATOMIC_CAS(ptr, old, new) atomic_compare_exchange_weak_explicit((_Atomic(void *)*)(ptr), (void **)(old), (void *)(new), memory_order_release, memory_order_relaxed)
#define ATOMIC_EXCHANGE(ptr, val) atomic_exchange_explicit((_Atomic(void *)*)(ptr), (val), memory_order_acq_rel)
#define ATOMIC_FETCH_ADD(ptr, val) atomic_fetch_add_explicit((_Atomic(size_t)*)(ptr), (val), memory_order_release)
#define ATOMIC_FETCH_SUB(ptr, val) atomic_fetch_sub_explicit((_Atomic(size_t)*)(ptr), (val), memory_order_release)
#define ATOMIC_EXCHANGE_INT(ptr, val) atomic_exchange_explicit((_Atomic(int)*)(ptr), (val), memory_order_acq_rel)
#endif

/* Lock-free single-producer, single-consumer channel
   Michael & Scott non-blocking queue algorithm
   Uses C11 atomics on GCC/Clang, Interlocked intrinsics on MSVC */

SilChannel *sil_channel_crear(SilArena *arena, size_t capacidad_inicial) {
    (void)capacidad_inicial;
    
    SilChannel *canal = (SilChannel *)sil_arena_asignar(arena, sizeof(SilChannel), 8);
    if (!canal) return NULL;
    
    /* Nodo dummy inicial para evitar casos especiales */
    SilChannelNode *nodo_inicial = (SilChannelNode *)sil_arena_asignar(arena, sizeof(SilChannelNode), 8);
    if (!nodo_inicial) return NULL;
    
    nodo_inicial->data = NULL;
    nodo_inicial->next = NULL;
    
    ATOMIC_STORE(&canal->queue.head, nodo_inicial);
    ATOMIC_STORE(&canal->queue.tail, nodo_inicial);
    canal->arena = arena;
    canal->item_count = 0;
    canal->closed = 0;
    
    return canal;
}

void sil_channel_destruir(SilChannel *canal) {
    if (!canal) return;
    
    SilChannelNode *actual = (SilChannelNode *)ATOMIC_LOAD(&canal->queue.head);
    while (actual) {
        SilChannelNode *siguiente = actual->next;
        /* Nodos se liberan via arena, no free() */
        actual = siguiente;
    }
    ATOMIC_STORE(&canal->queue.head, NULL);
    ATOMIC_STORE(&canal->queue.tail, NULL);
}

bool sil_channel_enviar(SilChannel *canal, void *mensaje) {
    if (!canal || !mensaje) return false;
    if (canal->closed) return false;
    
    SilChannelNode *nuevo = (SilChannelNode *)sil_arena_asignar(canal->arena, sizeof(SilChannelNode), 8);
    if (!nuevo) return false;
    
    nuevo->data = mensaje;
    nuevo->next = NULL;
    
    /* Michael & Scott lock-free enqueue:
       1. CAS tail->next from NULL to new node
       2. CAS tail from old tail to new node */
    while (1) {
        SilChannelNode *tail = (SilChannelNode *)ATOMIC_LOAD(&canal->queue.tail);
        SilChannelNode *next = tail->next;
        
        if (next != NULL) {
            /* Tail está adelantado, ayudar a avanzarlo */
            ATOMIC_CAS(&canal->queue.tail, tail, next);
            continue;
        }
        
        /* Intentar enlazar el nuevo nodo */
        if (ATOMIC_CAS(&tail->next, next, nuevo)) {
            /* Enlazado exitoso, ahora avanzar tail */
            ATOMIC_CAS(&canal->queue.tail, tail, nuevo);
            break;
        }
        /* CAS falló, reintentar */
    }
    
    ATOMIC_FETCH_ADD(&canal->item_count, 1);
    return true;
}

bool sil_channel_recibir(SilChannel *canal, void **mensaje) {
    if (!canal || !mensaje) return false;
    
    while (1) {
        SilChannelNode *head = (SilChannelNode *)ATOMIC_LOAD(&canal->queue.head);
        SilChannelNode *tail = (SilChannelNode *)ATOMIC_LOAD(&canal->queue.tail);
        SilChannelNode *next = head->next;
        
        if (head == tail) {
            if (next == NULL) {
                return false;  /* Cola vacía */
            }
            /* Tail se quedó atrás, ayudar a avanzarlo */
            ATOMIC_CAS(&canal->queue.tail, tail, next);
            continue;
        }
        
        /* Obtener el mensaje del siguiente nodo */
        void *msg = next->data;
        if (msg == NULL) {
            return false;
        }
        
        /* Intentar avanzar head */
        if (ATOMIC_CAS(&canal->queue.head, head, next)) {
            *mensaje = msg;
            /* Nodo dummy viejo se deja para arena cleanup */
            ATOMIC_FETCH_SUB(&canal->item_count, 1);
            return true;
        }
        /* CAS falló, reintentar */
    }
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

/* MPSC (Multi-Producer, Single-Consumer) Channel
   Based on Dmitry Vyukov's MPSC queue algorithm */

SilChannelMPSC *sil_channel_mpsc_crear(SilArena *arena, size_t capacidad_inicial) {
    (void)capacidad_inicial;
    
    SilChannelMPSC *canal = (SilChannelMPSC *)sil_arena_asignar(arena, sizeof(SilChannelMPSC), 8);
    if (!canal) return NULL;
    
    /* Nodo dummy inicial */
    SilChannelNode *nodo_inicial = (SilChannelNode *)sil_arena_asignar(arena, sizeof(SilChannelNode), 8);
    if (!nodo_inicial) return NULL;
    
    nodo_inicial->data = NULL;
    nodo_inicial->next = NULL;
    
    ATOMIC_STORE(&canal->head, nodo_inicial);
    ATOMIC_STORE(&canal->tail, nodo_inicial);
    canal->arena = arena;
    canal->item_count = 0;
    canal->closed = 0;
    
    return canal;
}

void sil_channel_mpsc_destruir(SilChannelMPSC *canal) {
    if (!canal) return;
    
    SilChannelNode *actual = (SilChannelNode *)ATOMIC_LOAD(&canal->head);
    while (actual) {
        SilChannelNode *siguiente = actual->next;
        actual = siguiente;
    }
    ATOMIC_STORE(&canal->head, NULL);
    ATOMIC_STORE(&canal->tail, NULL);
}

bool sil_channel_mpsc_enviar(SilChannelMPSC *canal, void *mensaje) {
    if (!canal || !mensaje) return false;
    if (canal->closed) return false;
    
    SilChannelNode *nuevo = (SilChannelNode *)sil_arena_asignar(canal->arena, sizeof(SilChannelNode), 8);
    if (!nuevo) return false;
    
    nuevo->data = mensaje;
    nuevo->next = NULL;
    
    /* Vyukov MPSC enqueue:
       1. Atomically swap tail with new node (returns old tail)
       2. Link old tail->next to new node */
    SilChannelNode *prev_tail = (SilChannelNode *)ATOMIC_EXCHANGE(&canal->tail, nuevo);
    
    prev_tail->next = nuevo;
    ATOMIC_FETCH_ADD(&canal->item_count, 1);
    
    return true;
}

bool sil_channel_mpsc_recibir(SilChannelMPSC *canal, void **mensaje) {
    if (!canal || !mensaje) return false;
    
    SilChannelNode *head = (SilChannelNode *)ATOMIC_LOAD(&canal->head);
    SilChannelNode *next = head->next;
    
    if (next == NULL) {
        return false;  /* Cola vacía */
    }
    
    void *msg = next->data;
    if (msg == NULL) {
        return false;
    }
    
    /* Avanzar head al siguiente nodo */
    ATOMIC_STORE(&canal->head, next);
    
    *mensaje = msg;
    ATOMIC_FETCH_SUB(&canal->item_count, 1);
    return true;
}

void sil_channel_mpsc_cerrar(SilChannelMPSC *canal) {
    if (!canal) return;
    canal->closed = 1;
}

size_t sil_channel_mpsc_contar(SilChannelMPSC *canal) {
    if (!canal) return 0;
    return (size_t)canal->item_count;
}

bool sil_channel_mpsc_esta_vacio(SilChannelMPSC *canal) {
    if (!canal) return true;
    return canal->item_count == 0;
}

bool sil_channel_mpsc_esta_cerrado(SilChannelMPSC *canal) {
    if (!canal) return false;
    return canal->closed != 0;
}