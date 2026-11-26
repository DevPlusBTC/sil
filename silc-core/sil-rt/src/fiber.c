/* sil_fiber.c - Fibras cooperativas del runtime nativo SIL.
 *
 * Windows: Win32 Fibers (ConvertThreadToFiber / CreateFiber / SwitchToFiber).
 * POSIX:   ucontext (getcontext / makecontext / swapcontext).
 *
 * Modelo de ejecucion (verificado en sil-rt/tests/test_fiber.c):
 *  - El scheduler del hilo es el contexto raiz; cada fibra guarda SU contexto
 *    en su propio bloque de control. Reanudar una fibra consiste en restaurar
 *    ese contexto, no el inicial: es lo que permite que `sil_fibra_yield`
 *    continue donde se quedo (el bug anterior reanudaba `makecontext`, por lo
 *    que cada yield reiniciaba la fibra -> bucle infinito).
 *  - Reparto round-robin real con cursor: ninguna fibra puede acaparar el
 *    scheduler aunque no ceda el control mientras el resto este lista.
 *  - Un scheduler por hilo; el estado del hilo actual vive en TLS portable.
 */

#include "sil_fiber.h"
#include <stdlib.h>
#include <string.h>

#define SIL_FIBRA_STACK_SIZE (64u * 1024u)
#define SIL_FIBRA_MAGIC 0xF18AA1F8u
#define SIL_FIBRA_MAX 100000u

#if defined(_MSC_VER)
#define SIL_TLS __declspec(thread)
#elif defined(__GNUC__)
#define SIL_TLS __thread
#else
#define SIL_TLS _Thread_local
#endif

#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
typedef LPVOID FibraHandle_;
#else
#define _XOPEN_SOURCE 700
#include <ucontext.h>
typedef ucontext_t *FibraHandle_;
#endif

typedef enum {
    FIBRA_LISTA = 0,
    FIBRA_FINALIZADA = 1,
} FibraEstado_;

typedef struct FibraControl_ {
    SilFibra pub_;
    void (*entrada)(void *);
    void *arg;
    unsigned magic;
    struct FibraControl_ *siguiente;
#if defined(_WIN32)
    FibraHandle_ handle;
#else
    ucontext_t ctx;   /* contexto propio: se reanuda desde aqui */
    void *stack;      /* pila de la fibra */
#endif
} FibraControl_;

struct SilScheduler {
    FibraControl_ *cabeza;
    FibraControl_ *cola;
    FibraControl_ *cursor;   /* siguiente fibra a considerar (round-robin) */
#if defined(_WIN32)
    FibraHandle_ sched_fiber;
    int hilo_convertido;
#else
    ucontext_t *sched_ctx;
#endif
    unsigned n_fibras;
    unsigned finalizadas;
    unsigned en_ejecucion;
};

static SIL_TLS FibraControl_ *fibra_actual_tls = 0;
static SIL_TLS SilScheduler *sched_actual_tls = 0;

#if defined(_WIN32)
typedef struct {
    FibraControl_ *fc;
    SilScheduler *sched;
} TrampolinArg_;

static VOID CALLBACK fibra_trampolin_win(LPVOID param) {
    TrampolinArg_ *ta = (TrampolinArg_ *)param;
    FibraControl_ *fc = ta ? ta->fc : 0;
    SilScheduler *sched = ta ? ta->sched : 0;
    free(ta);
    if (!fc || !sched) return;
    fc->entrada(fc->arg);
    fc->pub_.estado = FIBRA_FINALIZADA;
    sched->finalizadas++;
    SwitchToFiber(sched->sched_fiber);
    /* Inalcanzable: el scheduler no reanuda fibras finalizadas. */
}
#else
static void fibra_trampolin_posix(void) {
    FibraControl_ *fc = fibra_actual_tls;
    SilScheduler *sched = sched_actual_tls;
    if (!fc || !sched) return;
    fc->entrada(fc->arg);
    fc->pub_.estado = FIBRA_FINALIZADA;
    sched->finalizadas++;
    /* Volver al scheduler y no regresar nunca a este contexto. */
    swapcontext(&fc->ctx, sched->sched_ctx);
}
#endif

SilScheduler *sil_sched_crear(unsigned n_hilos_os) {
    SilScheduler *s;
    (void)n_hilos_os; /* el scheduler es por hilo; M:N esta pendiente */
    s = (SilScheduler *)calloc(1, sizeof(SilScheduler));
    if (!s) return 0;
#if defined(_WIN32)
    s->sched_fiber = ConvertThreadToFiber(0);
    if (s->sched_fiber) {
        s->hilo_convertido = 1;
    } else {
        s->sched_fiber = GetCurrentFiber();
        s->hilo_convertido = 0;
        if (!s->sched_fiber) {
            free(s);
            return 0;
        }
    }
#else
    s->sched_ctx = (ucontext_t *)malloc(sizeof(ucontext_t));
    if (!s->sched_ctx) {
        free(s);
        return 0;
    }
    memset(s->sched_ctx, 0, sizeof(*s->sched_ctx));
#endif
    return s;
}

void sil_sched_destruir(SilScheduler *s) {
    FibraControl_ *c;
    if (!s) return;
    c = s->cabeza;
    while (c) {
        FibraControl_ *nx = c->siguiente;
#if defined(_WIN32)
        if (c->handle) DeleteFiber(c->handle);
#else
        free(c->stack);
#endif
        free(c);
        c = nx;
    }
#if defined(_WIN32)
    if (s->hilo_convertido && s->sched_fiber) {
        ConvertFiberToThread();
    }
#else
    free(s->sched_ctx);
#endif
    free(s);
}

bool sil_sched_spawn(SilScheduler *s, void (*entrada)(void *), void *arg) {
    FibraControl_ *fc;
    if (!s || !entrada) return false;
    if (s->n_fibras >= SIL_FIBRA_MAX) return false;
    if (s->en_ejecucion) return false;

    fc = (FibraControl_ *)calloc(1, sizeof(FibraControl_));
    if (!fc) return false;
    fc->pub_.estado = FIBRA_LISTA;
    fc->entrada = entrada;
    fc->arg = arg;
    fc->magic = SIL_FIBRA_MAGIC;

#if defined(_WIN32)
    {
        TrampolinArg_ *ta = (TrampolinArg_ *)malloc(sizeof(TrampolinArg_));
        if (!ta) {
            free(fc);
            return false;
        }
        ta->fc = fc;
        ta->sched = s;
        fc->handle = CreateFiber(SIL_FIBRA_STACK_SIZE, fibra_trampolin_win, ta);
        if (!fc->handle) {
            free(ta);
            free(fc);
            return false;
        }
    }
#else
    fc->stack = malloc(SIL_FIBRA_STACK_SIZE);
    if (!fc->stack) {
        free(fc);
        return false;
    }
    if (getcontext(&fc->ctx) != 0) {
        free(fc->stack);
        free(fc);
        return false;
    }
    fc->ctx.uc_stack.ss_sp = fc->stack;
    fc->ctx.uc_stack.ss_size = SIL_FIBRA_STACK_SIZE;
    fc->ctx.uc_link = s->sched_ctx;
    makecontext(&fc->ctx, fibra_trampolin_posix, 0);
#endif

    fc->siguiente = 0;
    if (s->cola) {
        s->cola->siguiente = fc;
    } else {
        s->cabeza = fc;
        s->cursor = fc;
    }
    s->cola = fc;
    s->n_fibras++;
    return true;
}

/* Siguiente fibra lista en orden circular a partir del cursor. */
static FibraControl_ *siguiente_lista(SilScheduler *s) {
    FibraControl_ *inicio = s->cursor ? s->cursor : s->cabeza;
    FibraControl_ *fc = inicio;
    unsigned visitadas = 0;
    if (!inicio) return 0;
    do {
        if (fc->magic == SIL_FIBRA_MAGIC && fc->pub_.estado == FIBRA_LISTA) {
            s->cursor = fc->siguiente ? fc->siguiente : s->cabeza;
            return fc;
        }
        fc = fc->siguiente ? fc->siguiente : s->cabeza;
        visitadas++;
    } while (fc != inicio && visitadas <= s->n_fibras);
    return 0;
}

void sil_sched_ejecutar(SilScheduler *s) {
    if (!s || s->en_ejecucion) return;
    s->en_ejecucion = 1;
    sched_actual_tls = s;

    while (s->finalizadas < s->n_fibras) {
        FibraControl_ *elegida = siguiente_lista(s);
        if (!elegida) break;
        fibra_actual_tls = elegida;
#if defined(_WIN32)
        SwitchToFiber(elegida->handle);
#else
        if (swapcontext(s->sched_ctx, &elegida->ctx) != 0) break;
#endif
    }

    fibra_actual_tls = 0;
    sched_actual_tls = 0;
    s->en_ejecucion = 0;
}

void sil_fibra_yield(void) {
    SilScheduler *s = sched_actual_tls;
#if defined(_WIN32)
    if (!s) return;
    SwitchToFiber(s->sched_fiber);
#else
    FibraControl_ *fc = fibra_actual_tls;
    if (!s || !fc) return;
    swapcontext(&fc->ctx, s->sched_ctx);
#endif
}
