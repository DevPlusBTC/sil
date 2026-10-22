/* sil_fiber.c - Fibras Cooperativas M7 (Windows + POSIX).
 * Windows: Win32 Fibers.
 * POSIX (macOS, Linux): ucontext API.
 */

#include "sil_fiber.h"
#include <stdlib.h>
#include <string.h>

#define SIL_FIBRA_STACK_SIZE (64u * 1024u)
#define SIL_FIBRA_MAGIC 0xF18AA1F8u
#define SIL_FIBRA_MAX 100000u

#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
typedef LPVOID FibraHandle_;
#define FIBRA_NULA 0
#elif defined(__APPLE__) || defined(__linux__) || defined(__unix__)
#include <ucontext.h>
typedef ucontext_t* FibraHandle_;
#define FIBRA_NULA ((FibraHandle_)0)
#else
#error "Unsupported platform for fibers"
#endif

typedef enum {
    FIBRA_LISTA = 0,
    FIBRA_FINALIZADA = 1,
} FibraEstado_;

typedef struct FibraControl_ {
    SilFibra pub_;
    void (*entrada)(void*);
    void *arg;
    unsigned magic;
    struct FibraControl_ *siguiente;
    FibraHandle_ handle;
    ucontext_t ctx;          // only used on POSIX
    void *stack;             // stack memory for POSIX
} FibraControl_;

struct SilScheduler {
    FibraControl_ *cabeza;
    FibraControl_ *cola;
    FibraHandle_ sched_fiber;
    int hilo_convertido;
    unsigned n_fibras;
    unsigned finalizadas;
    unsigned en_ejecucion;
};

#if defined(_MSC_VER)
__declspec(thread) static FibraControl_ *fibra_actual_tls = 0;
__declspec(thread) static SilScheduler *sched_actual_tls = 0;
#else
static _Thread_local FibraControl_ *fibra_actual_tls = 0;
static _Thread_local SilScheduler *sched_actual_tls = 0;
#endif

/* Forward declarations */
static void fibra_trampolin_win(void *param);
static void fibra_trampolin_posix(void);

#if defined(_WIN32)
static void __stdcall fibra_trampolin_win(LPVOID param) {
    TrampolinArg_ *ta = (TrampolinArg_*)param;
    FibraControl_ *fc = ta->fc;
    SilScheduler *sched = ta->sched;
    free(ta);
    fc->entrada(fc->arg);
    fc->pub_.estado = FIBRA_FINALIZADA;
    sched->finalizadas++;
    SwitchToFiber(sched->sched_fiber);
    abort();
}
#else
static void fibra_trampolin_posix(void) {
    FibraControl_ *fc = fibra_actual_tls;
    SilScheduler *sched = sched_actual_tls;
    fc->entrada(fc->arg);
    fc->pub_.estado = FIBRA_FINALIZADA;
    sched->finalizadas++;
    /* return to scheduler */
    swapcontext(&fc->ctx, sched->sched_fiber);
}
#endif

typedef struct { FibraControl_ *fc; SilScheduler *sched; } TrampolinArg_;

SilScheduler* sil_sched_crear(unsigned n_hilos_os) {
    (void)n_hilos_os;
    SilScheduler *s = (SilScheduler*)calloc(1, sizeof(SilScheduler));
    if (!s) return 0;
#if defined(_WIN32)
    s->sched_fiber = ConvertThreadToFiber(0);
    if (!s->sched_fiber) {
        s->sched_fiber = GetCurrentFiber();
        if (!s->sched_fiber) { free(s); return 0; }
        s->hilo_convertido = 0;
    } else {
        s->hilo_convertido = 1;
    }
#else
    s->sched_fiber = (FibraHandle_)malloc(sizeof(ucontext_t));
    if (!s->sched_fiber) { free(s); return 0; }
    getcontext(s->sched_fiber);
    s->hilo_convertido = 0;
#endif
    return s;
}

void sil_sched_destruir(SilScheduler *s) {
    if (!s) return;
    FibraControl_ *c = s->cabeza;
    while (c) {
        FibraControl_ *nx = c->siguiente;
#if defined(_WIN32)
        if (c->handle) DeleteFiber(c->handle);
#else
        if (c->stack) free(c->stack);
        if (c->handle) free(c->handle);
#endif
        free(c);
        c = nx;
    }
#if !defined(_WIN32)
    if (s->sched_fiber) free(s->sched_fiber);
#endif
    free(s);
}

bool sil_sched_spawn(SilScheduler *s, void (*entrada)(void*), void *arg) {
    if (!s || !entrada) return false;
    if (s->n_fibras >= SIL_FIBRA_MAX) return false;
    if (s->en_ejecucion) return false;

    FibraControl_ *fc = (FibraControl_*)calloc(1, sizeof(FibraControl_));
    if (!fc) return false;
    fc->pub_.estado = FIBRA_LISTA;
    fc->entrada = entrada;
    fc->arg = arg;
    fc->magic = SIL_FIBRA_MAGIC;

#if defined(_WIN32)
    {
        TrampolinArg_ *ta = (TrampolinArg_*)malloc(sizeof(TrampolinArg_));
        if (!ta) { free(fc); return false; }
        ta->fc = fc;
        ta->sched = s;
        fc->handle = CreateFiber(SIL_FIBRA_STACK_SIZE, fibra_trampolin_win, ta);
        if (!fc->handle) { free(ta); free(fc); return false; }
    }
#else
    fc->stack = malloc(SIL_FIBRA_STACK_SIZE);
    if (!fc->stack) { free(fc); return false; }
    fc->handle = (FibraHandle_)malloc(sizeof(ucontext_t));
    if (!fc->handle) { free(fc->stack); free(fc); return false; }
    getcontext(fc->handle);
    fc->handle->uc_stack.ss_sp = fc->stack;
    fc->handle->uc_stack.ss_size = SIL_FIBRA_STACK_SIZE;
    fc->handle->uc_link = s->sched_fiber;
    makecontext(fc->handle, (void(*)())fibra_trampolin_posix, 0);
#endif

    fc->siguiente = 0;
    if (s->cola) s->cola->siguiente = fc;
    else s->cabeza = fc;
    s->cola = fc;
    s->n_fibras++;
    return true;
}

void sil_sched_ejecutar(SilScheduler *s) {
    if (!s || s->en_ejecucion) return;
    s->en_ejecucion = 1;
    sched_actual_tls = s;

    while (s->finalizadas < s->n_fibras) {
        FibraControl_ *elegida = 0;
        for (FibraControl_ *fc = s->cabeza; fc; fc = fc->siguiente) {
            if (fc->magic != SIL_FIBRA_MAGIC) break;
            if (fc->pub_.estado == FIBRA_LISTA) { elegida = fc; break; }
        }
        if (!elegida) break;

        fibra_actual_tls = elegida;
#if defined(_WIN32)
        SwitchToFiber(elegida->handle);
#else
        swapcontext(s->sched_fiber, elegida->handle);
#endif
    }

    sched_actual_tls = 0;
    s->en_ejecucion = 0;
}

void sil_fibra_yield(void) {
    SilScheduler *s = sched_actual_tls;
    if (!s) return;
#if defined(_WIN32)
    SwitchToFiber(s->sched_fiber);
#else
    FibraControl_ *fc = fibra_actual_tls;
    swapcontext(&fc->ctx, s->sched_fiber);
#endif
}