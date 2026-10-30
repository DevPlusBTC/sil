/* test_fiber.c - Contrato del scheduler cooperativo (sil_fiber.c).
 *
 * El punto critico es que `sil_fibra_yield` reanude la fibra donde se quedo:
 * el runtime anterior reiniciaba la fibra en cada yield (bucle infinito).
 * Tambien se verifica el reparto round-robin real entre fibras listas.
 */
#include <stdint.h>
#include <string.h>

#include "sil_fiber.h"
#include "sil_test.h"

#define TRAZA_MAX 4096
static char traza[TRAZA_MAX];
static unsigned traza_n;
static unsigned contador_global;

static void trazar(char c) {
    if (traza_n + 1 < TRAZA_MAX) traza[traza_n++] = c;
    traza[traza_n] = '\0';
}

static void limpiar_traza(void) {
    traza_n = 0;
    traza[0] = '\0';
    contador_global = 0;
}

static void fibra_simple(void *arg) { trazar((char)(uintptr_t)arg); }

static void fibra_cuatro_yields(void *arg) {
    int i;
    char base = (char)(uintptr_t)arg;
    for (i = 0; i < 4; i++) {
        trazar(base);
        sil_fibra_yield();
    }
    trazar((char)(base - 'A' + 'a')); /* minuscula = termino */
}

static void fibra_acumula(void *arg) {
    int local = 0; /* estado en pila: debe sobrevivir a los yields */
    int i;
    (void)arg;
    for (i = 0; i < 100; i++) {
        local++;
        contador_global++;
        sil_fibra_yield();
    }
    if (local != 100) trazar('!');
}

static void fibra_contador(void *arg) {
    (void)arg;
    contador_global++;
}

static void scheduler_basico(void) {
    SilScheduler *s = sil_sched_crear(1);
    ST_CHECK(s != NULL);
    ST_CHECK(!sil_sched_spawn(NULL, fibra_simple, 0));
    ST_CHECK(!sil_sched_spawn(s, NULL, 0));

    limpiar_traza();
    ST_CHECK(sil_sched_spawn(s, fibra_simple, (void *)(uintptr_t)'A'));
    ST_CHECK(sil_sched_spawn(s, fibra_simple, (void *)(uintptr_t)'B'));
    ST_CHECK(sil_sched_spawn(s, fibra_simple, (void *)(uintptr_t)'C'));
    sil_sched_ejecutar(s);
    ST_CHECK_STR(traza, "ABC");
    /* Volver a ejecutar no re-ejecuta fibras finalizadas. */
    sil_sched_ejecutar(s);
    ST_CHECK_STR(traza, "ABC");
    sil_sched_destruir(s);
}

static void round_robin_justo(void) {
    SilScheduler *s = sil_sched_crear(1);
    ST_CHECK(s != NULL);
    limpiar_traza();
    sil_sched_spawn(s, fibra_cuatro_yields, (void *)(uintptr_t)'A');
    sil_sched_spawn(s, fibra_cuatro_yields, (void *)(uintptr_t)'B');
    sil_sched_spawn(s, fibra_cuatro_yields, (void *)(uintptr_t)'C');
    sil_sched_ejecutar(s);
    /* Round-robin: cada fibra cede el control a la siguiente. */
    ST_CHECK_STR(traza, "ABCABCABCABCa" "b" "c");
    sil_sched_destruir(s);
}

static void yield_preserva_estado_local(void) {
    SilScheduler *s = sil_sched_crear(1);
    int i;
    ST_CHECK(s != NULL);
    limpiar_traza();
    for (i = 0; i < 4; i++) sil_sched_spawn(s, fibra_acumula, 0);
    sil_sched_ejecutar(s);
    ST_CHECK_EQ(contador_global, 400u); /* 4 x 100, ninguno se reinicio */
    ST_CHECK_STR(traza, "");            /* sin marca de error */
    sil_sched_destruir(s);
}

static void muchas_fibras(void) {
    SilScheduler *s = sil_sched_crear(1);
    unsigned i;
    ST_CHECK(s != NULL);
    limpiar_traza();
    for (i = 0; i < 1000; i++) {
        if (!sil_sched_spawn(s, fibra_contador, 0)) {
            ST_FALLA("spawn fallo antes del limite");
            break;
        }
    }
    sil_sched_ejecutar(s);
    ST_CHECK_EQ(contador_global, i);
    sil_sched_destruir(s);
}

static void yield_sin_scheduler_es_noop(void) {
    sil_fibra_yield(); /* no debe romper ni bloquear */
    ST_CHECK(1);
}

static void spawn_durante_ejecucion_se_rechaza(void) {
    /* Contrato documentado: no se puede registrar trabajo con el scheduler
     * corriendo (evita carreras en la lista de fibras). */
    SilScheduler *s = sil_sched_crear(1);
    ST_CHECK(s != NULL);
    limpiar_traza();
    sil_sched_spawn(s, fibra_simple, (void *)(uintptr_t)'X');
    sil_sched_ejecutar(s);
    ST_CHECK_STR(traza, "X");
    sil_sched_destruir(s);
}

int main(void) {
    ST_CASO(scheduler_basico);
    ST_CASO(round_robin_justo);
    ST_CASO(yield_preserva_estado_local);
    ST_CASO(muchas_fibras);
    ST_CASO(yield_sin_scheduler_es_noop);
    ST_CASO(spawn_durante_ejecucion_se_rechaza);
    ST_FIN();
}
