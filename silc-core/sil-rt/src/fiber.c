/* sil_fiber.c - Fibras Cooperativas M7 (Windows-only).
 *
 * M7: implementacion real con primativas Win32 Fibers.
 * Stack dedicado de 64KB por fibra, scheduler round-robin single-thread.
 * sil_fibra_yield(): SwitchToFiber(scheduler) — sin syscall de kernel.
 *
 * Disciplina: magic numbers, validacion de punteros, cleanup en destruir.
 * Nota: path Unix (ucontext) no compatibilizado con MSVC; solo Win32.
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
/* Path MSVC: ucontext no disponible. Tipos placeholder. */
typedef struct ucontext_t ucontext_t;
#define getcontext(c) /* no-op */
#define swapcontext(o,n) /* no-op */
#define makecontext(c,f,a) /* no-op */
#endif

typedef enum {
    FIBRA_LISTA = 0,
    FIBRA_FINALIZADA = 1,
} FibraEstado_;

typedef struct FibraControl_ {
    void *ip;
    void *sp_arena;
    uint64_t estado;
    uint8_t cap_token[32];
    uint8_t _pad[8];
    void (*entrada)(void*);
    void *arg;
    unsigned magic;
    struct FibraControl_ *siguiente;
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
    /* Volver al scheduler. La fibra muere (no reusar stack). */
    SwitchToFiber(sched->sched_fiber);
    /* Inalcanzable. */
    abort();
}
#else
static void fibra_trampolin_unix(void) {
    /* No-op para MSVC: el scheduler round-robin no se cambia de contexto
     ucontext en esta plataforma. El yield se hace al thread principal. */
    (void)sched_actual_tls;
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
#else
    /* MSVC: scheduler en modo lite sin fibras Win32. */
    (void)n_hilos_os;
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
        /* Sin stack dedico: nothing explicit to free. */
#endif
        free(c);
        c = nx;
    }
    free(s);
}

bool sil_sched_spawn(SilScheduler *s, void (*entrada)(void*), void *arg) {
    if (!s || !entrada) return false;
    if (s->n_fibras >= SIL_FIBRA_MAX) return false;
    if (s->en_ejecucion) return false; /* No spawn durante ejecuci n (M7). */

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
    /* MSVC: no hay CreateFiber; registramos como lista para futuras extensiones. */
    (void)fc;
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
        /* Scan lineal: primera no-finalizada. O(n) por pick, n pequeno en M7. */
        FibraControl_ *elegida = 0;
        for (FibraControl_ *fc = s->cabeza; fc; fc = fc->siguiente) {
            if (fc->magic != SIL_FIBRA_MAGIC) break;
            if (fc->pub_.estado == FIBRA_LISTA) { elegida = fc; break; }
        }
        if (!elegida) break; /* Todas finalizadas. */

#if defined(_WIN32)
        SwitchToFiber(elegida->handle);
#else
        /* MSVC: yield al thread principal. */
        (void)elegida;
#endif
        /* Al volver: la fibra hizo yield (sigue LISTA? no: sigue no-finalizada)
         * o finalizó. Marcar como LISTA para re-pick si no finalizó.
         * En nuestro protocolo, las fibras no-yieldean a un estado distinto;
         * yield() solo cede (siguen LISTA hasta que trampolín las finaliza).
         * Para evitar re-ejecutar una fibra a medias (que con Win32 Fibers
         * SICorrecto se reanuda correctamente via SwitchToFiber), simplemente
         * continuamos el loop: si no finaliz, se volver a elegir. */
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
    /* MSVC: yield cooperativo al thread principal. */
    (void)s;
#endif
}