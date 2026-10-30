/* sil_test.h - Microframework de pruebas para el runtime C de SIL.
 *
 * Sin dependencias externas: cada test es una funcion `static void` y el
 * runner devuelve 1 si alguna comprobacion fallo. Los tests se compilan en
 * modo estricto (-std=c99 -Wall -Wextra -Werror -pedantic) y se ejecutan bajo
 * AddressSanitizer/UBSan (y ThreadSanitizer en los tests concurrentes).
 */
#ifndef SIL_TEST_H
#define SIL_TEST_H

#include <stddef.h>
#include <stdio.h>
#include <string.h>

static int sil_test_fallos = 0;
static int sil_test_checks = 0;
static const char *sil_test_actual = "";

#define ST_CASO(fn)                       \
    do {                                  \
        sil_test_actual = #fn;            \
        fn();                             \
    } while (0)

#define ST_FALLA(msg)                                                            \
    do {                                                                         \
        sil_test_fallos++;                                                       \
        fprintf(stderr, "FALLO %s (%s:%d): %s\n", sil_test_actual, __FILE__,     \
                __LINE__, (msg));                                                \
    } while (0)

#define ST_CHECK(cond)                                                           \
    do {                                                                         \
        sil_test_checks++;                                                       \
        if (!(cond)) ST_FALLA(#cond);                                            \
    } while (0)

#define ST_CHECK_EQ(a, b)                                                        \
    do {                                                                         \
        sil_test_checks++;                                                       \
        unsigned long long st_a = (unsigned long long)(a);                       \
        unsigned long long st_b = (unsigned long long)(b);                       \
        if (st_a != st_b) {                                                      \
            char st_buf[160];                                                    \
            snprintf(st_buf, sizeof st_buf, "%s == %s (%llu != %llu)", #a, #b,   \
                     st_a, st_b);                                                \
            ST_FALLA(st_buf);                                                    \
        }                                                                        \
    } while (0)

#define ST_CHECK_STR(a, b)                                                       \
    do {                                                                         \
        sil_test_checks++;                                                       \
        if (strcmp((a), (b)) != 0) {                                             \
            char st_buf[256];                                                    \
            snprintf(st_buf, sizeof st_buf, "cadena distinta: %.60s != %.60s",   \
                     (a), (b));                                                  \
            ST_FALLA(st_buf);                                                    \
        }                                                                        \
    } while (0)

/* Declara el final del runner: imprime el resumen y devuelve el codigo. */
#define ST_FIN()                                                                 \
    do {                                                                         \
        printf("%s: %d comprobaciones, %d fallos\n", __FILE__, sil_test_checks,  \
               sil_test_fallos);                                                 \
        return sil_test_fallos == 0 ? 0 : 1;                                     \
    } while (0)

#endif /* SIL_TEST_H */
