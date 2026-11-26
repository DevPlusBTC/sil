/* test_arena.c - Contrato de las arenas causales (sil_arena.c).
 *
 * Cada comprobacion corresponde a una garantia documentada en
 * sil-rt/include/sil_arena.h. Se ejecuta bajo ASan/UBSan: si una asignacion
 * devolviera memoria fuera del bloque, el `memset` de verificacion lo delata.
 */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "sil_arena.h"
#include "sil_test.h"

static void arena_vacia_y_destruir(void) {
    SilArena a = sil_arena_crear(0);
    ST_CHECK(a.inicio != NULL); /* 0 usa la pagina por defecto */
    ST_CHECK(a.asignado_total >= SIL_ARENA_PAGE_SIZE);
    ST_CHECK_EQ(sil_arena_ciclo_actual(&a), 0);
    sil_arena_destruir(&a);
    ST_CHECK(a.inicio == NULL);
    ST_CHECK(a.actual == NULL);
    /* Tras destruir, la arena sigue siendo utilizable: reasigna perezosamente. */
    void *p = sil_arena_asignar(&a, 32, 8);
    ST_CHECK(p != NULL);
    memset(p, 0xAB, 32);
    sil_arena_destruir(&a);
}

static void alineacion_se_respeta(void) {
    static const size_t alineaciones[] = {1, 2, 4, 8, 16, 32, 64};
    static const size_t tamanios[] = {1, 3, 7, 8, 15, 16, 33, 64, 100};
    size_t i, j;
    SilArena a = sil_arena_crear(64); /* pagina minima: fuerza bloques nuevos */
    for (i = 0; i < sizeof(alineaciones) / sizeof(alineaciones[0]); i++) {
        for (j = 0; j < sizeof(tamanios) / sizeof(tamanios[0]); j++) {
            void *p = sil_arena_asignar(&a, tamanios[j], alineaciones[i]);
            ST_CHECK(p != NULL);
            if (!p) continue;
            ST_CHECK_EQ((uintptr_t)p % alineaciones[i], 0);
            memset(p, 0x5A, tamanios[j]); /* ASan detecta salida del bloque */
        }
    }
    sil_arena_destruir(&a);
}

static void alineacion_invalida_devuelve_null(void) {
    SilArena a = sil_arena_crear(256);
    ST_CHECK(sil_arena_asignar(&a, 8, 3) == NULL);   /* no potencia de dos */
    ST_CHECK(sil_arena_asignar(&a, 8, 24) == NULL);  /* no potencia de dos */
    ST_CHECK(sil_arena_asignar(&a, 8, 0) != NULL);   /* 0 = alineacion natural */
    sil_arena_destruir(&a);
}

static void tamanio_cero_da_punteros_distintos(void) {
    SilArena a = sil_arena_crear(1024);
    void *p = sil_arena_asignar(&a, 0, 8);
    void *q = sil_arena_asignar(&a, 0, 8);
    ST_CHECK(p != NULL);
    ST_CHECK(q != NULL);
    ST_CHECK(p != q);
    sil_arena_destruir(&a);
}

static void desbordamiento_no_rompe(void) {
    SilArena a = sil_arena_crear(1024);
    size_t antes = a.asignado_total;
    ST_CHECK(sil_arena_asignar(&a, SIZE_MAX, 8) == NULL);
    ST_CHECK(sil_arena_asignar(&a, SIZE_MAX - 4, 16) == NULL);
    ST_CHECK_EQ(a.asignado_total, antes); /* la arena no se toco */
    void *p = sil_arena_asignar(&a, 16, 8); /* sigue siendo usable */
    ST_CHECK(p != NULL);
    sil_arena_destruir(&a);
}

static void peticion_grande_usa_bloque_propio(void) {
    SilArena a = sil_arena_crear(1024);
    size_t pagina = (size_t)SIL_ARENA_PAGE_SIZE;
    void *p = sil_arena_asignar(&a, pagina * 3 + 7, 64);
    ST_CHECK(p != NULL);
    ST_CHECK_EQ((uintptr_t)p % 64, 0);
    memset(p, 0x11, pagina * 3 + 7);
    ST_CHECK(a.asignado_total >= pagina * 3);
    sil_arena_destruir(&a);
}

static void reset_conserva_bloques(void) {
    SilArena a = sil_arena_crear(256);
    void *p1 = sil_arena_asignar(&a, 64, 8);
    void *p2;
    sil_arena_reset(&a);
    p2 = sil_arena_asignar(&a, 64, 8);
    ST_CHECK_EQ(p1, p2); /* mismo offset tras el reset */
    ST_CHECK_EQ(sil_arena_ciclo_actual(&a), 0);
    sil_arena_destruir(&a);
}

static void ciclo_reclama_y_reutiliza(void) {
    SilArena a = sil_arena_crear(1024);
    void *p1 = sil_arena_asignar(&a, 32, 8);
    uint32_t c, p2, p3;
    void *m1, *m2;

    c = sil_arena_ciclo_iniciar(&a);
    ST_CHECK(c != 0);
    ST_CHECK_EQ(sil_arena_ciclo_actual(&a), c);
    m1 = sil_arena_asignar(&a, 64, 8);
    ST_CHECK_EQ(sil_arena_ciclo_bytes(&a, c), 64);
    sil_arena_ciclo_finalizar(&a);
    ST_CHECK_EQ(sil_arena_ciclo_actual(&a), 0);
    ST_CHECK_EQ(sil_arena_ciclo_bytes(&a, c), 0); /* reclamado */
    m2 = sil_arena_asignar(&a, 64, 8);
    ST_CHECK_EQ(m1, m2); /* el offset volvio a la marca */
    (void)p1;
    p2 = sil_arena_ciclo_iniciar(&a);
    p3 = sil_arena_ciclo_iniciar(&a);
    ST_CHECK(p3 != p2); /* ids monotonos */
    sil_arena_ciclo_finalizar(&a);
    sil_arena_ciclo_finalizar(&a);
    sil_arena_ciclo_finalizar(&a); /* de mas: no-op */
    sil_arena_destruir(&a);
}

static void ciclos_anidados_contabilidad(void) {
    SilArena a = sil_arena_crear(4096);
    uint32_t c1 = sil_arena_ciclo_iniciar(&a);
    uint32_t c2;
    uint32_t abiertos = 0;
    size_t bytes_actual = 0;
    sil_arena_asignar(&a, 16, 8);
    ST_CHECK_EQ(sil_arena_ciclo_bytes(&a, c1), 16);
    c2 = sil_arena_ciclo_iniciar(&a);
    ST_CHECK(c2 != c1);
    sil_arena_asignar(&a, 48, 8);
    sil_arena_ciclo_estadisticas(&a, &abiertos, &bytes_actual);
    ST_CHECK_EQ(abiertos, 2);
    ST_CHECK_EQ(bytes_actual, 48); /* el interno solo cuenta lo suyo */
    sil_arena_ciclo_finalizar(&a);
    ST_CHECK_EQ(sil_arena_ciclo_actual(&a), c1);
    ST_CHECK_EQ(sil_arena_ciclo_bytes(&a, c1), 16);
    ST_CHECK_EQ(sil_arena_ciclo_bytes(&a, c2), 0);
    sil_arena_ciclo_finalizar(&a);
    ST_CHECK_EQ(sil_arena_ciclo_actual(&a), 0);
    sil_arena_destruir(&a);
}

static void ciclo_libera_bloques_creados_dentro(void) {
    SilArena a = sil_arena_crear(128);
    size_t total_antes;
    uint32_t c = sil_arena_ciclo_iniciar(&a);
    void *p;
    ST_CHECK(c != 0);
    total_antes = a.asignado_total;
    p = sil_arena_asignar(&a, (size_t)SIL_ARENA_PAGE_SIZE * 2, 8); /* bloque nuevo */
    ST_CHECK(p != NULL);
    ST_CHECK(a.asignado_total > total_antes);
    sil_arena_ciclo_finalizar(&a);
    ST_CHECK_EQ(a.asignado_total, total_antes); /* el bloque extra se libero */
    ST_CHECK_EQ(a.actual, a.inicio);
    memset(sil_arena_asignar(&a, 16, 8), 0, 16); /* arena usable */
    sil_arena_destruir(&a);
}

static void ciclo_en_arena_vacia(void) {
    SilArena a = sil_arena_crear(64);
    uint32_t c;
    sil_arena_destruir(&a);
    c = sil_arena_ciclo_iniciar(&a);
    ST_CHECK(c != 0);
    ST_CHECK(sil_arena_asignar(&a, 32, 8) != NULL);
    sil_arena_ciclo_finalizar(&a);
    ST_CHECK(a.inicio == NULL);
    ST_CHECK_EQ(a.asignado_total, 0);
    ST_CHECK(sil_arena_asignar(&a, 32, 8) != NULL);
    sil_arena_destruir(&a);
}

static void limite_de_ciclos(void) {
    SilArena a = sil_arena_crear(256);
    uint32_t i, id = 0;
    for (i = 0; i < SIL_ARENA_MAX_CICLOS; i++) {
        id = sil_arena_ciclo_iniciar(&a);
        ST_CHECK(id != 0);
    }
    ST_CHECK_EQ(sil_arena_ciclo_iniciar(&a), 0); /* desbordamiento controlado */
    ST_CHECK_EQ(a.ciclos_abiertos, SIL_ARENA_MAX_CICLOS);
    for (i = 0; i < SIL_ARENA_MAX_CICLOS; i++) sil_arena_ciclo_finalizar(&a);
    ST_CHECK_EQ(a.ciclos_abiertos, 0);
    sil_arena_destruir(&a);
}

static void promover_transfiere_bloques(void) {
    SilArena origen = sil_arena_crear(256);
    SilArena destino = sil_arena_crear(256);
    void *po = sil_arena_asignar(&origen, 4096, 8); /* bloque propio */
    void *pd = sil_arena_asignar(&destino, 16, 8);
    size_t total_esperado;
    ST_CHECK(po != NULL);
    ST_CHECK(pd != NULL);
    total_esperado = destino.asignado_total + origen.asignado_total;
    sil_arena_promover(&origen, &destino, po, 4096);
    ST_CHECK(origen.inicio == NULL);
    ST_CHECK(origen.actual == NULL);
    ST_CHECK_EQ(origen.asignado_total, 0);
    ST_CHECK_EQ(destino.asignado_total, total_esperado);
    memset(po, 0x33, 4096); /* memoria viva en el destino, sin copia */
    sil_arena_destruir(&destino);
    sil_arena_destruir(&origen);
}

static void promover_es_idempotente_y_seguro(void) {
    SilArena a = sil_arena_crear(256);
    SilArena vacia = sil_arena_crear(256);
    void *p = sil_arena_asignar(&a, 64, 8);
    sil_arena_promover(&a, &a, p, 64);      /* no debe auto-enlazar */
    sil_arena_promover(&vacia, &a, NULL, 0); /* origen vacio: no-op */
    ST_CHECK(a.inicio != NULL);
    memset(sil_arena_asignar(&a, 16, 8), 0, 16);
    sil_arena_destruir(&a); /* si hubiera ciclo en la lista, colgaria aqui */
    sil_arena_destruir(&vacia);
}

static void estres_sin_solapes(void) {
    SilArena a = sil_arena_crear(512);
    enum { N = 2000 };
    unsigned char *ptrs[N];
    size_t tam[N];
    size_t i, j;
    unsigned semilla = 12345u;

    for (i = 0; i < N; i++) {
        size_t alineaciones[3] = {8, 16, 64};
        semilla = semilla * 1103515245u + 12345u;
        tam[i] = (semilla >> 16) % 200 + 1;
        ptrs[i] = (unsigned char *)sil_arena_asignar(&a, tam[i], alineaciones[i % 3]);
        ST_CHECK(ptrs[i] != NULL);
        if (!ptrs[i]) break;
        ST_CHECK_EQ((uintptr_t)ptrs[i] % alineaciones[i % 3], 0);
        memset(ptrs[i], (int)(i & 0xFF), tam[i]);
    }
    for (i = 0; i < N; i++) {
        if (!ptrs[i]) continue;
        for (j = 0; j < tam[i]; j++) {
            if (ptrs[i][j] != (unsigned char)(i & 0xFF)) {
                ST_FALLA("memoria de otra asignacion solapada");
                i = N;
                break;
            }
        }
    }
    sil_arena_destruir(&a);
}

int main(void) {
    ST_CASO(arena_vacia_y_destruir);
    ST_CASO(alineacion_se_respeta);
    ST_CASO(alineacion_invalida_devuelve_null);
    ST_CASO(tamanio_cero_da_punteros_distintos);
    ST_CASO(desbordamiento_no_rompe);
    ST_CASO(peticion_grande_usa_bloque_propio);
    ST_CASO(reset_conserva_bloques);
    ST_CASO(ciclo_reclama_y_reutiliza);
    ST_CASO(ciclos_anidados_contabilidad);
    ST_CASO(ciclo_libera_bloques_creados_dentro);
    ST_CASO(ciclo_en_arena_vacia);
    ST_CASO(limite_de_ciclos);
    ST_CASO(promover_transfiere_bloques);
    ST_CASO(promover_es_idempotente_y_seguro);
    ST_CASO(estres_sin_solapes);
    ST_FIN();
}
