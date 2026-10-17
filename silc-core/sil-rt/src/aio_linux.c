/* sil_aio_linux.c - Async I/O para Linux usando io_uring
 * 
 * Implementación completa de I/O asíncrono usando io_uring (Linux 5.1+).
 * Requiere liburing.
 */

#include "sil_aio.h"
#include <stdlib.h>
#include <string.h>
#include <errno.h>

#if defined(__linux__)
#include <liburing.h>

typedef struct {
    struct io_uring ring;
    int inicializado;
} SilAioCtxInterno;

SilAioCtx* sil_aio_ctx_crear(void) {
    SilAioCtx *ctx = (SilAioCtx*)malloc(sizeof(SilAioCtx));
    if (!ctx) return NULL;
    
    SilAioCtxInterno *interno = (SilAioCtxInterno*)malloc(sizeof(SilAioCtxInterno));
    if (!interno) {
        free(ctx);
        return NULL;
    }
    
    /* Inicializar io_uring con 256 entradas */
    int ret = io_uring_queue_init(256, &interno->ring, 0);
    if (ret < 0) {
        free(interno);
        free(ctx);
        return NULL;
    }
    
    interno->inicializado = 1;
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
    
    /* Preparar operación de lectura */
    if (req->offset > 0) {
        io_uring_prep_readv(sqe, req->fd, &(struct iovec){req->buffer, req->longitud}, 1, req->offset);
    } else {
        io_uring_prep_read(sqe, req->fd, req->buffer, req->longitud, req->offset);
    }
    
    /* Guardar puntero a la request en user_data */
    io_uring_sqe_set_data(sqe, req);
    req->interno = sqe;
    
    return true;
}

bool sil_aio_escribir(SilAioCtx *ctx, SilAioReq *req) {
    if (!ctx || !req || !ctx->interno) return false;
    
    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    if (!interno->inicializado) return false;
    
    struct io_uring_sqe *sqe = io_uring_get_sqe(&interno->ring);
    if (!sqe) return false;
    
    /* Preparar operación de escritura */
    if (req->offset > 0) {
        io_uring_prep_writev(sqe, req->fd, &(struct iovec){req->buffer, req->longitud}, 1, req->offset);
    } else {
        io_uring_prep_write(sqe, req->fd, req->buffer, req->longitud, req->offset);
    }
    
    /* Guardar puntero a la request en user_data */
    io_uring_sqe_set_data(sqe, req);
    req->interno = sqe;
    
    return true;
}

int sil_aio_procesar(SilAioCtx *ctx, int max_eventos, int timeout_ms) {
    if (!ctx || !ctx->interno) return 0;
    
    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    if (!interno->inicializado) return 0;
    
    /* Enviar SQEs pendientes */
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
            if (ret == -EAGAIN) break;  // Sin eventos
        }
        
        if (ret < 0) {
            if (ret == -ETIME) break;  // Timeout
            continue;
        }
        
        SilAioReq *req = (SilAioReq*)io_uring_cqe_get_data(cqe);
        if (req) {
            req->resultado = cqe->res;
            if (req->callback) {
                req->callback(req, req->user_data, req->resultado);
            }
        }
        
        io_uring_cqe_seen(&interno->ring, cqe);
        procesados++;
        
        if (timeout_ms > 0) break;
    }
    
    return procesados;
}

bool sil_aio_cancelar(SilAioCtx *ctx, SilAioReq *req) {
    if (!ctx || !req || !req->interno) return false;
    
    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    if (!interno->inicializado) return false;
    
    struct io_uring_sqe *sqe = io_uring_get_sqe(&interno->ring);
    if (!sqe) return false;
    
    /* Preparar cancelación */
    io_uring_prep_cancel(sqe, req->interno, 0);
    io_uring_sqe_set_data(sqe, req);
    
    int ret = io_uring_submit(&interno->ring);
    return ret >= 0;
}

bool sil_aio_tiene_pendientes(SilAioCtx *ctx) {
    if (!ctx || !ctx->interno) return false;
    
    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    if (!interno->inicializado) return false;
    
    /* Verificar si hay CQEs disponibles sin bloquear */
    struct io_uring_cqe *cqe;
    int ret = io_uring_peek_cqe(&interno->ring, &cqe);
    return ret == 0;
}

#else
/* Stub para plataformas no-Linux */
SilAioCtx* sil_aio_ctx_crear(void) { return NULL; }
void sil_aio_ctx_destruir(SilAioCtx *ctx) { (void)ctx; }
bool sil_aio_leer(SilAioCtx *ctx, SilAioReq *req) { (void)ctx; (void)req; return false; }
bool sil_aio_escribir(SilAioCtx *ctx, SilAioReq *req) { (void)ctx; (void)req; return false; }
int sil_aio_procesar(SilAioCtx *ctx, int max_eventos, int timeout_ms) { (void)ctx; (void)max_eventos; (void)timeout_ms; return 0; }
bool sil_aio_cancelar(SilAioCtx *ctx, SilAioReq *req) { (void)ctx; (void)req; return false; }
bool sil_aio_tiene_pendientes(SilAioCtx *ctx) { (void)ctx; return false; }

#endif // __linux__