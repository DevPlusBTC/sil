/* sil_aio_win.c - Async I/O para Windows usando Overlapped I/O + IOCP
 * 
 * Implementación completa de I/O asíncrono para archivos y sockets.
 * Usa CreateIoCompletionPort para alta performance.
 */

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdlib.h>
#include <string.h>
#include "sil_aio.h"

#define SIL_AIO_MAX_EVENTOS 64

typedef struct {
    OVERLAPPED overlapped;
    SilAioReq *req;
} SilAioOverlapped;

typedef struct {
    HANDLE iocp;
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
    
    /* Crear IOCP */
    interno->iocp = CreateIoCompletionPort(INVALID_HANDLE_VALUE, NULL, 0, 0);
    if (!interno->iocp) {
        free(interno);
        free(ctx);
        return NULL;
    }
    
    interno->pendientes = 0;
    ctx->interno = interno;
    return ctx;
}

void sil_aio_ctx_destruir(SilAioCtx *ctx) {
    if (!ctx || !ctx->interno) return;
    
    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    if (interno->iocp) {
        CloseHandle(interno->iocp);
    }
    free(interno);
    free(ctx);
}

/* Asociar handle al IOCP */
static bool asociar_handle(SilAioCtxInterno *interno, HANDLE handle) {
    HANDLE resultado = CreateIoCompletionPort(handle, interno->iocp, (ULONG_PTR)handle, 0);
    return resultado != NULL;
}

bool sil_aio_leer(SilAioCtx *ctx, SilAioReq *req) {
    if (!ctx || !req || !ctx->interno) return false;
    
    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    HANDLE handle = (HANDLE)req->fd;

    if (!asociar_handle(interno, handle)) return false;
    
    /* Preparar Overlapped */
    SilAioOverlapped *ol = (SilAioOverlapped*)malloc(sizeof(SilAioOverlapped));
    if (!ol) return false;
    
    memset(ol, 0, sizeof(SilAioOverlapped));
    ol->req = req;
    req->interno = ol;
    
    /* Configurar offset */
    ol->overlapped.Offset = (DWORD)(req->offset & 0xFFFFFFFF);
    ol->overlapped.OffsetHigh = (DWORD)(req->offset >> 32);
    
    DWORD bytes_leidos = 0;
    BOOL ok = ReadFile(
        handle,
        req->buffer,
        (DWORD)req->longitud,
        &bytes_leidos,
        &ol->overlapped
    );
    
    if (!ok) {
        DWORD error = GetLastError();
        if (error != ERROR_IO_PENDING) {
            free(ol);
            req->interno = NULL;
            return false;
        }
    }

    interno->pendientes++;
    return true;
}

bool sil_aio_escribir(SilAioCtx *ctx, SilAioReq *req) {
    if (!ctx || !req || !ctx->interno) return false;
    
    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    HANDLE handle = (HANDLE)req->fd;

    if (!asociar_handle(interno, handle)) return false;
    
    /* Preparar Overlapped */
    SilAioOverlapped *ol = (SilAioOverlapped*)malloc(sizeof(SilAioOverlapped));
    if (!ol) return false;
    
    memset(ol, 0, sizeof(SilAioOverlapped));
    ol->req = req;
    req->interno = ol;
    
    /* Configurar offset */
    ol->overlapped.Offset = (DWORD)(req->offset & 0xFFFFFFFF);
    ol->overlapped.OffsetHigh = (DWORD)(req->offset >> 32);
    
    DWORD bytes_escritos = 0;
    BOOL ok = WriteFile(
        handle,
        req->buffer,
        (DWORD)req->longitud,
        &bytes_escritos,
        &ol->overlapped
    );
    
    if (!ok) {
        DWORD error = GetLastError();
        if (error != ERROR_IO_PENDING) {
            free(ol);
            req->interno = NULL;
            return false;
        }
    }

    interno->pendientes++;
    return true;
}

int sil_aio_procesar(SilAioCtx *ctx, int max_eventos, int timeout_ms) {
    if (!ctx || !ctx->interno) return 0;
    
    SilAioCtxInterno *interno = (SilAioCtxInterno*)ctx->interno;
    DWORD bytes_transferidos;
    ULONG_PTR clave_completacion;
    LPOVERLAPPED overlapped;
    
    int procesados = 0;
    DWORD timeout = (timeout_ms < 0) ? INFINITE : (DWORD)timeout_ms;
    
    while (procesados < max_eventos) {
        BOOL ok = GetQueuedCompletionStatus(
            interno->iocp,
            &bytes_transferidos,
            &clave_completacion,
            &overlapped,
            timeout
        );
        
        if (!ok) {
            DWORD error = GetLastError();
            if (error == WAIT_TIMEOUT) {
                break;  // Timeout
            }
            if (!overlapped) {
                continue;  // Error en el IOCP mismo
            }
            // Error en la operación, pero overlapped es válido
        }
        
        if (!overlapped) {
            break;
        }
        
        SilAioOverlapped *ol = (SilAioOverlapped*)overlapped;
        SilAioReq *req = ol->req;

        if (ok) {
            req->resultado = (int)bytes_transferidos;
        } else {
            req->resultado = -(int)GetLastError();
        }

        if (interno->pendientes > 0) interno->pendientes--;

        if (req->callback) {
            req->callback(req, req->user_data, req->resultado);
        }

        free(ol);
        req->interno = NULL;
        procesados++;
    }
    
    return procesados;
}

bool sil_aio_cancelar(SilAioCtx *ctx, SilAioReq *req) {
    if (!ctx || !req || !req->interno) return false;
    
    SilAioOverlapped *ol = (SilAioOverlapped*)req->interno;
    HANDLE handle = (HANDLE)req->fd;
    
    BOOL ok = CancelIoEx(handle, &ol->overlapped);
    return ok != FALSE;
}

bool sil_aio_tiene_pendientes(SilAioCtx *ctx) {
    if (!ctx || !ctx->interno) return false;
    return ((SilAioCtxInterno*)ctx->interno)->pendientes > 0;
}