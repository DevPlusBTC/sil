#ifndef SIL_AIO_H
#define SIL_AIO_H

#include <stdint.h>
#include <stddef.h>
#include "sil_types.h"

/* =============================================================================
 * M33: E/S Asíncrona Nativa (AIO)
 * =============================================================================
 *
 * Proporciona una fachada unificada para operaciones de I/O asíncrona en
 * múltiples plataformas:
 *   - Linux: io_uring (preferido)
 *   - Fallback: poll/select (portable)
 *
 * Conceptos clave:
 *   - aio_context: contexto de contexto de operaciones asíncronas
 *   - aio_request: petición individual (fd, buf, size, flags)
 *   - aio_event: evento completado con resultado y bandera de éxito
 *   - Patrón: polling, callback, o completion retrieval
 *
 * NOTA: En MSVC/Windows el path usa Overlapped I/O; en Linux usa io_uring.
 * Este header declara la API; la implementación por plataforma está en aio_*.c
 */

/* Estados de operación AIO */
#define SIL_AIO_PENDING  0  /* En curso */
#define SIL_AIO_COMPLETED 1  /* Finalizada exitosamente */
#define SIL_AIO_FAILED   2  /* Error */
#define SIL_AIO_CANCELLED 3  /* Cancelada */

/* Bandera de operación */
#define SIL_AIO_DIRECT   0x01  /* I/O directo (skip cache) */
#define SIL_AIO_NONBLOCK 0x02  /* No bloqueante */

/* Máximo número de eventos por batch retrieval */
#define SIL_AIO_MAX_EVENTS 64

typedef struct {
    int fd;
    void *buf;
    size_t len;
    size_t offset;
    int flags;
    int status;      /* SIL_AIO_*, al completar */
    int64_t bytes_transferred; /* Bytes efectivamente transferidos */
} sil_aio_request;

/* Contexto de AIO - gestiona el anillo subyacente */
typedef struct sil_aio_context {
    int fd;                  /* fd de io_uring o -1 para fallback */
    unsigned int ring_size;  /* Tamaño del anillo (power of 2) */
    unsigned int head;       /* Índice de próxima submission */
    unsigned int tail;       /* Índice de próxima completación */
    /* Datos específicos por plataforma en la cola abajo */
} sil_aio_context;

/* Funciones de la API SIL (M33) */

/* Crear contexto AIO. Retorna NULL en error. */
sil_aio_context *sil_aio_crear(unsigned int ring_size);

/* Destruir contexto AIO y limpiar recursos. */
void sil_aio_destruir(sil_aio_context *ctx);

/* Subir una operación de I/O.
   Retorna índice de la petición o -1 en error.
   El caller debe mantener 'request' válido hasta completamiento. */
int sil_aio_submit(sil_aio_context *ctx, sil_aio_request *req);

/* Obtener eventos completados.
   nr: máximo de eventos a recuperar.
   timeout_ms: timeout en milisegundos (0 = non-block, -1 = bloqueante).
   Retorna número de eventos recuperados (0 = timeout). */
int sil_aio_getevents(sil_aio_context *ctx, int nr, int timeout_ms,
                      sil_aio_request *reqs[], int *processed);

/* Marcar una operación como cancelada. */
void sil_aio_cancel(sil_aio_context *ctx, int req_idx);

/* Obtener estado de una petición. */
int sil_aio_status(sil_aio_context *ctx, int req_idx);
bool sil_aio_success(int status);
int64_t sil_aio_bytes_transferred(int status);

/* API de nivel superior conveniente */

#define sil_aio_read(ctx, fd, buf, len, offset, flags) \
    sil_aio_operacion_general((ctx), (fd), (buf), (len), \
                               SIL_OP_READ,  (offset), (flags))

#define sil_aio_write(ctx, fd, buf, len, offset, flags) \
    sil_aio_operacion_general((ctx), (fd), (buf), (len), \
                               SIL_OP_WRITE, (offset), (flags))

#ifdef __cplusplus
extern "C" {
#endif

/* Operaciones internas */

typedef enum {
    SIL_OP_READ  = 0,
    SIL_OP_WRITE = 1,
    SIL_OP_CONNECT = 2,
    SIL_OP_ACCEPT = 3,
} sil_aio_op_type;

/* Submit una operación de lectura/escritura genérica. */
int sil_aio_operacion_general(sil_aio_context *ctx, int fd, void *buf,
                               size_t len, sil_aio_op_type op,
                               size_t offset, int flags);

#ifdef __cplusplus
}
#endif

#endif /* SIL_AIO_H */