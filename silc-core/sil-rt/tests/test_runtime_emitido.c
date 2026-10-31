/* test_runtime_emitido.c - Pruebas sobre el runtime C que silc EMITE.
 *
 * El backend C99 embebe su propio runtime como texto (RUNTIME_ARENA,
 * RUNTIME_TIPOS, RUNTIME_CONFIANZA). Este test se compila contra la extraccion
 * literal de silc-backend/src/c99.rs, de modo que lo que se valida es
 * exactamente el codigo que recibe el usuario que ejecuta `silc build`.
 *
 * Requiere que scripts/run_c_tests.sh haya generado `runtime_emitido.h`.
 */
#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "runtime_emitido.h"
#include "sil_test.h"

static void arena_emitida_alinea(void) {
    static const size_t alineaciones[] = {1, 2, 4, 8, 16, 32, 64};
    static const size_t tamanios[] = {1, 3, 8, 15, 16, 33, 100, 4096};
    size_t i, j;
    SilArena_ a = sil_arena_crear_(64);
    for (i = 0; i < sizeof(alineaciones) / sizeof(alineaciones[0]); i++) {
        for (j = 0; j < sizeof(tamanios) / sizeof(tamanios[0]); j++) {
            void *p = sil_arena_asignar_(&a, tamanios[j], alineaciones[i]);
            ST_CHECK(p != NULL);
            if (!p) continue;
            ST_CHECK_EQ((uintptr_t)p % alineaciones[i], 0);
            memset(p, 0x7F, tamanios[j]); /* ASan: nada fuera del bloque */
        }
    }
    sil_arena_destruir_(&a);
}

static void arena_emitida_alineacion_cero(void) {
    SilArena_ a = sil_arena_crear_(1024);
    void *p = sil_arena_asignar_(&a, 8, 0);
    void *q = sil_arena_asignar_(&a, 8, 0);
    /* El runtime anterior con `al = 0` calculaba `a = 0` y devolvia siempre el
     * mismo puntero sin reservar memoria. */
    ST_CHECK(p != NULL);
    ST_CHECK(q != NULL);
    ST_CHECK(p != q);
    ST_CHECK_EQ((uintptr_t)p % SIL_ARENA_ALINEACION_DEFECTO_, 0);
    sil_arena_destruir_(&a);

    ST_CHECK(sil_arena_asignar_(&a, 8, 3) == NULL); /* potencia de dos */
    ST_CHECK(sil_arena_asignar_(&a, (size_t)-1, 8) == NULL); /* desbordamiento */
    p = sil_arena_asignar_(&a, 32, 8); /* arena destruida: crea bloque */
    ST_CHECK(p != NULL);
    memset(p, 0, 32);
    sil_arena_destruir_(&a);
}

static void arena_emitida_peticion_enorme(void) {
    SilArena_ a = sil_arena_crear_(1024);
    size_t grande = (size_t)SIL_ARENA_PAGE_ * 2 + 5;
    void *p = sil_arena_asignar_(&a, grande, 64);
    ST_CHECK(p != NULL);
    ST_CHECK_EQ((uintptr_t)p % 64, 0);
    memset(p, 0xAB, grande);
    ST_CHECK(a.asignado_total >= grande);
    sil_arena_destruir_(&a);
}

static void canales_emitidos_contrato(void) {
    long long h, v = 0;
    int i;

    /* Capacidad 0 -> valor por defecto (64). */
    h = sil_canal_crear_(0);
    ST_CHECK(h >= 0);
    ST_CHECK(sil_canal_enviar_(h, 99));
    ST_CHECK(sil_canal_recibir_(h, &v));
    ST_CHECK_EQ(v, 99);
    ST_CHECK(!sil_canal_recibir_(h, &v)); /* vacio */
    sil_canal_cerrar_(h);
    ST_CHECK(!sil_canal_enviar_(h, 1)); /* cerrado */
    sil_canal_destruir_(h);

    /* Capacidad 4: contrapresion y orden FIFO. */
    h = sil_canal_crear_(4);
    ST_CHECK(h >= 0);
    for (i = 0; i < 4; i++) ST_CHECK(sil_canal_enviar_(h, i + 1));
    ST_CHECK(!sil_canal_enviar_(h, 5)); /* lleno */
    for (i = 0; i < 4; i++) {
        ST_CHECK(sil_canal_recibir_(h, &v));
        ST_CHECK_EQ(v, i + 1);
    }
    ST_CHECK(!sil_canal_recibir_(h, &v));
    sil_canal_destruir_(h);

    /* Capacidad absurda: rechazo sin desbordar `sizeof(long long) * cap`. */
    ST_CHECK_EQ(sil_canal_crear_((1UL << 40)), -1);

    /* El pool se recicla: crear/destruir 100 canales no lo agota. */
    for (i = 0; i < 100; i++) {
        h = sil_canal_crear_(2);
        ST_CHECK(h >= 0);
        if (h < 0) break;
        sil_canal_destruir_(h);
    }
}

static void hmac_emitido(void) {
    static const char esperado[] =
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
    unsigned char clave[20];
    unsigned char mac[32];
    char hex[65];
    static const char digitos[] = "0123456789abcdef";
    int i;

    memset(clave, 0x0b, sizeof clave);
    sil_hmac_(clave, sizeof clave, (const unsigned char *)"Hi There", 8, mac);
    for (i = 0; i < 32; i++) {
        hex[i * 2] = digitos[mac[i] >> 4];
        hex[i * 2 + 1] = digitos[mac[i] & 0x0Fu];
    }
    hex[64] = '\0';
    ST_CHECK_STR(hex, esperado);
}

static void capacidades_emitidas(void) {
    SilCap_ cap;
    ST_CHECK_EQ(setenv("SIL_CAP_KEY",
                       "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
                       1),
                0);
    cap = sil_cap_solicitar_("/pagos", SIL_PERM_LEER_, 5000000000ull);
    ST_CHECK(cap.valida);
    ST_CHECK(sil_cap_validar_(&cap, "/pagos", SIL_PERM_LEER_));
    ST_CHECK(!sil_cap_validar_(&cap, "/pagos", SIL_PERM_ESCRIBIR_));
    ST_CHECK(!sil_cap_validar_(&cap, "/admin", SIL_PERM_LEER_));

    cap.firma[0] ^= 0xFF;
    ST_CHECK(!sil_cap_validar_(&cap, "/pagos", SIL_PERM_LEER_));

    cap = sil_cap_solicitar_("/pagos", SIL_PERM_LEER_, 5000000000ull);
    cap.expiracion_ns = 0; /* ya expirada */
    ST_CHECK(!sil_cap_validar_(&cap, "/pagos", SIL_PERM_LEER_));

    /* Sin clave no se emiten capacidades. */
    unsetenv("SIL_CAP_KEY");
    cap = sil_cap_solicitar_("/pagos", SIL_PERM_LEER_, 1);
    ST_CHECK(!cap.valida);
    ST_CHECK(!sil_cap_validar_(&cap, "/pagos", SIL_PERM_LEER_));
}

static int ejecutadas;
static void fibra_emitida(void *ctx) {
    if (ctx) (*(int *)ctx)++;
    ejecutadas++;
}

static void scheduler_emitido(void) {
    SilSched_ s;
    int ctx_a = 0, ctx_b = 0;
    unsigned long i;
    memset(&s, 0, sizeof s);
    ejecutadas = 0;
    ST_CHECK(sil_sched_spawn_(&s, fibra_emitida, &ctx_a));
    ST_CHECK(sil_sched_spawn_(&s, fibra_emitida, &ctx_b));
    ST_CHECK(!sil_sched_spawn_(&s, NULL, 0));
    sil_sched_ejecutar_(&s);
    ST_CHECK_EQ(ctx_a, 1);
    ST_CHECK_EQ(ctx_b, 1);
    ST_CHECK_EQ(ejecutadas, 2);
    /* Ejecutar de nuevo no repite trabajo ya terminado. */
    sil_sched_ejecutar_(&s);
    ST_CHECK_EQ(ejecutadas, 2);
    /* Limite del pool. */
    memset(&s, 0, sizeof s);
    for (i = 0; i < SIL_FIBRAS_MAX_; i++) {
        if (!sil_sched_spawn_(&s, fibra_emitida, 0)) {
            ST_FALLA("spawn fallo antes del limite del pool");
            break;
        }
    }
    ST_CHECK(!sil_sched_spawn_(&s, fibra_emitida, 0));
    ejecutadas = 0;
    sil_sched_ejecutar_(&s);
    ST_CHECK_EQ(ejecutadas, (int)SIL_FIBRAS_MAX_);
}

/* --- E/S por capacidades del runtime emitido -----------------------------
 * Se ejercitan sil_io_leer_, sil_io_escribir_, sil_red_* y el scheduler tal
 * como los emite silc: el fichero incluye la extraccion literal del runtime
 * (scripts/extraer_runtime_c.py), no una copia.
 */
#include <pthread.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <unistd.h>

static const char *CLAVE_HEX_RT =
    "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

static void fijar_clave(void) { setenv("SIL_CAP_KEY", CLAVE_HEX_RT, 1); }

static void escribir_archivo(const char *ruta, const char *datos, size_t n) {
    FILE *f = fopen(ruta, "wb");
    if (!f) return;
    if (n) fwrite(datos, 1, n, f);
    fclose(f);
}

static void *escritor_fifo(void *p) {
    const char *ruta = (const char *)p;
    FILE *f = fopen(ruta, "wb");
    if (!f) return NULL;
    fwrite("flujo-fifo!!", 1, 12, f);
    fclose(f);
    return NULL;
}


static void eos_leer_fichero_pequeno(void) {
    char ruta[64];
    char esperado[32] = "contenido";
    SilCap_ cap;
    SilTexto_ t;
    int fd;

    fijar_clave();
    snprintf(ruta, sizeof ruta, "/tmp/sil_io_XXXXXX");
    fd = mkstemp(ruta);
    ST_CHECK(fd >= 0);
    if (fd < 0) return;
    close(fd);
    escribir_archivo(ruta, esperado, 9);

    cap = sil_cap_solicitar_(ruta, SIL_PERM_LEER_, 5000000000ull);
    ST_CHECK(cap.valida);
    {
        SilArena_ a = sil_arena_crear_(1024);
        t = sil_io_leer_(&a, ruta, &cap);
        ST_CHECK_EQ(t.len, 9);
        ST_CHECK(t.ptr != NULL);
        ST_CHECK_STR(t.ptr, esperado); /* terminado en 0 */
        sil_arena_destruir_(&a);
    }
    unlink(ruta);
}

static void eos_leer_fichero_vacio_y_ausente(void) {
    char ruta[64];
    SilCap_ cap;
    SilTexto_ t;
    int fd;

    fijar_clave();
    snprintf(ruta, sizeof ruta, "/tmp/sil_vac_XXXXXX");
    fd = mkstemp(ruta); /* fichero vacio */
    ST_CHECK(fd >= 0);
    if (fd < 0) return;
    close(fd);
    cap = sil_cap_solicitar_(ruta, SIL_PERM_LEER_, 5000000000ull);
    {
        SilArena_ a = sil_arena_crear_(1024);
        size_t antes = a.asignado_total;
        t = sil_io_leer_(&a, ruta, &cap);
        ST_CHECK_EQ(t.len, 0);
        ST_CHECK_STR(t.ptr, "");
        /* Un fichero vacio no consume arena: se lee a un buffer de pila. */
        ST_CHECK_EQ(a.asignado_total, antes);
        sil_arena_destruir_(&a);
    }
    unlink(ruta);

    /* Ruta inexistente: vacio, sin romper. */
    cap = sil_cap_solicitar_("/tmp/sil_no_existe_98765", SIL_PERM_LEER_, 5000000000ull);
    {
        SilArena_ a = sil_arena_crear_(1024);
        t = sil_io_leer_(&a, "/tmp/sil_no_existe_98765", &cap);
        ST_CHECK_EQ(t.len, 0);
        sil_arena_destruir_(&a);
    }
}

static void eos_leer_fichero_grande(void) {
    /* 200 KiB: ejercita el crecimiento por trozos (el `ftell` anterior no
     * funciona con flujos no buscables y se limita a long). */
    char ruta[64];
    char *grande;
    const size_t n = 200u * 1024u;
    SilCap_ cap;
    SilTexto_ t;
    size_t i;
    int fd;

    fijar_clave();
    grande = (char *)malloc(n);
    ST_CHECK(grande != NULL);
    if (!grande) return;
    for (i = 0; i < n; i++) grande[i] = (char)('a' + (i % 26));

    snprintf(ruta, sizeof ruta, "/tmp/sil_big_XXXXXX");
    fd = mkstemp(ruta);
    ST_CHECK(fd >= 0);
    if (fd < 0) {
        free(grande);
        return;
    }
    close(fd);
    escribir_archivo(ruta, grande, n);

    cap = sil_cap_solicitar_(ruta, SIL_PERM_LEER_, 5000000000ull);
    {
        SilArena_ a = sil_arena_crear_(4096);
        t = sil_io_leer_(&a, ruta, &cap);
        ST_CHECK_EQ((size_t)t.len, n);
        if ((size_t)t.len == n) {
            ST_CHECK_EQ(memcmp(t.ptr, grande, n), 0);
            ST_CHECK_EQ(t.ptr[n], 0);
        }
        sil_arena_destruir_(&a);
    }
    free(grande);
    unlink(ruta);
}

static void eos_leer_flujo_no_buscable(void) {
    /* FIFO: `fopen` + `fread` funcionan; el `ftell` anterior devolvia -1 y
     * habria devuelto vacio. */
    char ruta[64];
    pthread_t h;
    SilCap_ cap;
    SilTexto_ t;

    fijar_clave();
    snprintf(ruta, sizeof ruta, "/tmp/sil_fifo_XXXXXX");
    {
        int fd = mkstemp(ruta);
        ST_CHECK(fd >= 0);
        if (fd < 0) return;
        close(fd);
        unlink(ruta);
    }
    ST_CHECK_EQ(mkfifo(ruta, 0600), 0);

    if (pthread_create(&h, NULL, escritor_fifo, ruta) != 0) {
        ST_FALLA("no se pudo crear el hilo escritor del FIFO");
        unlink(ruta);
        return;
    }
    cap = sil_cap_solicitar_(ruta, SIL_PERM_LEER_, 5000000000ull);
    {
        SilArena_ a = sil_arena_crear_(1024);
        t = sil_io_leer_(&a, ruta, &cap);
        ST_CHECK_EQ(t.len, 12);
        ST_CHECK_STR(t.ptr, "flujo-fifo!!");
        sil_arena_destruir_(&a);
    }
    pthread_join(h, NULL);
    unlink(ruta);
}

/* --- Red: servidor loopback en un hilo ---------------------------------- */

typedef struct {
    int modo;         /* 0 = leer lo enviado, 1 = enviar una respuesta */
    int escucha;
    int puerto;
    int bytes_vistos;
    volatile int listo;
} ServidorPrueba;

static void *hilo_servidor(void *p) {
    ServidorPrueba *s = (ServidorPrueba *)p;
    struct sockaddr_in dir;
    socklen_t largo = sizeof dir;
    int c;
    char buf[4096];
    ssize_t n;

    if (getsockname(s->escucha, (struct sockaddr *)&dir, &largo) == 0) {
        s->puerto = (int)ntohs(dir.sin_port);
    }
    s->listo = 1;
    c = accept(s->escucha, NULL, NULL);
    if (c < 0) return NULL;
    if (s->modo == 0) {
        n = recv(c, buf, sizeof buf, 0);
        s->bytes_vistos = (n > 0) ? (int)n : -1;
    } else {
        n = send(c, "respuesta", 9, 0);
        s->bytes_vistos = (n > 0) ? (int)n : -1;
        shutdown(c, SHUT_WR);
    }
    close(c);
    return NULL;
}

static int servidor_arrancar(ServidorPrueba *s, int modo, char *dest, size_t n) {
    struct sockaddr_in dir;
    pthread_t h;
    int escucha = socket(AF_INET, SOCK_STREAM, 0);
    if (escucha < 0) return 0;
    memset(&dir, 0, sizeof dir);
    dir.sin_family = AF_INET;
    dir.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    dir.sin_port = 0;
    if (bind(escucha, (struct sockaddr *)&dir, sizeof dir) != 0) {
        close(escucha);
        return 0;
    }
    if (listen(escucha, 1) != 0) {
        close(escucha);
        return 0;
    }
    memset(s, 0, sizeof *s);
    s->modo = modo;
    s->escucha = escucha;
    if (pthread_create(&h, NULL, hilo_servidor, s) != 0) {
        close(escucha);
        return 0;
    }
    pthread_detach(h);
    while (!s->listo) usleep(1000);
    snprintf(dest, n, "127.0.0.1:%d", s->puerto);
    return s->puerto > 0;
}

static void red_enviar_y_recibir(void) {
    ServidorPrueba srv;
    char dest[64];
    SilCap_ cap;
    long long n;

    fijar_clave();
    if (!servidor_arrancar(&srv, 0, dest, sizeof dest)) {
        ST_FALLA("no se pudo arrancar el servidor loopback");
        return;
    }
    cap = sil_cap_solicitar_(dest, SIL_PERM_RED_, 5000000000ull);
    ST_CHECK(cap.valida);
    n = sil_red_enviar_(dest, (const unsigned char *)"hola!", 5);
    ST_CHECK_EQ(n, 5);
    close(srv.escucha);
    {
        int esperas = 0;
        while (srv.bytes_vistos == 0 && esperas++ < 2000) usleep(1000);
    }
    ST_CHECK_EQ(srv.bytes_vistos, 5);

    /* Recibir binario: el servidor envia "respuesta" y cierra. */
    if (!servidor_arrancar(&srv, 1, dest, sizeof dest)) {
        ST_FALLA("no se pudo arrancar el servidor loopback (modo recv)");
        return;
    }
    cap = sil_cap_solicitar_(dest, SIL_PERM_RED_, 5000000000ull);
    {
        unsigned char buf[64];
        memset(buf, 0, sizeof buf);
        n = sil_red_recibir_(dest, buf, sizeof buf);
        ST_CHECK_EQ(n, 9);
        ST_CHECK_EQ(memcmp(buf, "respuesta", 9), 0);
    }
    close(srv.escucha);

    /* Recibir texto: arena + terminador. */
    if (!servidor_arrancar(&srv, 1, dest, sizeof dest)) {
        ST_FALLA("no se pudo arrancar el servidor loopback (modo texto)");
        return;
    }
    cap = sil_cap_solicitar_(dest, SIL_PERM_RED_, 5000000000ull);
    {
        SilArena_ a = sil_arena_crear_(4096);
        SilTexto_ t = sil_red_recibir_t_(&a, dest, &cap);
        ST_CHECK_EQ(t.len, 9);
        ST_CHECK_STR(t.ptr, "respuesta");
        sil_arena_destruir_(&a);
    }
    close(srv.escucha);

    /* Destino sin puerto valido o inalcanzable: -1, sin romper. */
    ST_CHECK_EQ(sil_red_enviar_("sin-puerto", (const unsigned char *)"x", 1), -1);
    ST_CHECK_EQ(sil_red_enviar_(NULL, (const unsigned char *)"x", 1), -1);
    ST_CHECK_EQ(sil_red_recibir_("127.0.0.1:1", (unsigned char *)dest, 8) < 0, 1);
}

static void eos_y_red_respetan_capacidades(void) {
    SilCap_ cap;
    SilTexto_ t;
    char ruta[64];
    int fd;

    fijar_clave();
    snprintf(ruta, sizeof ruta, "/tmp/sil_cap_XXXXXX");
    fd = mkstemp(ruta);
    ST_CHECK(fd >= 0);
    if (fd < 0) return;
    close(fd);
    escribir_archivo(ruta, "secreto", 7);

    /* Capacidad de otro recurso: el hash no coincide. */
    cap = sil_cap_solicitar_("/tmp/otro_sitio", SIL_PERM_LEER_, 5000000000ull);
    ST_CHECK(cap.valida);
    {
        SilArena_ a = sil_arena_crear_(1024);
        SilTexto_ datos;
        datos.ptr = "x";
        datos.len = 1;
        t = sil_io_leer_(&a, ruta, &cap);
        ST_CHECK_EQ(t.len, 0);
        ST_CHECK_EQ(sil_io_escribir_(ruta, datos, &cap), -1);
        sil_arena_destruir_(&a);
    }

    /* Capacidad solo de lectura: escribir se rechaza. */
    cap = sil_cap_solicitar_(ruta, SIL_PERM_LEER_, 5000000000ull);
    {
        SilTexto_ datos;
        datos.ptr = "x";
        datos.len = 1;
        ST_CHECK_EQ(sil_io_escribir_(ruta, datos, &cap), -1);
    }

    /* Longitud negativa: se rechaza antes de intentar escribir nada (antes se
     * convertia a size_t y pedia escribir 2^64-1 bytes). */
    cap = sil_cap_solicitar_(ruta, SIL_PERM_ESCRIBIR_, 5000000000ull);
    {
        SilTexto_ datos;
        datos.ptr = "x";
        datos.len = -1;
        ST_CHECK_EQ(sil_io_escribir_(ruta, datos, &cap), -1);
    }

    /* Escritura legitima y relectura. */
    {
        SilTexto_ datos;
        SilArena_ a = sil_arena_crear_(1024);
        datos.ptr = "0123456789";
        datos.len = 10;
        ST_CHECK_EQ(sil_io_escribir_(ruta, datos, &cap), 10);
        cap = sil_cap_solicitar_(ruta, SIL_PERM_LEER_, 5000000000ull);
        t = sil_io_leer_(&a, ruta, &cap);
        ST_CHECK_EQ(t.len, 10);
        ST_CHECK_STR(t.ptr, "0123456789");
        sil_arena_destruir_(&a);
    }

    /* Sin clave no se pueden emitir capacidades: todo queda denegado. */
    unsetenv("SIL_CAP_KEY");
    cap = sil_cap_solicitar_(ruta, SIL_PERM_LEER_, 5000000000ull);
    ST_CHECK(!cap.valida);
    {
        SilArena_ a = sil_arena_crear_(1024);
        t = sil_io_leer_(&a, ruta, &cap);
        ST_CHECK_EQ(t.len, 0);
        sil_arena_destruir_(&a);
    }
    unlink(ruta);
}

int main(void) {
    ST_CASO(arena_emitida_alinea);
    ST_CASO(arena_emitida_alineacion_cero);
    ST_CASO(arena_emitida_peticion_enorme);
    ST_CASO(canales_emitidos_contrato);
    ST_CASO(hmac_emitido);
    ST_CASO(capacidades_emitidas);
    ST_CASO(scheduler_emitido);
    ST_CASO(eos_leer_fichero_pequeno);
    ST_CASO(eos_leer_fichero_vacio_y_ausente);
    ST_CASO(eos_leer_fichero_grande);
    ST_CASO(eos_leer_flujo_no_buscable);
    ST_CASO(red_enviar_y_recibir);
    ST_CASO(eos_y_red_respetan_capacidades);
    ST_FIN();
}
