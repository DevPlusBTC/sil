/* sil_fiber.c - Fibras stackless + scheduler M:N. M0: stubs que compilan. */
#include "sil_fiber.h"
#include <stdlib.h>

struct SilScheduler { unsigned n_hilos; };

SilScheduler* sil_sched_crear(unsigned n_hilos_os) {
    SilScheduler *s = (SilScheduler*)malloc(sizeof(SilScheduler));
    s->n_hilos = n_hilos_os ? n_hilos_os : 1;
    return s;
}
void sil_sched_destruir(SilScheduler *s) { free(s); }
bool sil_sched_spawn(SilScheduler *s, void (*entrada)(void*), void *arg) {
    (void)s; (void)entrada; (void)arg; return true; /* TODO(M7) */
}
void sil_sched_ejecutar(SilScheduler *s) { (void)s; /* TODO(M7) */ }
void sil_fibra_yield(void) { /* TODO(M7): 4 ciclos, sin syscall */ }
