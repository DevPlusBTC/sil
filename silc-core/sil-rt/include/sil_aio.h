#ifndef SIL_AIO_H
#define SIL_AIO_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

/* Async I/O operations for SIL runtime.
 * Windows: Overlapped I/O (IOCP)
 * Linux: io_uring si <liburing.h> está disponible; si no, pread/pwrite
 * macOS/BSD: pread/pwrite (solo archivos)
 */

typedef enum {
    SIL_AIO_LEER = 0,
    SIL_AIO_ESCRIBIR = 1,
} SilAioOp;

typedef struct SilAioReq SilAioReq;
typedef struct SilAioCtx SilAioCtx;

/* Callback cuando la operación completa */
typedef void (*SilAioCallback)(SilAioReq *req, void *user_data, int resultado);

/* Solicitud de I/O asíncrono */
struct SilAioReq {
    SilAioOp op;
    int fd;                    // Handle de archivo/socket
    void *buffer;              // Buffer de datos
    size_t longitud;           // Longitud del buffer
    uint64_t offset;           // Offset en archivo (para archivos)
    SilAioCallback callback;   // Callback de completación
    void *user_data;           // Datos de usuario para callback
    int resultado;             // Resultado (bytes transferidos o error)
    void *interno;             // Datos internos específicos de plataforma
};

/* Contexto de I/O asíncrono */
struct SilAioCtx {
    void *interno;             // IOCP handle (Windows) o io_uring (Linux)
};

/* Crear contexto de I/O asíncrono */
SilAioCtx* sil_aio_ctx_crear(void);

/* Destruir contexto */
void sil_aio_ctx_destruir(SilAioCtx *ctx);

/* Enviar operación de lectura asíncrona */
bool sil_aio_leer(SilAioCtx *ctx, SilAioReq *req);

/* Enviar operación de escritura asíncrona */
bool sil_aio_escribir(SilAioCtx *ctx, SilAioReq *req);

/* Procesar completaciones (llamar periódicamente o en loop) */
int sil_aio_procesar(SilAioCtx *ctx, int max_eventos, int timeout_ms);

/* Cancelar operación pendiente */
bool sil_aio_cancelar(SilAioCtx *ctx, SilAioReq *req);

/* Verificar si hay operaciones pendientes */
bool sil_aio_tiene_pendientes(SilAioCtx *ctx);

#endif // SIL_AIO_H