#ifndef SIL_SERVICIO_H
#define SIL_SERVICIO_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

/* Network service runtime for SIL.
 *
 * Proporciona funciones para crear servidores TCP:
 * - sil_servicio_crear: crea un socket servidor, bind+listen
 * - sil_servicio_aceptar: acepta conexiones entrantes (bloqueante)
 * - sil_servicio_aceptar_async: versión async para integración con sil_aio
 * - sil_servicio_detener: cierra el socket servidor
 * - sil_servicio_fd: obtiene el fd del socket servidor
 */

#ifdef _WIN32
#include <winsock2.h>
#include <ws2tcpip.h>
typedef SOCKET SilSocketFd;
#define SIL_SOCKET_INVALID INVALID_SOCKET
#else
#include <sys/socket.h>
#include <netinet/in.h>
#include <unistd.h>
typedef int SilSocketFd;
#define SIL_SOCKET_INVALID (-1)
#endif

typedef struct SilServicio SilServicio;

/* Configuración del servicio */
typedef struct {
    int puerto;              // Puerto TCP (ej. 8080)
    const char *protocolo;   // "tcp" (futuro: "udp", "tls")
    int backlog;             // Tamaño de cola de conexiones pendientes (default 128)
    bool reutilizar_direccion; // SO_REUSEADDR (default true)
} SilServicioConfig;

/* Resultado de aceptar una conexión */
typedef struct {
    SilSocketFd cliente_fd;
    char ip[46];             // IPv4 o IPv6 en formato string
    int puerto;              // Puerto del cliente
} SilConexion;

/* Crea e inicializa un servicio TCP (socket + bind + listen).
 * Retorna puntero a SilServicio (opaco) o NULL en error.
 * El llamador debe llamar a sil_servicio_detener() para liberar. */
SilServicio* sil_servicio_crear(const SilServicioConfig *config);

/* Acepta una conexión entrante (bloqueante).
 * Retorna 0 en éxito (llena *conexion), -1 en error.
 * En Windows, requiere que WSAStartup ya se haya llamado. */
int sil_servicio_aceptar(SilServicio *servicio, SilConexion *conexion);

/* Obtiene el file descriptor del socket servidor (para uso con sil_aio/epoll).
 * Retorna SIL_SOCKET_INVALID si no está disponible. */
SilSocketFd sil_servicio_fd(const SilServicio *servicio);

/* Detiene y destruye el servicio (cierra socket, libera recursos). */
void sil_servicio_detener(SilServicio *servicio);

/* Verifica si el servicio está activo (socket abierto y escuchando). */
bool sil_servicio_activo(const SilServicio *servicio);

#endif // SIL_SERVICIO_H