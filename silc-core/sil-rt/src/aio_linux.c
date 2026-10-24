/* sil_aio_posix.c - Async I/O para POSIX.
 *
 * En Linux usa io_uring si <liburing.h> está disponible (Linux 5.1+).
 * En el resto de plataformas POSIX (macOS, BSD) o Linux sin liburing,
 * la operación se ejecuta con pread/pwrite y la completación se entrega
 * en sil_aio_procesar(). No hay stub: el I/O siempre se realiza.
 */

#include "sil_aio.h"
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <unistd.h>
#include <signal.h>
#include <fcntl.h>

/* io_uring es opt-in: se activa compilando con -DSIL_IO_URING (o
 * SIL_IO_URING=1 en el entorno de cargo). Por defecto se usa pread/pwrite,
 * así el build no depende de liburing ni de los macros POSIX de cada distro.
 */
#if defined(__linux__) && defined(SIL_IO_URING) && defined(__has_include)
#if __has_include(<liburing.h>)
#include <liburing.h>
#define SIL_AIO_USAR_IO_URING 1
#endif
#endif
#ifndef SIL_AIO_USAR_IO_URING
#define SIL_AIO_USAR_IO_URING 0
#endif

#if SIL_AIO_USAR_IO_URING

typedef struct {
    struct io_uring ring;
    int inicializado;
    int pendientes;
} SilAioCtxInterno;

SilAioCtx* sil_aio_ctx_crear(void) {
    SilAioCtx *ctx = (SilAioCtx*)malloc(sizeof(SilAioCtx));
    if (!ctx) return NULL;

    SilAioCtxInterno *interno = (SilAioCtxInterno*)malloc(sizeof(SilAioCtxInterno));
    if (!interno) {
        free(ctx);
        return NULL;
    }

    int ret = io_uring_queue_init(256, &interno->ring, 0);
    if (ret < 0) {
        free(interno);
        free(ctx);
        return NULL;
    }

    interno->inicializado = 1;
    interno->pendientes = 0;
    ctx->interno = interno;
    return ctx;
}

void sil_aio_ctx_destruir(SilAioCtx *ctx) {
    if (!ctx || !ctx->interno) return;

    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    if (interno->inicializado) {
        io_uring_queue_exit(&interno->ring);
    }
    free(interno);
    free(ctx);
}

bool sil_aio_leer(SilAioCtx *ctx, SilAioReq *req) {
    if (!ctx || !req || !ctx->interno) return false;

    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    if (!interno->inicializado) return false;

    struct io_uring_sqe *sqe = io_uring_get_sqe(&interno->ring);
    if (!sqe) return false;

    if (req->offset > 0) {
        struct iovec iov = { req->buffer, req->longitud };
        io_uring_prep_readv(sqe, req->fd, &iov, 1, req->offset);
    } else {
        io_uring_prep_read(sqe, req->fd, req->buffer, req->longitud, req->offset);
    }

    io_uring_sqe_set_data(sqe, req);
    req->interno = NULL;
    interno->pendientes++;
    return true;
}

bool sil_aio_escribir(SilAioCtx *ctx, SilAioReq *req) {
    if (!ctx || !req || !ctx->interno) return false;

    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    if (!interno->inicializado) return false;

    struct io_uring_sqe *sqe = io_uring_get_sqe(&interno->ring);
    if (!sqe) return false;

    if (req->offset > 0) {
        struct iovec iov = { req->buffer, req->longitud };
        io_uring_prep_writev(sqe, req->fd, &iov, 1, req->offset);
    } else {
        io_uring_prep_write(sqe, req->fd, req->buffer, req->longitud, req->offset);
    }

    io_uring_sqe_set_data(sqe, req);
    req->interno = NULL;
    interno->pendientes++;
    return true;
}

int sil_aio_procesar(SilAioCtx *ctx, int max_eventos, int timeout_ms) {
    if (!ctx || !ctx->interno || max_eventos <= 0) return 0;

    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    if (!interno->inicializado) return 0;

    int ret = io_uring_submit(&interno->ring);
    if (ret < 0) return ret;

    int procesados = 0;
    struct io_uring_cqe *cqe;

    while (procesados < max_eventos) {
        if (timeout_ms >= 0) {
            struct __kernel_timespec ts = {
                .tv_sec = timeout_ms / 1000,
                .tv_nsec = (timeout_ms % 1000) * 1000000
            };
            ret = io_uring_wait_cqe_timeout(&interno->ring, &cqe, &ts);
        } else {
            ret = io_uring_peek_cqe(&interno->ring, &cqe);
            if (ret == -EAGAIN) break;
        }

        if (ret < 0) {
            if (ret == -ETIME) break;
            continue;
        }

        SilAioReq *req = (SilAioReq*)io_uring_cqe_get_data(cqe);
        if (req) {
            req->resultado = cqe->res;
            if (interno->pendientes > 0) interno->pendientes--;
            if (req->callback) {
                req->callback(req, req->user_data, req->resultado);
            }
        }

        io_uring_cqe_seen(&interno->ring, cqe);
        procesados++;
    }

    return procesados;
}

bool sil_aio_cancelar(SilAioCtx *ctx, SilAioReq *req) {
    if (!ctx || !ctx->interno || !req) return false;

    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    if (!interno->inicializado) return false;

    struct io_uring_sqe *sqe = io_uring_get_sqe(&interno->ring);
    if (!sqe) return false;

    /* El user_data de la petición original es `req`, no el SQE. */
    io_uring_prep_cancel(sqe, req, 0);
    io_uring_sqe_set_data(sqe, req);

    int ret = io_uring_submit(&interno->ring);
    return ret >= 0;
}

bool sil_aio_tiene_pendientes(SilAioCtx *ctx) {
    if (!ctx || !ctx->interno) return false;

    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    return interno->inicializado && interno->pendientes > 0;
}

#else /* !SIL_AIO_USAR_IO_URING */

#define SIL_AIO_COLA 256

typedef struct {
    SilAioReq *req;
    int resultado;
} SilAioEvento;

typedef struct {
    SilAioEvento cola[SIL_AIO_COLA];
    int cabeza;
    int cuenta;
} SilAioCtxInterno;

SilAioCtx* sil_aio_ctx_crear(void) {
    SilAioCtx *ctx = (SilAioCtx*)malloc(sizeof(SilAioCtx));
    if (!ctx) return NULL;

    SilAioCtxInterno *interno = (SilAioCtxInterno*)calloc(1, sizeof(SilAioCtxInterno));
    if (!interno) {
        free(ctx);
        return NULL;
    }

    ctx->interno = interno;
    return ctx;
}

void sil_aio_ctx_destruir(SilAioCtx *ctx) {
    if (!ctx) return;
    free(ctx->interno);
    free(ctx);
}

static bool encolar(SilAioCtxInterno *interno, SilAioReq *req, int resultado) {
    if (interno->cuenta >= SIL_AIO_COLA) return false;
    int pos = (interno->cabeza + interno->cuenta) % SIL_AIO_COLA;
    interno->cola[pos].req = req;
    interno->cola[pos].resultado = resultado;
    interno->cuenta++;
    return true;
}

bool sil_aio_leer(SilAioCtx *ctx, SilAioReq *req) {
    if (!ctx || !req || !ctx->interno) return false;

    ssize_t n = pread(req->fd, req->buffer, req->longitud, (off_t)req->offset);
    int resultado = (n < 0) ? -errno : (int)n;
    req->resultado = resultado;
    req->interno = NULL;
    return encolar((SilAioCtxInterno*)ctx->interno, req, resultado);
}

bool sil_aio_escribir(SilAioCtx *ctx, SilAioReq *req) {
    if (!ctx || !req || !ctx->interno) return false;

    ssize_t n = pwrite(req->fd, req->buffer, req->longitud, (off_t)req->offset);
    int resultado = (n < 0) ? -errno : (int)n;
    req->resultado = resultado;
    req->interno = NULL;
    return encolar((SilAioCtxInterno*)ctx->interno, req, resultado);
}

int sil_aio_procesar(SilAioCtx *ctx, int max_eventos, int timeout_ms) {
    /* El I/O ya se completó al enviarlo; timeout_ms no aplica. */
    (void)timeout_ms;
    if (!ctx || !ctx->interno || max_eventos <= 0) return 0;

    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    int procesados = 0;

    while (procesados < max_eventos && interno->cuenta > 0) {
        SilAioEvento *ev = &interno->cola[interno->cabeza];
        interno->cabeza = (interno->cabeza + 1) % SIL_AIO_COLA;
        interno->cuenta--;

        if (ev->req && ev->req->callback) {
            ev->req->callback(ev->req, ev->req->user_data, ev->resultado);
        }
        procesados++;
    }

    return procesados;
}

bool sil_aio_cancelar(SilAioCtx *ctx, SilAioReq *req) {
    /* La operación ya se ejecutó: no hay nada que cancelar. */
    (void)ctx;
    (void)req;
    return false;
}

bool sil_aio_tiene_pendientes(SilAioCtx *ctx) {
    if (!ctx || !ctx->interno) return false;
    return ((SilAioCtxInterno*)ctx->interno)->cuenta > 0;
}

#endif /* SIL_AIO_USAR_IO_URING */
