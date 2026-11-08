/* sil_servicio.c - Runtime de servicio de red para SIL
 *
 * Implementa servidor TCP con bind/listen/accept multiplataforma.
 * Windows: Winsock2 (IOCP-ready)
 * POSIX:   sockets BSD (epoll/kqueue/io_uring compatible)
 */

#include "sil_servicio.h"

#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#ifdef _WIN32
#include <winsock2.h>
#include <ws2tcpip.h>
#pragma comment(lib, "ws2_32.lib")
#else
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <unistd.h>
#include <fcntl.h>
#include <errno.h>
#endif

struct SilServicio {
    SilSocketFd fd;
    int puerto;
    bool activo;
};

/* Inicialización de Winsock en Windows (idempotente) */
#ifdef _WIN32
static bool winsock_init_done = false;
static void winsock_ensure_init(void) {
    if (!winsock_init_done) {
        WSADATA wsa;
        WSAStartup(MAKEWORD(2, 2), &wsa);
        winsock_init_done = true;
    }
}
#else
static void winsock_ensure_init(void) {}
#endif

SilServicio* sil_servicio_crear(const SilServicioConfig *config) {
    if (!config || config->puerto <= 0 || config->puerto > 65535) return NULL;

    winsock_ensure_init();

    SilServicio *svc = (SilServicio*)calloc(1, sizeof(SilServicio));
    if (!svc) return NULL;

    svc->fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (svc->fd == SIL_SOCKET_INVALID) {
        free(svc);
        return NULL;
    }

    /* SO_REUSEADDR para permitir reinicio rápido */
    if (config->reutilizar_direccion) {
        int opt = 1;
#ifdef _WIN32
        setsockopt(svc->fd, SOL_SOCKET, SO_REUSEADDR, (const char*)&opt, sizeof(opt));
#else
        setsockopt(svc->fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
#endif
    }

    /* Bind a INADDR_ANY (todas las interfaces) */
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = INADDR_ANY;
    addr.sin_port = htons((uint16_t)config->puerto);

    if (bind(svc->fd, (struct sockaddr*)&addr, sizeof(addr)) != 0) {
#ifdef _WIN32
        closesocket(svc->fd);
#else
        close(svc->fd);
#endif
        free(svc);
        return NULL;
    }

    /* Listen */
    int backlog = config->backlog > 0 ? config->backlog : 128;
    if (listen(svc->fd, backlog) != 0) {
#ifdef _WIN32
        closesocket(svc->fd);
#else
        close(svc->fd);
#endif
        free(svc);
        return NULL;
    }

    svc->puerto = config->puerto;
    svc->activo = true;
    return svc;
}

int sil_servicio_aceptar(SilServicio *servicio, SilConexion *conexion) {
    if (!servicio || !servicio->activo || servicio->fd == SIL_SOCKET_INVALID) return -1;
    if (!conexion) return -1;

    struct sockaddr_in addr;
#ifdef _WIN32
    int addrlen = sizeof(addr);
#else
    socklen_t addrlen = sizeof(addr);
#endif

    SilSocketFd cliente_fd = accept(servicio->fd, (struct sockaddr*)&addr, &addrlen);
    if (cliente_fd == SIL_SOCKET_INVALID) return -1;

    /* Convertir IP a string */
    const char *ip_str = inet_ntop(AF_INET, &addr.sin_addr, conexion->ip, sizeof(conexion->ip));
    if (!ip_str) {
        /* Fallback: copiar bytes manualmente si inet_ntop falla */
        snprintf(conexion->ip, sizeof(conexion->ip), "%u.%u.%u.%u",
                 (addr.sin_addr.s_addr >> 0) & 0xFF,
                 (addr.sin_addr.s_addr >> 8) & 0xFF,
                 (addr.sin_addr.s_addr >> 16) & 0xFF,
                 (addr.sin_addr.s_addr >> 24) & 0xFF);
    }
    conexion->puerto = ntohs(addr.sin_port);
    conexion->cliente_fd = cliente_fd;
    return 0;
}

SilSocketFd sil_servicio_fd(const SilServicio *servicio) {
    if (!servicio || !servicio->activo) return SIL_SOCKET_INVALID;
    return servicio->fd;
}

bool sil_servicio_activo(const SilServicio *servicio) {
    return servicio && servicio->activo && servicio->fd != SIL_SOCKET_INVALID;
}

void sil_servicio_detener(SilServicio *servicio) {
    if (!servicio) return;
    if (servicio->activo && servicio->fd != SIL_SOCKET_INVALID) {
#ifdef _WIN32
        shutdown(servicio->fd, SD_BOTH);
        closesocket(servicio->fd);
#else
        shutdown(servicio->fd, SHUT_RDWR);
        close(servicio->fd);
#endif
        servicio->fd = SIL_SOCKET_INVALID;
    }
    servicio->activo = false;
    free(servicio);
}