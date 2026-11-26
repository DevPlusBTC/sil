/* test_aio.c - Contrato de la E/S asincrona POSIX (sil-rt/src/aio_linux.c).
 *
 * Sin io_uring (ruta por defecto): la operacion se ejecuta con pread/pwrite y
 * la completacion se entrega en sil_aio_procesar() invocando el callback.
 * Aqui se fija ese contrato, incluida la semantica de la cola llena.
 */
#define _POSIX_C_SOURCE 200809L
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "sil_aio.h"
#include "sil_test.h"

#define COLA_MAX 256 /* SIL_AIO_COLA en aio_linux.c */

static int completadas;
static int ultimo_resultado;

static void al_completar(SilAioReq *req, void *user_data, int resultado) {
    (void)req;
    (void)user_data;
    completadas++;
    ultimo_resultado = resultado;
}

static int archivo_temporal(char *ruta, size_t n) {
    const char *dir = getenv("TMPDIR");
    int fd;
    if (!dir || !*dir) dir = "/tmp";
    snprintf(ruta, n, "%s/sil_aio_XXXXXX", dir);
    fd = mkstemp(ruta);
    return fd;
}

static void parametros_invalidos(void) {
    SilAioReq req;
    memset(&req, 0, sizeof req);
    {
        SilAioCtx *ctx = sil_aio_ctx_crear();
        ST_CHECK(ctx != NULL);
        ST_CHECK(!sil_aio_leer(NULL, &req));
        ST_CHECK(!sil_aio_leer(ctx, NULL));
        ST_CHECK(!sil_aio_escribir(NULL, &req));
        ST_CHECK(!sil_aio_cancelar(NULL, &req));
        ST_CHECK(!sil_aio_cancelar(ctx, NULL));
        ST_CHECK_EQ(sil_aio_procesar(ctx, 0, 0), 0);
        ST_CHECK_EQ(sil_aio_procesar(ctx, -1, 0), 0);
        ST_CHECK(!sil_aio_tiene_pendientes(ctx));
        ST_CHECK(!sil_aio_tiene_pendientes(NULL));
        sil_aio_ctx_destruir(ctx);
        sil_aio_ctx_destruir(NULL); /* no debe romper */
    }
}

static void escribir_y_leer_con_offset(void) {
    char ruta[64];
    char lectura[32];
    SilAioReq req;
    int fd = archivo_temporal(ruta, sizeof ruta);
    SilAioCtx *ctx;
    ST_CHECK(fd >= 0);
    if (fd < 0) return;
    ctx = sil_aio_ctx_crear();
    ST_CHECK(ctx != NULL);
    completadas = 0;

    memset(&req, 0, sizeof req);
    req.op = SIL_AIO_ESCRIBIR;
    req.fd = fd;
    req.buffer = (void *)"0123456789";
    req.longitud = 10;
    req.offset = 0;
    req.callback = al_completar;
    ST_CHECK(sil_aio_escribir(ctx, &req));
    ST_CHECK_EQ(req.resultado, 10);
    ST_CHECK(sil_aio_tiene_pendientes(ctx));

    /* El callback solo corre en procesar, no al enviar. */
    ST_CHECK_EQ(completadas, 0);
    ST_CHECK_EQ(sil_aio_procesar(ctx, 8, 0), 1);
    ST_CHECK_EQ(completadas, 1);
    ST_CHECK_EQ(ultimo_resultado, 10);
    ST_CHECK(!sil_aio_tiene_pendientes(ctx));

    memset(&req, 0, sizeof req);
    memset(lectura, 0, sizeof lectura);
    req.op = SIL_AIO_LEER;
    req.fd = fd;
    req.buffer = lectura;
    req.longitud = 4;
    req.offset = 3; /* "3456" */
    req.callback = al_completar;
    ST_CHECK(sil_aio_leer(ctx, &req));
    ST_CHECK_EQ(sil_aio_procesar(ctx, 8, 0), 1);
    ST_CHECK_EQ(completadas, 2);
    ST_CHECK_STR(lectura, "3456");

    /* Lectura mas alla del final: 0 bytes, sin error. */
    memset(&req, 0, sizeof req);
    req.op = SIL_AIO_LEER;
    req.fd = fd;
    req.buffer = lectura;
    req.longitud = 16;
    req.offset = 1000;
    req.callback = al_completar;
    ST_CHECK(sil_aio_leer(ctx, &req));
    ST_CHECK_EQ(sil_aio_procesar(ctx, 8, 0), 1);
    ST_CHECK_EQ(ultimo_resultado, 0);

    sil_aio_ctx_destruir(ctx);
    close(fd);
    unlink(ruta);
}

static void errores_se_reportan_como_negativos(void) {
    char lectura[8];
    SilAioReq req;
    SilAioCtx *ctx = sil_aio_ctx_crear();
    ST_CHECK(ctx != NULL);
    completadas = 0;

    /* Descriptor invalido: resultado negativo, callback igualmente invocado. */
    memset(&req, 0, sizeof req);
    req.op = SIL_AIO_LEER;
    req.fd = -1;
    req.buffer = lectura;
    req.longitud = sizeof lectura;
    req.callback = al_completar;
    ST_CHECK(sil_aio_leer(ctx, &req));
    ST_CHECK(req.resultado < 0);
    ST_CHECK_EQ(sil_aio_procesar(ctx, 4, 0), 1);
    ST_CHECK_EQ(completadas, 1);
    ST_CHECK(ultimo_resultado < 0);

    /* Sin callback: la operacion se procesa sin romper. */
    memset(&req, 0, sizeof req);
    req.op = SIL_AIO_LEER;
    req.fd = -1;
    req.buffer = lectura;
    req.longitud = sizeof lectura;
    req.callback = NULL;
    ST_CHECK(sil_aio_leer(ctx, &req));
    ST_CHECK_EQ(sil_aio_procesar(ctx, 4, 0), 1);
    ST_CHECK_EQ(completadas, 1);

    sil_aio_ctx_destruir(ctx);
}

static void orden_fifo_de_completaciones(void) {
    char ruta[64];
    SilAioReq reqs[8];
    int fd = archivo_temporal(ruta, sizeof ruta);
    SilAioCtx *ctx = sil_aio_ctx_crear();
    int i;
    ST_CHECK(fd >= 0);
    ST_CHECK(ctx != NULL);
    if (fd < 0 || !ctx) return;
    completadas = 0;

    for (i = 0; i < 8; i++) {
        memset(&reqs[i], 0, sizeof reqs[i]);
        reqs[i].op = SIL_AIO_ESCRIBIR;
        reqs[i].fd = fd;
        reqs[i].buffer = (void *)"abcdefgh";
        reqs[i].longitud = 8;
        reqs[i].offset = (uint64_t)(i * 8);
        reqs[i].callback = al_completar;
        reqs[i].user_data = (void *)(uintptr_t)i;
        ST_CHECK(sil_aio_escribir(ctx, &reqs[i]));
    }
    ST_CHECK_EQ(sil_aio_procesar(ctx, 3, 0), 3); /* max_eventos se respeta */
    ST_CHECK_EQ(completadas, 3);
    ST_CHECK_EQ(sil_aio_procesar(ctx, 100, 0), 5);
    ST_CHECK_EQ(completadas, 8);
    ST_CHECK_EQ(sil_aio_procesar(ctx, 100, 0), 0); /* ya no hay nada */

    sil_aio_ctx_destruir(ctx);
    close(fd);
    unlink(ruta);
}

static void cola_llena_no_miente(void) {
    char ruta[64];
    static SilAioReq reqs[COLA_MAX + 8];
    int fd = archivo_temporal(ruta, sizeof ruta);
    SilAioCtx *ctx = sil_aio_ctx_crear();
    int i, aceptadas = 0, rechazadas = 0;
    ST_CHECK(fd >= 0);
    ST_CHECK(ctx != NULL);
    if (fd < 0 || !ctx) return;
    completadas = 0;

    for (i = 0; i < COLA_MAX + 8; i++) {
        memset(&reqs[i], 0, sizeof reqs[i]);
        reqs[i].op = SIL_AIO_ESCRIBIR;
        reqs[i].fd = fd;
        reqs[i].buffer = (void *)"x";
        reqs[i].longitud = 1;
        reqs[i].offset = (uint64_t)i;
        reqs[i].callback = al_completar;
        if (sil_aio_escribir(ctx, &reqs[i])) {
            aceptadas++;
        } else {
            rechazadas++;
        }
    }
    /* La cola es acotada: al desbordar se rechaza (false), no se pierde en
     * silencio la completacion de una operacion aceptada. */
    ST_CHECK_EQ(aceptadas, COLA_MAX);
    ST_CHECK(rechazadas > 0);
    ST_CHECK(sil_aio_tiene_pendientes(ctx));

    /* Drenar todo devuelve exactamente las aceptadas. */
    for (i = 0; i < 4; i++) (void)sil_aio_procesar(ctx, 1000, 0);
    ST_CHECK_EQ(completadas, COLA_MAX);
    ST_CHECK(!sil_aio_tiene_pendientes(ctx));

    sil_aio_ctx_destruir(ctx);
    close(fd);
    unlink(ruta);
}

static void cancelar_no_aplica_y_destruir_con_pendientes(void) {
    char ruta[64];
    SilAioReq req;
    int fd = archivo_temporal(ruta, sizeof ruta);
    SilAioCtx *ctx = sil_aio_ctx_crear();
    ST_CHECK(fd >= 0);
    ST_CHECK(ctx != NULL);
    if (fd < 0 || !ctx) return;

    memset(&req, 0, sizeof req);
    req.op = SIL_AIO_ESCRIBIR;
    req.fd = fd;
    req.buffer = (void *)"z";
    req.longitud = 1;
    ST_CHECK(sil_aio_escribir(ctx, &req));
    /* En la ruta pread/pwrite la operacion ya se ejecuto: no hay nada que
     * cancelar y se declara explicitamente con false. */
    ST_CHECK(!sil_aio_cancelar(ctx, &req));

    /* Destruir con pendientes no debe invocar callbacks ni romper. */
    completadas = 0;
    sil_aio_ctx_destruir(ctx);
    ST_CHECK_EQ(completadas, 0);

    close(fd);
    unlink(ruta);
}

int main(void) {
    ST_CASO(parametros_invalidos);
    ST_CASO(escribir_y_leer_con_offset);
    ST_CASO(errores_se_reportan_como_negativos);
    ST_CASO(orden_fifo_de_completaciones);
    ST_CASO(cola_llena_no_miente);
    ST_CASO(cancelar_no_aplica_y_destruir_con_pendientes);
    ST_FIN();
}
