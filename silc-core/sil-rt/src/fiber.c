/* sil_fiber.c - Fibras cooperativas + scheduler round-robin (M7).
 *
 * M7: implementacion real con primitivas OS:
 * - Windows: Win32 Fibers (CreateFiber/SwitchToFiber/DeleteFiber).
 * - Unix: ucontext (getcontext/makecontext/swapcontext).
 * - Stack dedicado 64KB por fibra, scheduler round-robin single-thread.
 * - sil_fibra_yield(): SwitchToFiber(scheduler) — sin syscall de kernel.
 *
 * Disciplina: magic numbers, validacion de punteros, cleanup en destruir.
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
#else
#include <ucontext.h>
#include <sys/mman.h>
#endif

typedef enum {
    FIBRA_LISTA = 0,
    FIBRA_FINALIZADA = 1,
} FibraEstado_;

typedef struct FibraControl_ {
    SilFibra pub_;              /* Header público de 64B */
#if defined(_WIN32)
    FibraHandle_ handle;        /* Win32 fiber */
#else
    ucontext_t ctx;             /* Unix context */
    unsigned char *stack;       /* Stack dedicado */
#endif
    void (*entrada)(void*);     /* Función de entrada */
    void *arg;                  /* Argumento */
    unsigned magic;
    struct FibraControl_ *siguiente;
} FibraControl_;

struct SilScheduler {
    FibraControl_ *cabeza;
    FibraControl_ *cola;
#if defined(_WIN32)
    FibraHandle_ sched_fiber;   /* Fibra del scheduler (hilo convertido) */
    int hilo_convertido;
#else
    ucontext_t sched_ctx;
#endif
    unsigned n_fibras;
    unsigned finalizadas;
    unsigned en_ejecucion;      /* Guard contra re-entrada */
};

#if defined(_MSC_VER)
__declspec(thread) static FibraControl_ *fibra_actual_tls = 0;
__declspec(thread) static SilScheduler *sched_actual_tls = 0;
#else
static _Thread_local FibraControl_ *fibra_actual_tls = 0;
static _Thread_local SilScheduler *sched_actual_tls = 0;
#endif

/* Forward: trampolín de fibra. */
static void fibra_trampolin_param(void *param);

typedef struct { FibraControl_ *fc; SilScheduler *sched; } TrampolinArg_;

#if defined(_WIN32)
static void __stdcall fibra_trampolin_win(LPVOID param) {
    TrampolinArg_ *ta = (TrampolinArg_*)param;
    FibraControl_ *fc = ta->fc;
    SilScheduler *sched = ta->sched;
    free(ta);
    fc->entrada(fc->arg);
    fc->pub_.estado = FIBRA_FINALIZADA;
    sched->finalizadas++;
    /* Volver al scheduler. La fibra muere aquí (no reusar su stack). */
    SwitchToFiber(sched->sched_fiber);
    /* Inalcanzable. */
    abort();
}
#else
static void fibra_trampolin_unix(void) {
    /* Recuperamos fc/sched vía TLS (establecido antes del swap). */
    FibraControl_ *fc = fibra_actual_tls;
    SilScheduler *sched = sched_actual_tls;
    if (!fc || !sched) return;
    fc->entrada(fc->arg);
    fc->pub_.estado = FIBRA_FINALIZADA;
    sched->finalizadas++;
    /* Volver al scheduler. */
    FibraControl_ *me = fc;
    fibra_actual_tls = 0;
    swapcontext(&me->ctx, &sched->sched_ctx);
}
static void fibra_trampolin_param(void *param) {
    (void)param;
    fibra_trampolin_unix();
}
#endif

SilScheduler* sil_sched_crear(unsigned n_hilos_os) {
    (void)n_hilos_os;
    SilScheduler *s = (SilScheduler*)calloc(1, sizeof(SilScheduler));
    if (!s) return 0;
#if defined(_WIN32)
    /* Convertir el hilo actual en fibra (necesario para SwitchToFiber). */
    s->sched_fiber = ConvertThreadToFiber(0);
    if (!s->sched_fiber) {
        /* Si ya era fibra (ERROR_ALREADY_FIBER = 0x4E8), obtener la actual. */
        s->sched_fiber = GetCurrentFiber();
        if (!s->sched_fiber) { free(s); return 0; }
        s->hilo_convertido = 0;
    } else {
        s->hilo_convertido = 1;
    }
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
        free(c->stack);
#endif
        free(c);
        c = nx;
    }
#if defined(_WIN32)
    if (s->hilo_convertido && s->sched_fiber) {
        ConvertFiberToThread();
    }
#endif
    free(s);
}

bool sil_sched_spawn(SilScheduler *s, void (*entrada)(void*), void *arg) {
    if (!s || !entrada) return false;
    if (s->n_fibras >= SIL_FIBRA_MAX) return false;
    if (s->en_ejecucion) return false; /* No spawn durante ejecución (M7). */

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
    fc->stack = (unsigned char*)malloc(SIL_FIBRA_STACK_SIZE);
    if (!fc->stack) { free(fc); return false; }
    if (getcontext(&fc->ctx) != 0) { free(fc->stack); free(fc); return false; }
    fc->ctx.uc_stack.ss_sp = fc->stack;
    fc->ctx.uc_stack.ss_size = SIL_FIBRA_STACK_SIZE;
    fc->ctx.uc_link = 0;
    makecontext(&fc->ctx, fibra_trampolin_unix, 0);
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

    /* Round-robin hasta que todas finalicen. */
    while (s->finalizadas < s->n_fibras) {
        /* Scan lineal: primera no-finalizada. O(n) por pick, n pequeño en M7. */
        FibraControl_ *elegida = 0;
        for (FibraControl_ *fc = s->cabeza; fc; fc = fc->siguiente) {
            if (fc->magic != SIL_FIBRA_MAGIC) break;
            if (fc->pub_.estado == FIBRA_LISTA) { elegida = fc; break; }
        }
        if (!elegida) break; /* Todas finalizadas. */

        fibra_actual_tls = elegida;
#if defined(_WIN32)
        SwitchToFiber(elegida->handle);
#else
        swapcontext(&s->sched_ctx, &elegida->ctx);
#endif
        fibra_actual_tls = 0;
        /* Al volver: la fibra hizo yield (sigue LISTA? no: sigue no-finalizada)
         * o finalizó. Marcar como LISTA para re-pick si no finalizó.
         * En nuestro protocolo, las fibras no-yieldean a un estado distinto;
         * yield() solo cede (siguen LISTA hasta que trampolín las finaliza).
         * Para evitar re-ejecutar una fibra a medias (que con Win32 Fibers
         * SÍ se reanuda correctamente vía SwitchToFiber), simplemente
         * continuamos el loop: si no finalizó, se volverá a elegir. */
    }

    sched_actual_tls = 0;
    s->en_ejecucion = 0;
}

void sil_fibra_yield(void) {
    SilScheduler *s = sched_actual_tls;
    FibraControl_ *fc = fibra_actual_tls;
    if (!s || !fc) return;
    if (fc->magic != SIL_FIBRA_MAGIC) return;
#if defined(_WIN32)
    SwitchToFiber(s->sched_fiber);
#else
    swapcontext(&fc->ctx, &s->sched_ctx);
#endif
}
