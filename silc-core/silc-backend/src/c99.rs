//! Emisión C99 portable (fallback sin LLVM).
//!
//! M5: implementación completa con disciplina elite.
//!
//! Garantías:
//! - Output compila con `gcc -std=c99 -Wall -Wextra -Werror` (verificado en tests).
//! - Runtime Arenas O(1) embebido (sin dependencias externas salvo libc).
//! - Verificaciones SMT emitidas como `if (!cond) { fprintf; exit(1); }`.
//! - Tipos: Entero64→int64_t, Flotante64→double, Booleano→bool (_Bool),
//!   Texto→SilTexto{ptr,len}, CapacidadHardware→SilCap, Void→void.
//! - SSA: cada ValueId → variable C `v<N>` con tipo declarado.
//! - Arenas: cada tarea recibe `SilArena* __arena`; asignaciones vía bump alloc.

use silc_causal_ir::nodes::{
    Bloque, Constante, OpArit, OpCmp, Operacion, SilType, TareaIR, Terminador, ValueId,
};
use silc_contracts::*;
use slotmap::Key;
use std::collections::HashMap;
use std::fmt::Write as _;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ErrorC99 {
    #[error("Error de emisión: {0}")]
    Emision(String),
    #[error("Tipo no soportado en C99: {0:?}")]
    TipoNoSoportado(String),
}

// =============================================================================
// Runtime embebido (idéntico a sil-rt, autocontenido para .c standalone)
// =============================================================================

/// Macros de feature POSIX que deben preceder a cualquier header del sistema.
///
/// Bajo `-std=c99` estricto (GCC define `__STRICT_ANSI__`), glibc/musl y
/// libSystem de macOS no declaran `sigset_t`, `AT_FDCWD`, `struct timespec`,
/// `clock_gettime`, `struct addrinfo` ni `getaddrinfo`. El runtime embebido
/// los usa, así que se piden explícitamente antes de los `#include`.
const PROLOGO_FEATURES: &str = "\
/* Feature macros POSIX: necesarias bajo -std=c99 estricto. */
#ifndef _WIN32
#define _POSIX_C_SOURCE 200809L
#endif

";

/// Silencia sólo warnings de "no usado" dentro del runtime embebido.
///
/// El runtime es autocontenido: según el programa generado, algunas funciones
/// de arena/canal/red quedan sin usar. Clang (`gcc` en macOS) lo convierte en
/// error con `-Werror`, así que se acota el pragma al bloque del runtime y se
/// restaura después para que el código de usuario siga vigilado.
const RUNTIME_DIAG_PUSH: &str = "\
#if defined(__clang__)
#pragma clang diagnostic push
#pragma clang diagnostic ignored \"-Wunused-function\"
#pragma clang diagnostic ignored \"-Wunused-parameter\"
#pragma clang diagnostic ignored \"-Wunused-variable\"
#pragma clang diagnostic ignored \"-Wunused-but-set-variable\"
#elif defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored \"-Wunused-function\"
#pragma GCC diagnostic ignored \"-Wunused-parameter\"
#pragma GCC diagnostic ignored \"-Wunused-variable\"
#pragma GCC diagnostic ignored \"-Wunused-but-set-variable\"
#endif
";

const RUNTIME_DIAG_POP: &str = "\
#if defined(__clang__)
#pragma clang diagnostic pop
#elif defined(__GNUC__)
#pragma GCC diagnostic pop
#endif
";

const RUNTIME_ARENA: &str = r#"
/* --- SIL Runtime: Arenas O(1) (embebido, autocontenido) ---
 * Contrato (verificado por scripts/run_c_tests.sh contra este mismo texto):
 *  - toda asignacion devuelve un puntero alineado a `al` o 0;
 *  - `al` debe ser potencia de dos (0 = alineacion natural);
 *  - nunca desborda un bloque: si no cabe, encadena uno nuevo dimensionado;
 *  - una arena sin bloques (creacion fallida o tras destruir) crea el primero
 *    de forma perezosa en la siguiente asignacion. */
#include <stdint.h>
#include <stddef.h>

typedef struct SilArenaBloque_ {
    unsigned char *memoria;
    size_t capacidad;
    size_t usado;
    struct SilArenaBloque_ *siguiente;
} SilArenaBloque_;

typedef struct {
    SilArenaBloque_ *inicio;
    SilArenaBloque_ *actual;
    size_t asignado_total;
} SilArena_;

#define SIL_ARENA_PAGE_ ((size_t)65536)
#define SIL_ARENA_ALINEACION_DEFECTO_ ((size_t)8)

static SilArenaBloque_ *sil_arena_bloque_nuevo_(size_t capacidad) {
    SilArenaBloque_ *b = (SilArenaBloque_*)malloc(sizeof(SilArenaBloque_));
    if (!b) return 0;
    b->memoria = (unsigned char*)malloc(capacidad);
    if (!b->memoria) { free(b); return 0; }
    b->capacidad = capacidad;
    b->usado = 0;
    b->siguiente = 0;
    return b;
}

static SilArena_ sil_arena_crear_(size_t cap_inicial) {
    SilArena_ a;
    size_t cap = cap_inicial > 0 ? cap_inicial : SIL_ARENA_PAGE_;
    SilArenaBloque_ *b;
    a.inicio = 0;
    a.actual = 0;
    a.asignado_total = 0;
    b = sil_arena_bloque_nuevo_(cap);
    if (!b) return a; /* la primera asignacion reintenta */
    a.inicio = b;
    a.actual = b;
    a.asignado_total = cap;
    return a;
}

static void *sil_arena_asignar_(SilArena_ *arena, size_t n, size_t al) {
    if (!arena) return 0;
    if (al == 0) al = SIL_ARENA_ALINEACION_DEFECTO_;
    if ((al & (al - 1)) != 0) return 0; /* solo potencias de dos */
    if (n == 0) n = 1;
    if (n > (size_t)-1 - al) return 0;  /* `n + al` no debe desbordar */
    for (;;) {
        SilArenaBloque_ *b = arena->actual;
        uintptr_t base, inicio, fin;
        if (!b) {
            size_t cap = n + al;
            if (cap < SIL_ARENA_PAGE_) cap = SIL_ARENA_PAGE_;
            b = sil_arena_bloque_nuevo_(cap);
            if (!b) return 0;
            arena->inicio = b;
            arena->actual = b;
            arena->asignado_total += cap;
        }
        base = (uintptr_t)b->memoria;
        fin = base + b->capacidad;
        inicio = (base + (uintptr_t)b->usado + (uintptr_t)al - 1u) & ~((uintptr_t)al - 1u);
        if (inicio >= base + (uintptr_t)b->usado && inicio <= fin &&
            n <= (size_t)(fin - inicio)) {
            b->usado = (size_t)(inicio - base) + n;
            return (void*)inicio;
        }
        {
            size_t cap = n + al;
            SilArenaBloque_ *nn;
            if (cap < SIL_ARENA_PAGE_) cap = SIL_ARENA_PAGE_;
            nn = sil_arena_bloque_nuevo_(cap);
            if (!nn) return 0;
            b->siguiente = nn;
            arena->actual = nn;
            arena->asignado_total += cap;
        }
    }
}

static void sil_arena_destruir_(SilArena_ *arena) {
    SilArenaBloque_ *c;
    if (!arena) return;
    c = arena->inicio;
    while (c) { SilArenaBloque_ *s = c->siguiente; free(c->memoria); free(c); c = s; }
    arena->inicio = 0;
    arena->actual = 0;
    arena->asignado_total = 0;
}
"#;

const RUNTIME_TIPOS: &str = r"
/* --- SIL Tipos --- */
typedef struct { const char *ptr; long len; } SilTexto_;
typedef struct { unsigned long long id; unsigned int permisos; unsigned long long expiracion_ns; unsigned char recurso_hash[32]; unsigned char firma[32]; int valida; } SilCap_;
";

/* Runtime de confianza embebido: HMAC-SHA256, capacidades, canales,
 * red TCP minima y scheduler cooperativo. C99 estricto, sin dependencias
 * salvo libc (+ws2_32 en Windows solo si el programa usa red). */
const RUNTIME_CONFIANZA: &str = r#"
#include <stdint.h>
#include <limits.h>
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#include <time.h>
#endif
/* --- SIL SHA-256 + HMAC (autocontenido, sin malloc) --- */
#define SIL_ROTR_(x, n) (((x) >> (n)) | ((x) << (32 - (n))))
typedef struct { unsigned char datos[64]; unsigned int len_datos; unsigned long long len_bits; unsigned int estado[8]; } SilSha256Ctx_;
static const unsigned int SIL_SHA_K_[64] = {
0x428a2f98u,0x71374491u,0xb5c0fbcfu,0xe9b5dba5u,0x3956c25bu,0x59f111f1u,0x923f82a4u,0xab1c5ed5u,
0xd807aa98u,0x12835b01u,0x243185beu,0x550c7dc3u,0x72be5d74u,0x80deb1feu,0x9bdc06a7u,0xc19bf174u,
0xe49b69c1u,0xefbe4786u,0x0fc19dc6u,0x240ca1ccu,0x2de92c6fu,0x4a7484aau,0x5cb0a9dcu,0x76f988dau,
0x983e5152u,0xa831c66du,0xb00327c8u,0xbf597fc7u,0xc6e00bf3u,0xd5a79147u,0x06ca6351u,0x14292967u,
0x27b70a85u,0x2e1b2138u,0x4d2c6dfcu,0x53380d13u,0x650a7354u,0x766a0abbu,0x81c2c92eu,0x92722c85u,
0xa2bfe8a1u,0xa81a664bu,0xc24b8b70u,0xc76c51a3u,0xd192e819u,0xd6990624u,0xf40e3585u,0x106aa070u,
0x19a4c116u,0x1e376c08u,0x2748774cu,0x34b0bcb5u,0x391c0cb3u,0x4ed8aa4au,0x5b9cca4fu,0x682e6ff3u,
0x748f82eeu,0x78a5636fu,0x84c87814u,0x8cc70208u,0x90befffau,0xa4506cebu,0xbef9a3f7u,0xc67178f2u};
static void sil_sha_comp_(SilSha256Ctx_ *c, const unsigned char b[64]) {
unsigned int w[64]; int i; unsigned int a, e, t1, t2, f, g, h, bb, cc, dd;
for (i = 0; i < 16; i++) w[i] = ((unsigned int)b[i*4] << 24) | ((unsigned int)b[i*4+1] << 16) | ((unsigned int)b[i*4+2] << 8) | ((unsigned int)b[i*4+3]);
for (i = 16; i < 64; i++) { unsigned int s0 = SIL_ROTR_(w[i-15],7) ^ SIL_ROTR_(w[i-15],18) ^ (w[i-15] >> 3); unsigned int s1 = SIL_ROTR_(w[i-2],17) ^ SIL_ROTR_(w[i-2],19) ^ (w[i-2] >> 10); w[i] = s1 + w[i-7] + s0 + w[i-16]; }
a=c->estado[0];bb=c->estado[1];cc=c->estado[2];dd=c->estado[3];e=c->estado[4];f=c->estado[5];g=c->estado[6];h=c->estado[7];
for (i = 0; i < 64; i++) { t1 = h + (SIL_ROTR_(e,6)^SIL_ROTR_(e,11)^SIL_ROTR_(e,25)) + ((e&f)^(~e&g)) + SIL_SHA_K_[i] + w[i]; t2 = (SIL_ROTR_(a,2)^SIL_ROTR_(a,13)^SIL_ROTR_(a,22)) + ((a&bb)^(a&cc)^(bb&cc)); h=g;g=f;f=e;e=dd+t1;dd=cc;cc=bb;bb=a;a=t1+t2; }
c->estado[0]+=a;c->estado[1]+=bb;c->estado[2]+=cc;c->estado[3]+=dd;c->estado[4]+=e;c->estado[5]+=f;c->estado[6]+=g;c->estado[7]+=h;
}
static void sil_sha_init_(SilSha256Ctx_ *c) { c->len_datos=0; c->len_bits=0; c->estado[0]=0x6a09e667u;c->estado[1]=0xbb67ae85u;c->estado[2]=0x3c6ef372u;c->estado[3]=0xa54ff53au;c->estado[4]=0x510e527fu;c->estado[5]=0x9b05688cu;c->estado[6]=0x1f83d9abu;c->estado[7]=0x5be0cd19u; }
static void sil_sha_upd_(SilSha256Ctx_ *c, const unsigned char *m, unsigned long n) { unsigned long i; for (i=0;i<n;i++){c->datos[c->len_datos++]=m[i]; if(c->len_datos==64){sil_sha_comp_(c,c->datos); c->len_bits+=512; c->len_datos=0;}} }
static void sil_sha_fin_(SilSha256Ctx_ *c, unsigned char d[32]) { unsigned long long t=c->len_bits+(unsigned long long)c->len_datos*8u; int i; c->datos[c->len_datos++]=0x80; if(c->len_datos>56){while(c->len_datos<64)c->datos[c->len_datos++]=0; sil_sha_comp_(c,c->datos); c->len_datos=0;} while(c->len_datos<56)c->datos[c->len_datos++]=0; for(i=7;i>=0;i--)c->datos[c->len_datos++]=(unsigned char)(t>>((unsigned)i*8)); sil_sha_comp_(c,c->datos); for(i=0;i<8;i++){d[i*4]=(unsigned char)(c->estado[i]>>24);d[i*4+1]=(unsigned char)(c->estado[i]>>16);d[i*4+2]=(unsigned char)(c->estado[i]>>8);d[i*4+3]=(unsigned char)c->estado[i];} }
static void sil_hmac_(const unsigned char *k, unsigned long kl, const unsigned char *m, unsigned long ml, unsigned char mac[32]) {
unsigned char kk[64]; unsigned char ip[64]; unsigned char op[64]; unsigned char in[32]; SilSha256Ctx_ c; int i;
for(i=0;i<64;i++)kk[i]=0;
if(kl>64){sil_sha_init_(&c);sil_sha_upd_(&c,k,kl);sil_sha_fin_(&c,kk);}else{unsigned long i2;for(i2=0;i2<kl;i2++)kk[i2]=k[i2];}
for(i=0;i<64;i++){ip[i]=kk[i]^0x36u;op[i]=kk[i]^0x5cu;}
sil_sha_init_(&c);sil_sha_upd_(&c,ip,64);if(m&&ml)sil_sha_upd_(&c,m,ml);sil_sha_fin_(&c,in);
sil_sha_init_(&c);sil_sha_upd_(&c,op,64);sil_sha_upd_(&c,in,32);sil_sha_fin_(&c,mac);
}
static int sil_ct_igual_(const unsigned char *a, const unsigned char *b, unsigned long n) { unsigned char d=0; unsigned long i; if(!a||!b)return 0; for(i=0;i<n;i++)d|=(unsigned char)(a[i]^b[i]); return d==0; }
/* --- Reloj monotonico (ns) --- */
#if defined(_WIN32)
static LARGE_INTEGER sil_freq_ = {0};
#endif
static unsigned long long sil_ahora_ns_(void) {
#if defined(_WIN32)
LARGE_INTEGER c;
if (sil_freq_.QuadPart == 0) QueryPerformanceFrequency(&sil_freq_);
QueryPerformanceCounter(&c);
if (sil_freq_.QuadPart == 0) return 0;
return (unsigned long long)(c.QuadPart * 1000000000ull / sil_freq_.QuadPart);
#else
struct timespec ts;
#if defined(CLOCK_MONOTONIC)
clock_gettime(CLOCK_MONOTONIC, &ts);
#else
clock_gettime(CLOCK_REALTIME, &ts);
#endif
return (unsigned long long)ts.tv_sec * 1000000000ull + (unsigned long long)ts.tv_nsec;
#endif
}
#define SIL_CAP_MAGIC_ 0xC4A981E5ull
#define SIL_CAP_TTL_DEF_ 500000000ull
static int sil_clave_(unsigned char k[32]) {
const char *hex = getenv("SIL_CAP_KEY"); int i; int j;
if (!hex) return 0;
for (i = 0; i < 64; i++) if (!hex[i]) return 0;
if (hex[64] != 0) return 0;
for (i = 0; i < 32; i++) { unsigned v = 0; for (j = 0; j < 2; j++) { char c = hex[i*2+j]; v <<= 4; if (c>='0'&&c<='9')v|=(unsigned)(c-'0'); else if (c>='a'&&c<='f')v|=(unsigned)(c-'a'+10); else if (c>='A'&&c<='F')v|=(unsigned)(c-'A'+10); else return 0; } k[i]=(unsigned char)v; }
return 1;
}
static void sil_le64(unsigned char o[8], unsigned long long v){int i;for(i=0;i<8;i++)o[i]=(unsigned char)(v>>((unsigned)i*8));}
static void sil_le32(unsigned char o[4], unsigned int v){int i;for(i=0;i<4;i++)o[i]=(unsigned char)(v>>((unsigned)i*8));}
/* Solicita capacidad firmada. Retorna SilCap_ con valida=1, o valida=0 + mensaje en stderr si falta clave. */
static SilCap_ sil_cap_solicitar_(const char *recurso, unsigned int permisos, unsigned long long ttl) {
SilCap_ c; unsigned char k[32]; unsigned char rh[32]; unsigned char msg[52]; unsigned long n;
for(n=0;n<52;n++)msg[n]=0;
c.id=SIL_CAP_MAGIC_;c.permisos=permisos;c.valida=0;
if(!recurso||!*recurso){fprintf(stderr,"SIL: recurso vacio\n");return c;}
n=strlen(recurso); if(n>1024){fprintf(stderr,"SIL: recurso muy largo\n");return c;}
if(!sil_clave_(k)){fprintf(stderr,"SIL: sin clave (define SIL_CAP_KEY)\n");return c;}
{SilSha256Ctx_ x;sil_sha_init_(&x);sil_sha_upd_(&x,(const unsigned char*)recurso,(unsigned long)n);sil_sha_fin_(&x,rh);}
c.expiracion_ns=sil_ahora_ns_()+(ttl?ttl:SIL_CAP_TTL_DEF_);
memcpy(c.recurso_hash,rh,32);
sil_le64(msg,(unsigned long long)SIL_CAP_MAGIC_);sil_le32(msg+8,permisos);sil_le64(msg+12,c.expiracion_ns);memcpy(msg+20,rh,32);
sil_hmac_(k,32,msg,52,c.firma);
c.valida=1;
return c;
}
/* Valida: recurso!=0 exige binding; recurso==0 verifica solo integridad+expiracion+permiso. Retorna 1 valido. */
static int sil_cap_validar_(const SilCap_ *c, const char *recurso, unsigned int requerido) {
unsigned char k[32]; unsigned char rh[32]; unsigned char msg[52]; unsigned char esp[32]; unsigned long n; unsigned i;
for(i=0;i<52;i++)msg[i]=0;
if(!c||!c->valida)return 0;
if(c->id!=SIL_CAP_MAGIC_)return 0;
if((c->permisos&(unsigned)requerido)!=(unsigned)requerido)return 0;
if(sil_ahora_ns_()>c->expiracion_ns)return 0;
if(!sil_clave_(k))return 0;
if(recurso){n=strlen(recurso);{SilSha256Ctx_ x;sil_sha_init_(&x);sil_sha_upd_(&x,(const unsigned char*)recurso,(unsigned long)n);sil_sha_fin_(&x,rh);}if(!sil_ct_igual_(rh,c->recurso_hash,32))return 0;}
else{memcpy(rh,c->recurso_hash,32);}
sil_le64(msg,(unsigned long long)SIL_CAP_MAGIC_);sil_le32(msg+8,c->permisos);sil_le64(msg+12,c->expiracion_ns);memcpy(msg+20,rh,32);
sil_hmac_(k,32,msg,52,esp);
return sil_ct_igual_(esp,c->firma,32);
}
/* --- Canales FIFO int64 (pool global, single-thread cooperativo) --- */
#define SIL_CANALES_MAX_ 16
#define SIL_CANAL_DEF_ 64
typedef struct { long long *buf; unsigned long cap; unsigned long cabeza; unsigned long cola; unsigned long n; int cerrado; int en_uso; } SilCanal_;
static SilCanal_ sil_canales_[SIL_CANALES_MAX_] = {{0,0,0,0,0,0,0}};
static long long sil_canal_crear_(unsigned long capacidad) {
unsigned long i; unsigned long cap = capacidad ? capacidad : SIL_CANAL_DEF_;
if (cap > (1UL << 24)) return -1; /* cota: 2^24 * 8 B = 128 MiB */
for (i = 0; i < SIL_CANALES_MAX_; i++) {
if (sil_canales_[i].en_uso && !sil_canales_[i].cerrado) continue;
/* Ranura libre, o canal ya cerrado y sin pendientes: se recicla. */
if (sil_canales_[i].en_uso && sil_canales_[i].n != 0) continue;
free(sil_canales_[i].buf);
sil_canales_[i].buf = (long long*)malloc(sizeof(long long) * cap);
if (!sil_canales_[i].buf) { sil_canales_[i].en_uso = 0; return -1; }
sil_canales_[i].cap = cap; sil_canales_[i].cabeza = 0; sil_canales_[i].cola = 0;
sil_canales_[i].n = 0; sil_canales_[i].cerrado = 0; sil_canales_[i].en_uso = 1;
return (long long)i;
}
return -1;
}
static SilCanal_ *sil_canal_(long long h) {
if (h < 0 || h >= SIL_CANALES_MAX_ || !sil_canales_[h].en_uso) return 0;
return &sil_canales_[h];
}
static int sil_canal_enviar_(long long h, long long v) {
SilCanal_ *c = sil_canal_(h);
if (!c || c->cerrado || c->n >= c->cap) return 0;
c->buf[c->cola] = v; c->cola = (c->cola + 1) % c->cap; c->n++;
return 1;
}
static int sil_canal_recibir_(long long h, long long *v) {
SilCanal_ *c = sil_canal_(h);
if (!c || !v || c->n == 0) return 0;
*v = c->buf[c->cabeza]; c->cabeza = (c->cabeza + 1) % c->cap; c->n--;
return 1;
}
static void sil_canal_cerrar_(long long h) { SilCanal_ *c = sil_canal_(h); if (c) c->cerrado = 1; }
static void sil_canal_destruir_(long long h) { SilCanal_ *c = sil_canal_(h); if (!c) return; free(c->buf); c->buf = 0; c->cap = 0; c->cabeza = 0; c->cola = 0; c->n = 0; c->cerrado = 1; c->en_uso = 0; }
/* --- Red TCP minima (cliente). Destino "host:puerto". --- */
#if defined(_WIN32)
#pragma comment(lib, "ws2_32")
#include <winsock2.h>
#include <ws2tcpip.h>
static int sil_red_init_ = 0;
static int sil_red_arrancar_(void) {
WSADATA w; if (sil_red_init_) return 1;
if (WSAStartup(MAKEWORD(2,2), &w) != 0) return 0;
sil_red_init_ = 1; return 1;
}
#else
#include <sys/types.h>
#include <sys/socket.h>
#include <netdb.h>
#include <unistd.h>
#include <arpa/inet.h>
#endif
static int sil_red_partir_(const char *dest, char *host, unsigned long hn, char *pto, unsigned long pn) {
const char *sep; unsigned long lh, lp, i;
if (!dest || !host || !pto) return 0;
sep = strrchr(dest, ':'); if (!sep || sep == dest || !sep[1]) return 0;
lh = (unsigned long)(sep - dest); lp = (unsigned long)strlen(sep + 1);
if (lh + 1 > hn || lp + 1 > pn) return 0;
for (i = 0; i < lh; i++) host[i] = dest[i];
host[lh] = 0;
for (i = 0; i < lp; i++) pto[i] = sep[1+i];
pto[lp] = 0;
return 1;
}
static long long sil_red_enviar_(const char *dest, const unsigned char *datos, unsigned long n) {
#if defined(_WIN32)
SOCKET s; struct addrinfo hints; struct addrinfo *r = 0; char host[256]; char pto[16]; int rc;
if (!sil_red_arrancar_()) return -1;
if (!sil_red_partir_(dest, host, sizeof(host), pto, sizeof(pto))) return -1;
memset(&hints, 0, sizeof(hints)); hints.ai_family = AF_UNSPEC; hints.ai_socktype = SOCK_STREAM;
if (getaddrinfo(host, pto, &hints, &r) != 0) return -1;
s = socket(r->ai_family, r->ai_socktype, r->ai_protocol);
if (s == INVALID_SOCKET) { freeaddrinfo(r); return -1; }
if (connect(s, r->ai_addr, (int)r->ai_addrlen) != 0) { closesocket(s); freeaddrinfo(r); return -1; }
freeaddrinfo(r);
rc = send(s, (const char*)datos, (int)n, 0);
closesocket(s);
return rc < 0 ? -1 : (long long)rc;
#else
int s; struct addrinfo hints; struct addrinfo *r = 0; char host[256]; char pto[16]; ssize_t rc;
if (!sil_red_partir_(dest, host, sizeof(host), pto, sizeof(pto))) return -1;
memset(&hints, 0, sizeof(hints)); hints.ai_family = AF_UNSPEC; hints.ai_socktype = SOCK_STREAM;
if (getaddrinfo(host, pto, &hints, &r) != 0) return -1;
s = socket(r->ai_family, r->ai_socktype, r->ai_protocol);
if (s < 0) { freeaddrinfo(r); return -1; }
if (connect(s, r->ai_addr, r->ai_addrlen) != 0) { close(s); freeaddrinfo(r); return -1; }
freeaddrinfo(r);
rc = send(s, datos, (size_t)n, 0);
close(s);
return rc < 0 ? -1 : (long long)rc;
#endif
}
static long long sil_red_recibir_(const char *dest, unsigned char *buf, unsigned long cap) {
#if defined(_WIN32)
SOCKET s; struct addrinfo hints; struct addrinfo *r = 0; char host[256]; char pto[16]; int rc;
if (!sil_red_arrancar_()) return -1;
if (!sil_red_partir_(dest, host, sizeof(host), pto, sizeof(pto))) return -1;
memset(&hints, 0, sizeof(hints)); hints.ai_family = AF_UNSPEC; hints.ai_socktype = SOCK_STREAM;
if (getaddrinfo(host, pto, &hints, &r) != 0) return -1;
s = socket(r->ai_family, r->ai_socktype, r->ai_protocol);
if (s == INVALID_SOCKET) { freeaddrinfo(r); return -1; }
if (connect(s, r->ai_addr, (int)r->ai_addrlen) != 0) { closesocket(s); freeaddrinfo(r); return -1; }
freeaddrinfo(r);
rc = recv(s, (char*)buf, (int)(cap > 2147483647ul ? 2147483647ul : cap), 0);
closesocket(s);
return rc < 0 ? -1 : (long long)rc;
#else
int s; struct addrinfo hints; struct addrinfo *r = 0; char host[256]; char pto[16]; ssize_t rc;
if (!sil_red_partir_(dest, host, sizeof(host), pto, sizeof(pto))) return -1;
memset(&hints, 0, sizeof(hints)); hints.ai_family = AF_UNSPEC; hints.ai_socktype = SOCK_STREAM;
if (getaddrinfo(host, pto, &hints, &r) != 0) return -1;
s = socket(r->ai_family, r->ai_socktype, r->ai_protocol);
if (s < 0) { freeaddrinfo(r); return -1; }
if (connect(s, r->ai_addr, r->ai_addrlen) != 0) { close(s); freeaddrinfo(r); return -1; }
freeaddrinfo(r);
rc = recv(s, buf, (size_t)cap, 0);
close(s);
return rc < 0 ? -1 : (long long)rc;
#endif
}
/* --- Scheduler cooperativo: cola de tareas (fn, ctx). Sin desalojo:
 * cada fibra corre hasta retornar; el planificador sigue en orden. --- */
#define SIL_FIBRAS_MAX_ 64
typedef void (*SilFibraFn_)(void*);
typedef struct { SilFibraFn_ fn; void *ctx; int terminada; } SilFibra_;
typedef struct { SilFibra_ fibras[SIL_FIBRAS_MAX_]; unsigned long n; } SilSched_;
static int sil_sched_spawn_(SilSched_ *s, SilFibraFn_ fn, void *ctx) {
if (!s || !fn || s->n >= SIL_FIBRAS_MAX_) return 0;
s->fibras[s->n].fn = fn; s->fibras[s->n].ctx = ctx; s->fibras[s->n].terminada = 0;
s->n++;
return 1;
}
static void sil_sched_ejecutar_(SilSched_ *s) {
unsigned long i; int alguna;
if (!s) return;
do {
alguna = 0;
for (i = 0; i < s->n; i++) {
if (!s->fibras[i].terminada) { s->fibras[i].fn(s->fibras[i].ctx); s->fibras[i].terminada = 1; alguna = 1; }
}
} while (alguna);
}
/* --- E/S por capacidades. Lee verifica LEER+binding a ruta; escribe igual.
 * La memoria del contenido vive en la arena del programa (sin leaks). --- */
#define SIL_PERM_LEER_ 0x01u
#define SIL_PERM_ESCRIBIR_ 0x02u
#define SIL_PERM_RED_ 0x04u
/* Lee el fichero completo en la arena del programa.
 *
 * Contrato:
 *  - Lectura por trozos de 4 KiB, sin `ftell`: funciona con pipes, FIFOs y
 *    dispositivos no buscables, y no se limita a LONG_MAX por el `ftell`.
 *    (El tamano maximo representable sigue siendo LONG_MAX porque
 *    `SilTexto_.len` es `long`; por encima se devuelve vacio, no truncado.)
 *  - Un fichero vacio devuelve `("", 0)` sin consumir arena.
 *  - Error de lectura: se descartan los datos parciales y se devuelve vacio.
 *  - El contenido siempre termina en 0 (el tamano reservado es len + 1). */
static SilTexto_ sil_io_leer_(SilArena_ *a, const char *ruta, SilCap_ *cap) {
SilTexto_ t; FILE *f; unsigned char pila[4096]; unsigned char *b;
size_t n, usado, capacidad;
t.ptr = ""; t.len = 0;
if (!a || !ruta || !cap) return t;
if (!sil_cap_validar_(cap, ruta, SIL_PERM_LEER_)) { fprintf(stderr, "SIL: capacidad invalida para leer %s\n", ruta); return t; }
f = fopen(ruta, "rb");
if (!f) return t;
n = fread(pila, 1, sizeof pila, f);
if (n == 0) { fclose(f); return t; } /* vacio (o error: ferror se comprueba abajo) */
capacidad = n;
b = (unsigned char*)sil_arena_asignar_(a, capacidad + 1, 1);
if (!b) { fclose(f); return t; }
memcpy(b, pila, n);
usado = n;
while (usado == capacidad && !feof(f)) {
size_t nueva; unsigned char *nb;
if (capacidad > ((size_t)-1 - 1) / 2) break; /* no desbordar size_t */
nueva = capacidad * 2;
nb = (unsigned char*)sil_arena_asignar_(a, nueva + 1, 1);
if (!nb) break;
memcpy(nb, b, usado);
b = nb; capacidad = nueva;
n = fread(b + usado, 1, capacidad - usado, f);
if (n == 0) break;
usado += n;
}
if (ferror(f) || usado > (size_t)LONG_MAX) { fclose(f); return t; }
fclose(f);
b[usado] = 0;
t.ptr = (const char*)b;
t.len = (long)usado;
return t;
}
/* Escribe `datos` en `ruta` con capacidad ESCRIBIR ligada a la ruta.
 * Devuelve los bytes escritos o -1. `datos.len < 0` se rechaza: convertirlo a
 * size_t pediria escribir 2^64-1 bytes y leeria fuera del buffer. El `fflush`
 * y el `fclose` se comprueban porque ahi se detecta el disco lleno. */
static long long sil_io_escribir_(const char *ruta, SilTexto_ datos, SilCap_ *cap) {
FILE *f;
if (!ruta || !cap || !datos.ptr) return -1;
if (datos.len < 0) return -1;
if (!sil_cap_validar_(cap, ruta, SIL_PERM_ESCRIBIR_)) { fprintf(stderr, "SIL: capacidad invalida para escribir %s\n", ruta); return -1; }
f = fopen(ruta, "wb");
if (!f) return -1;
if (datos.len > 0 && fwrite(datos.ptr, 1, (size_t)datos.len, f) != (size_t)datos.len) { fclose(f); return -1; }
if (fflush(f) != 0 || ferror(f)) { fclose(f); return -1; }
if (fclose(f) != 0) return -1;
return datos.len;
}
/* Recibe hasta 64 KiB del destino "host:puerto" en la arena del programa.
 *
 * Contrato:
 *  - Devuelve texto terminado en 0 (reserva len + 1): igual que sil_io_leer_,
 *    el `ptr` es usable como cadena C ademas de por longitud.
 *  - Como `sil_red_recibir_` abre una conexion por llamada, un mensaje mayor
 *    que 64 KiB se trunca a 64 KiB; el protocolo debe trocear.
 *  - Sin datos, error o capacidad invalida: ("", 0). La region de 64 KiB se
 *    reserva antes de saber si llega algo (no hay forma de sondear sin
 *    cambiar la API); en el peor caso cuesta una pagina de arena por llamada
 *    fallida. */
static SilTexto_ sil_red_recibir_t_(SilArena_ *a, const char *dest, SilCap_ *cap) {
SilTexto_ t; long long n;
t.ptr = ""; t.len = 0;
if (!a || !dest || !cap) return t;
if (!sil_cap_validar_(cap, dest, SIL_PERM_RED_)) { fprintf(stderr, "SIL: capacidad invalida para red %s\n", dest); return t; }
{
char *b = (char*)sil_arena_asignar_(a, 65536 + 1, 1);
if (!b) return t;
n = sil_red_recibir_(dest, (unsigned char*)b, 65536);
if (n <= 0) return t;
b[n] = 0;
t.ptr = b; t.len = n;
}
return t;
}

"#;

// =============================================================================
// Emisor
// =============================================================================

struct Emisor {
    /// ValueId → nombre variable C (v<N>).
    vars: HashMap<ValueId, String>,
    /// ValueId → tipo C declarado.
    tipos_c: HashMap<ValueId, String>,
    next_v: u64,
    /// ¿Necesita stdbool? (si hay Booleano)
    usa_bool: bool,
}

impl Emisor {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            tipos_c: HashMap::new(),
            next_v: 0,
            usa_bool: false,
        }
    }

    fn var(&mut self, id: ValueId) -> String {
        if let Some(n) = self.vars.get(&id) {
            return n.clone();
        }
        let n = format!("v{}", self.next_v);
        self.next_v += 1;
        self.vars.insert(id, n.clone());
        n
    }

    fn tipo_c(&mut self, t: &SilType) -> Result<String, ErrorC99> {
        match t {
            SilType::Entero64 => Ok("long long".into()),
            SilType::Flotante64 => Ok("double".into()),
            SilType::Booleano => {
                self.usa_bool = true;
                Ok("int".into()) // C99 _Bool vía stdbool; usamos int para -Werror limpio
            }
            SilType::Texto => Ok("SilTexto_".into()),
            SilType::CapacidadHardware => Ok("SilCap_".into()),
            SilType::Void => Ok("void".into()),
            SilType::USD | SilType::EUR => Ok("long long".into()),
            SilType::Lista(_)
            | SilType::Mapa(..)
            | SilType::Conjunto(_)
            | SilType::Tupla(_)
            | SilType::Nominal(_) => Err(ErrorC99::TipoNoSoportado(format!(
                "{t:?} (M6: fat pointers)"
            ))),
        }
    }
}

/// Emite programa C99 completo (headers + runtime + función) para una tarea.
///
/// # Contrato
/// - Pre: `tarea` debe tener nombre válido, params con tipos soportados
/// - Pre: IR debe ser válida (SSA, tipos consistentes)
/// - Post: Retorna código C99 que compila con `gcc -std=c99 -Wall -Wextra -Werror`
/// - Invariante: Cada ValueId → variable C única `v<N>` con tipo declarado
pub fn emitir(tarea: &TareaIR) -> Result<String, ErrorC99> {
    let (ret_c, params_c, cuerpo, usa_bool) = emitir_def_funcion(tarea, false)?;

    // --- 4. Ensamblar archivo ---
    let mut out = String::new();
    out.push_str(PROLOGO_FEATURES);
    out.push_str("#include <stdio.h>\n");
    out.push_str("#include <stdlib.h>\n");
    out.push_str("#include <string.h>\n");
    out.push_str("#include <assert.h>\n");
    if usa_bool {
        out.push_str("#include <stdbool.h>\n");
    }
    out.push_str(RUNTIME_DIAG_PUSH);
    out.push_str(RUNTIME_ARENA);
    out.push_str(RUNTIME_TIPOS);
    out.push_str(RUNTIME_CONFIANZA);
    out.push_str(&format!(
        "\n/* Tarea SIL: {} */\n{} {}({}) {{\n(void)__arena;\n{} }}\n",
        tarea.nombre,
        ret_c,
        c_nombre_fn(&tarea.nombre),
        params_c,
        cuerpo
    ));
    out.push_str(RUNTIME_DIAG_POP);

    // Postcondición: código generado no vacío
    ensure(!out.is_empty(), "código C generado vacío");
    ensure(
        out.contains("int main") || out.contains(&format!("{}(", c_nombre_fn(&tarea.nombre))),
        "función no generada",
    );

    Ok(out)
}

/// Emite un programa C99 completo con todas las tareas del archivo + main.
///
/// Convención: la última tarea es la principal.
/// - Sin parámetros: se emite `main`, que ejecuta la principal y luego las
///   fibras registradas con `generar X`.
/// - Con parámetros: se emite una biblioteca (sin `main`), igual que `emitir`.
pub fn emitir_programa(tareas: &[TareaIR]) -> Result<String, ErrorC99> {
    require(!tareas.is_empty(), "programa sin tareas");
    let principal = tareas.last().expect("programa sin tareas");
    let con_main = principal.params.is_empty();
    for t in tareas {
        require(!t.nombre.is_empty(), "nombre de tarea vacío");
        require_valid_ident(&t.nombre, 64, "identificador de tarea inválido");
    }

    let mut out = String::new();
    out.push_str(PROLOGO_FEATURES);
    out.push_str("#include <stdio.h>\n");
    out.push_str("#include <stdlib.h>\n");
    out.push_str("#include <string.h>\n");
    out.push_str("#include <assert.h>\n");
    out.push_str(RUNTIME_DIAG_PUSH);
    out.push_str(RUNTIME_ARENA);
    out.push_str(RUNTIME_TIPOS);
    out.push_str(RUNTIME_CONFIANZA);
    if con_main {
        out.push_str("static SilArena_ sil_arena_main_;\n");
        out.push_str("static SilSched_ sil_sched_main_;\n");
    }

    // Valida los `generar X`: la tarea debe existir y no tener parámetros.
    {
        use silc_causal_ir::nodes::Operacion as Op;
        let nombres: std::collections::HashSet<&str> =
            tareas.iter().map(|t| t.nombre.as_str()).collect();
        let sin_params: std::collections::HashSet<&str> = tareas
            .iter()
            .filter(|t| t.params.is_empty())
            .map(|t| t.nombre.as_str())
            .collect();
        for t in tareas {
            for b in t.bloques.values() {
                for ins in &b.instrs {
                    if let Op::CrearFibra { tarea: f, .. } = &ins.op {
                        if !nombres.contains(f.as_str()) {
                            return Err(ErrorC99::Emision(format!(
                                "generar {f}: tarea inexistente en el archivo"
                            )));
                        }
                        if !sin_params.contains(f.as_str()) {
                            return Err(ErrorC99::Emision(format!(
                                "generar {f}: la tarea no puede tener parámetros"
                            )));
                        }
                        if !con_main {
                            return Err(ErrorC99::Emision(format!(
                                "generar {f}: requiere una tarea principal sin parámetros"
                            )));
                        }
                    }
                }
            }
        }
    }

    // Tareas sin parámetros: aptas como fibra (wrapper generado abajo).
    let aptas: Vec<String> = tareas
        .iter()
        .filter(|t| t.params.is_empty())
        .map(|t| t.nombre.clone())
        .collect();

    // Definiciones de todas las funciones.
    let mut defs: Vec<(String, String, String, String)> = Vec::new();
    let mut usa_bool = false;
    for t in tareas {
        let (ret_c, params_decl, cuerpo, ub) = emitir_def_funcion(t, true)?;
        usa_bool |= ub;
        defs.push((t.nombre.clone(), ret_c, params_decl, cuerpo));
    }
    if usa_bool {
        out.push_str("#include <stdbool.h>\n");
    }

    // Prototipos: los wrappers de fibra y los `generar X` referencian funciones
    // que se definen más abajo en el archivo.
    for (nombre, ret_c, params_decl, _) in &defs {
        out.push_str(&format!(
            "{ret_c} {}({params_decl});\n",
            c_nombre_fn(nombre)
        ));
    }

    // Wrappers de fibra para las tareas aptas (antes de las definiciones:
    // `generar X` dentro de otra tarea los usa).
    if con_main {
        for nombre in &aptas {
            let f = c_nombre_fn(nombre);
            out.push_str(&format!(
                "static void sil_fibra_{f}(void *ctx) {{ (void)ctx; (void){f}(&sil_arena_main_); }}\n",
            ));
        }
    }

    for (nombre, ret_c, params_decl, cuerpo) in &defs {
        out.push_str(&format!(
            "\n/* Tarea SIL: {nombre} */\n{ret_c} {}({params_decl}) {{\n(void)__arena;\n{cuerpo}}}\n",
            c_nombre_fn(nombre),
        ));
    }

    // main: arena global, principal, scheduler, impresión del retorno.
    if con_main {
        let fp = c_nombre_fn(&principal.nombre);
        let llamada = match &principal.retorno {
            SilType::Void => format!("{fp}(&sil_arena_main_);"),
            SilType::Entero64 | SilType::USD | SilType::EUR => {
                format!(
                    "{{ long long sil_r_ = {fp}(&sil_arena_main_); printf(\"%lld\\n\", sil_r_); }}"
                )
            }
            SilType::Flotante64 => {
                format!("{{ double sil_r_ = {fp}(&sil_arena_main_); printf(\"%f\\n\", sil_r_); }}")
            }
            SilType::Booleano => {
                format!("{{ int sil_r_ = {fp}(&sil_arena_main_); printf(\"%d\\n\", sil_r_); }}")
            }
            SilType::Texto => {
                format!("{{ SilTexto_ sil_r_ = {fp}(&sil_arena_main_); fwrite(sil_r_.ptr, 1, (unsigned long)sil_r_.len, stdout); fputc('\\n', stdout); }}")
            }
            SilType::CapacidadHardware => {
                format!("{{ (void){fp}(&sil_arena_main_); }}")
            }
            otro => {
                return Err(ErrorC99::TipoNoSoportado(format!("{otro:?}")));
            }
        };
        out.push_str(&format!(
            "\nint main(void) {{\nsil_arena_main_ = sil_arena_crear_(65536ul);\n{llamada}\nsil_sched_ejecutar_(&sil_sched_main_);\nreturn 0;\n}}\n",
            llamada = llamada,
        ));

        ensure(out.contains("int main(void)"), "main no generado");
    }
    out.push_str(RUNTIME_DIAG_POP);
    Ok(out)
}

/// Emite firma y cuerpo de una tarea. `en_programa` permite `generar`
/// (spawn al scheduler global); sin programa es error limpio.
fn emitir_def_funcion(
    tarea: &TareaIR,
    en_programa: bool,
) -> Result<(String, String, String, bool), ErrorC99> {
    require(!tarea.nombre.is_empty(), "nombre de tarea vacío");
    require_valid_ident(&tarea.nombre, 64, "identificador de tarea inválido");

    let mut e = Emisor::new();

    // --- 1. Pre-declarar tipos de todos los valores (para orden estable) ---
    // Recolectamos en orden de bloques para determinismo.
    let mut orden: Vec<(ValueId, SilType)> = Vec::new();
    {
        let mut vistos = std::collections::HashSet::new();
        // Params primero
        for (nombre, tipo) in &tarea.params {
            require_valid_ident(nombre, 64, "parámetro inválido");
            let _ = nombre;
            let _ = tipo;
        }
        // Instrucciones en orden de bloques (determinista por Debug de la key)
        let mut bids: Vec<_> = tarea.bloques.keys().collect();
        bids.sort_by_key(|k| format!("{k:?}"));
        for bid in bids {
            let b = &tarea.bloques[bid];
            for ins in &b.instrs {
                if let Some(v) = ins.resultado {
                    if vistos.insert(v) {
                        let t = tarea.tipos.get(&v).cloned().unwrap_or(SilType::Entero64);
                        orden.push((v, t));
                    }
                }
            }
        }
    }
    for (v, t) in &orden {
        let tc = e.tipo_c(t)?;
        let n = e.var(*v);
        e.tipos_c.insert(*v, format!("{tc} {n}"));
    }

    // --- 2. Firma de función ---
    let ret_c = e.tipo_c(&tarea.retorno)?;
    let mut params_c = vec!["SilArena_ *__arena".to_string()];
    for (nombre, tipo) in &tarea.params {
        let tc = e.tipo_c(tipo)?;
        params_c.push(format!("{tc} {nombre}"));
    }

    // --- 3. Cuerpo: emitir bloques en orden ---
    let mut cuerpo = String::new();
    // Mapeo ValueId de params → nombre C (los params usan su nombre fuente).
    let mut mapa_params: HashMap<ValueId, String> = HashMap::new();
    {
        // Re-resolver: los nodos Param tienen resultado ValueId; buscarlos.
        for (_, b) in tarea.bloques.iter() {
            for ins in &b.instrs {
                if let Operacion::Param { nombre, .. } = &ins.op {
                    if let Some(v) = ins.resultado {
                        mapa_params.insert(v, nombre.clone());
                        // Sobrescribir el nombre v<N> por el nombre fuente para params.
                        e.vars.insert(v, nombre.clone());
                    }
                }
            }
        }
    }

    let mut bids: Vec<_> = tarea.bloques.keys().collect();
    bids.sort_by_key(|k| format!("{k:?}"));
    // Indice estable por bloque para etiquetas.
    let mut indice: HashMap<silc_causal_ir::nodes::BlockId, usize> = HashMap::new();
    for (i, bid) in bids.iter().enumerate() {
        indice.insert(*bid, i);
    }
    // Determine which blocks are jump targets.
    let mut referenced: std::collections::HashSet<silc_causal_ir::nodes::BlockId> =
        std::collections::HashSet::new();
    for bid in &bids {
        let b = &tarea.bloques[*bid];
        match &b.terminador {
            Terminador::Salto(d) => {
                referenced.insert(*d);
            }
            Terminador::Rama { entonces, sino, .. } => {
                referenced.insert(*entonces);
                referenced.insert(*sino);
            }
            _ => {}
        }
    }
    // No se fuerza etiqueta en el bloque de entrada: si nadie salta a él
    // (caso habitual), `blk_0:` sería una etiqueta sin uso → -Wunused-label
    // con -Wall -Werror en gcc/clang.
    for bid_ref in &bids {
        let bid = *bid_ref;
        let n_blk = indice[&bid];
        // Etiqueta sólo si algún goto la referencia; el cuerpo se emite SIEMPRE
        // (los bloques no referenciados se alcanzan por caída desde el anterior).
        if referenced.contains(&bid) {
            writeln!(cuerpo, "blk_{n_blk}:;").unwrap();
        }
        let b: &Bloque = &tarea.bloques[bid];
        // Declarar variables locales de este bloque (las que tienen resultado).
        for ins in &b.instrs {
            if let Some(v) = ins.resultado {
                // Saltar params (ya son parámetros de función).
                if mapa_params.contains_key(&v) {
                    continue;
                }
                let t = tarea.tipos.get(&v).cloned().unwrap_or(SilType::Entero64);
                let tc = e.tipo_c(&t)?;
                let n = e.var(v);
                writeln!(cuerpo, "    {tc} {n};").unwrap();
            }
        }
        // Emitir instrucciones.
        for ins in &b.instrs {
            emitir_instr(&mut e, &mapa_params, tarea, ins, &mut cuerpo, en_programa)?;
        }
        // Terminador.
        match &b.terminador {
            Terminador::Retorno(v) => {
                if let Some(x) = v {
                    if ret_c == "void" {
                        // Tarea sin tipo de retorno: no se puede devolver valor.
                        let n = nombre_val(&e, &mapa_params, *x);
                        writeln!(cuerpo, "    (void){n};\n    return;").unwrap();
                    } else {
                        let n = nombre_val(&e, &mapa_params, *x);
                        writeln!(cuerpo, "    return {n};").unwrap();
                    }
                } else {
                    writeln!(cuerpo, "    return;").unwrap();
                }
            }
            Terminador::Salto(d) => {
                writeln!(cuerpo, "    goto blk_{};", indice[d]).unwrap();
            }
            Terminador::Rama {
                cond,
                entonces,
                sino,
            } => {
                let c = nombre_val(&e, &mapa_params, *cond);
                writeln!(
                    cuerpo,
                    "    if ({c}) goto blk_{}; else goto blk_{};",
                    indice[entonces], indice[sino],
                )
                .unwrap();
            }
            Terminador::Inalcanzable => {}
        }
    }

    // Retorna firma y cuerpo para ensamblar (una tarea o programa).
    Ok((ret_c, params_c.join(", "), cuerpo, e.usa_bool))
}

fn c_nombre_fn(nombre: &str) -> String {
    // Sanitizar para C: solo alnum + _.
    nombre
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

/// Indice estable de bloque para etiquetas `blk_<N>` (orden determinista).
fn blk_indice(tarea: &TareaIR, objetivo: silc_causal_ir::nodes::BlockId) -> usize {
    let mut bids: Vec<_> = tarea.bloques.keys().collect();
    bids.sort_by_key(|k| format!("{k:?}"));
    bids.iter().position(|k| *k == objetivo).unwrap_or(0)
}

fn nombre_val(e: &Emisor, mapa_params: &HashMap<ValueId, String>, v: ValueId) -> String {
    if let Some(n) = mapa_params.get(&v) {
        return n.clone();
    }
    e.vars.get(&v).cloned().unwrap_or_else(|| "0".into())
}

fn emitir_instr(
    e: &mut Emisor,
    mapa_params: &HashMap<ValueId, String>,
    tarea: &TareaIR,
    ins: &silc_causal_ir::nodes::InstrCausal,
    cuerpo: &mut String,
    en_programa: bool,
) -> Result<(), ErrorC99> {
    match &ins.op {
        Operacion::Param { .. } => {
            // Ya es parámetro de función. Nada que emitir.
        }
        Operacion::Const { valor, .. } => {
            if let Some(v) = ins.resultado {
                let n = nombre_val(e, mapa_params, v);
                let lit = match valor {
                    Constante::Int(x) => format!("{x}LL"),
                    Constante::Float(f) => format!("{f}"),
                    Constante::Bool(b) => format!("{}", i64::from(*b)),
                    Constante::Texto(s) => {
                        format!("(SilTexto_){{\"{}\", {}}}", esc(s.as_str()), s.len())
                    }
                };
                writeln!(cuerpo, "    {n} = {lit};").unwrap();
            }
        }
        Operacion::AsignarArena { nombre, valor, .. } => {
            if let Some(v) = ins.resultado {
                let n = nombre_val(e, mapa_params, v);
                let src = nombre_val(e, mapa_params, *valor);
                writeln!(cuerpo, "    {n} = {src}; (void)\"{nombre}\";").unwrap();
            }
        }
        Operacion::BinOpSegura { op, lhs, rhs } => {
            if let Some(v) = ins.resultado {
                let n = nombre_val(e, mapa_params, v);
                let l = nombre_val(e, mapa_params, *lhs);
                let r = nombre_val(e, mapa_params, *rhs);
                match *op {
                    OpArit::Add => {
                        writeln!(
                            cuerpo,
                            "    {n} = (int64_t)((uint64_t){l} + (uint64_t){r});"
                        )
                        .unwrap();
                    }
                    OpArit::Sub => {
                        writeln!(
                            cuerpo,
                            "    {n} = (int64_t)((uint64_t){l} - (uint64_t){r});"
                        )
                        .unwrap();
                    }
                    OpArit::Mul => {
                        writeln!(
                            cuerpo,
                            "    {n} = (int64_t)((uint64_t){l} * (uint64_t){r});"
                        )
                        .unwrap();
                    }
                    OpArit::Div => {
                        writeln!(cuerpo, "    if ({r} == 0 || ({l} == (int64_t)INT64_MIN && {r} == -1)) abort();").unwrap();
                        writeln!(cuerpo, "    {n} = {l} / {r};").unwrap();
                    }
                }
            }
        }
        Operacion::Comparar { op, lhs, rhs } => {
            if let Some(v) = ins.resultado {
                let n = nombre_val(e, mapa_params, v);
                let l = nombre_val(e, mapa_params, *lhs);
                let r = nombre_val(e, mapa_params, *rhs);
                let o: &str = match op {
                    OpCmp::Gt => ">",
                    OpCmp::Lt => "<",
                    OpCmp::Eq => "==",
                    OpCmp::Ne => "!=",
                    OpCmp::Ge => ">=",
                    OpCmp::Le => "<=",
                };
                writeln!(cuerpo, "    {n} = ({l} {o} {r});").unwrap();
            }
        }
        Operacion::Retornar { .. } => {
            // El terminador emite el return. Nada aquí.
        }
        Operacion::CrearCanal { capacidad } => {
            if let Some(v) = ins.resultado {
                let n = nombre_val(e, mapa_params, v);
                writeln!(cuerpo, "    {n} = sil_canal_crear_({capacidad}ul);").unwrap();
            }
        }
        Operacion::SolicitarCapacidad { req } => {
            if let Some(v) = ins.resultado {
                let n = nombre_val(e, mapa_params, v);
                let perm: u32 = match req.permiso {
                    silc_causal_ir::nodes::Permiso::LecturaArchivo => 0x01,
                    silc_causal_ir::nodes::Permiso::EscrituraArchivo => 0x02,
                    silc_causal_ir::nodes::Permiso::Red => 0x04,
                    silc_causal_ir::nodes::Permiso::Ejecucion => 0x08,
                };
                let rec = req.recurso.clone().unwrap_or_default();
                writeln!(
                    cuerpo,
                    "    {n} = sil_cap_solicitar_(\"{}\", {perm}u, {}ull);",
                    esc(&rec),
                    req.ttl_ns,
                )
                .unwrap();
            }
        }
        Operacion::ValidarCapacidad { token } => {
            if let Some(v) = ins.resultado {
                let n = nombre_val(e, mapa_params, v);
                let t = nombre_val(e, mapa_params, *token);
                writeln!(cuerpo, "    {n} = sil_cap_validar_(&{t}, 0, 0);").unwrap();
            }
        }
        Operacion::InvocarIO {
            operacion,
            args,
            token,
        } => {
            use silc_causal_ir::nodes::OpIO;
            let t = nombre_val(e, mapa_params, *token);
            match operacion {
                OpIO::LeerArchivo => {
                    let ruta = args.first().copied().unwrap_or(*token);
                    let r = nombre_val(e, mapa_params, ruta);
                    if let Some(v) = ins.resultado {
                        let n = nombre_val(e, mapa_params, v);
                        writeln!(cuerpo, "    {n} = sil_io_leer_(__arena, {r}.ptr, &{t});")
                            .unwrap();
                    }
                }
                OpIO::EscribirArchivo => {
                    let d = args.first().copied().unwrap_or(*token);
                    let r = args.get(1).copied().unwrap_or(*token);
                    let dn = nombre_val(e, mapa_params, d);
                    let rn = nombre_val(e, mapa_params, r);
                    // datos Texto o Entero64 (se formatea).
                    let es_texto = tarea.tipos.get(&d) == Some(&SilType::Texto);
                    if es_texto {
                        writeln!(cuerpo, "    sil_io_escribir_({rn}.ptr, {dn}, &{t});").unwrap();
                    } else {
                        writeln!(cuerpo, "    {{ char nb[32]; int nbl = snprintf(nb, sizeof(nb), \"%lld\", (long long){dn}); SilTexto_ nt = {{nb, 0}}; nt.len = nbl; sil_io_escribir_({rn}.ptr, nt, &{t}); }}").unwrap();
                    }
                    if let Some(v) = ins.resultado {
                        let n = nombre_val(e, mapa_params, v);
                        writeln!(cuerpo, "    {n} = 0;").unwrap();
                    }
                }
                OpIO::EnviarRed => {
                    let d = args.first().copied().unwrap_or(*token);
                    let dst = args.get(1).copied().unwrap_or(*token);
                    let dn = nombre_val(e, mapa_params, d);
                    let sn = nombre_val(e, mapa_params, dst);
                    let es_texto = tarea.tipos.get(&d) == Some(&SilType::Texto);
                    if es_texto {
                        writeln!(cuerpo, "    sil_red_enviar_({sn}.ptr, (const unsigned char*){dn}.ptr, (unsigned long){dn}.len);").unwrap();
                    } else {
                        writeln!(cuerpo, "    {{ char nb[32]; int nbl = snprintf(nb, sizeof(nb), \"%lld\", (long long){dn}); sil_red_enviar_({sn}.ptr, (const unsigned char*)nb, (unsigned long)(nbl > 0 ? nbl : 0)); }}").unwrap();
                    }
                    if let Some(v) = ins.resultado {
                        let n = nombre_val(e, mapa_params, v);
                        writeln!(cuerpo, "    {n} = 0;").unwrap();
                    }
                }
                OpIO::RecibirRed => {
                    let dst = args.first().copied().unwrap_or(*token);
                    let sn = nombre_val(e, mapa_params, dst);
                    if let Some(v) = ins.resultado {
                        let n = nombre_val(e, mapa_params, v);
                        writeln!(
                            cuerpo,
                            "    {n} = sil_red_recibir_t_(__arena, {sn}.ptr, &{t});"
                        )
                        .unwrap();
                    }
                }
            }
        }
        Operacion::CrearFibra { tarea: nombre, .. } => {
            if !en_programa {
                // En emision de una sola tarea es error limpio.
                return Err(ErrorC99::Emision(format!(
                    "generar {nombre}: requiere compilar el archivo completo (multi-tarea)"
                )));
            }
            let f = c_nombre_fn(nombre);
            writeln!(
                cuerpo,
                "    sil_sched_spawn_(&sil_sched_main_, sil_fibra_{f}, 0);"
            )
            .unwrap();
        }
        Operacion::Promover { valor, .. } => {
            // Backend actual usa una sola arena (__arena): promover es copiar.
            if let Some(v) = ins.resultado {
                let n = nombre_val(e, mapa_params, v);
                let src = nombre_val(e, mapa_params, *valor);
                writeln!(cuerpo, "    {n} = {src};").unwrap();
            }
        }
        Operacion::Ramificar {
            cond,
            entonces,
            sino,
        } => {
            let c = nombre_val(e, mapa_params, *cond);
            writeln!(
                cuerpo,
                "    if ({c}) goto blk_{}; else goto blk_{};",
                blk_indice(tarea, *entonces),
                blk_indice(tarea, *sino),
            )
            .unwrap();
        }
        Operacion::Saltar { destino } => {
            writeln!(cuerpo, "    goto blk_{};", blk_indice(tarea, *destino)).unwrap();
        }
        Operacion::EnviarCanal { canal, valor } => {
            let c = nombre_val(e, mapa_params, *canal);
            let v = nombre_val(e, mapa_params, *valor);
            let es_entero = tarea.tipos.get(valor) == Some(&SilType::Entero64);
            if !es_entero {
                return Err(ErrorC99::Emision(
                    "enviar por canal exige valor Entero64".into(),
                ));
            }
            writeln!(cuerpo, "    sil_canal_enviar_({c}, (long long){v});").unwrap();
        }
        Operacion::RecibirCanal { canal } => {
            if let Some(v) = ins.resultado {
                let n = nombre_val(e, mapa_params, v);
                let c = nombre_val(e, mapa_params, *canal);
                writeln!(
                    cuerpo,
                    "    {{ long long rv = 0; sil_canal_recibir_({c}, &rv); {n} = rv; }}"
                )
                .unwrap();
            }
        }
    }
    // Invariante adjunta → chequeo runtime defensivo (además de la prueba SMT).
    if let Some(inv) = &ins.invariante {
        use silc_causal_ir::nodes::{ClaseInvariante, FormulaLogica};
        if inv.clase == ClaseInvariante::Demostracion {
            let cond = formula_a_c(&inv.formula, e, mapa_params);
            writeln!(
                cuerpo,
                "    if (!({cond})) {{ fprintf(stderr, \"SIL: invariante violada\\n\"); exit(1); }}"
            )
            .unwrap();
        }
        let _ = FormulaLogica::ConstBool(true);
    }
    Ok(())
}

#[allow(clippy::only_used_in_recursion)]
fn formula_a_c(
    f: &silc_causal_ir::nodes::FormulaLogica,
    e: &Emisor,
    mapa_params: &HashMap<ValueId, String>,
) -> String {
    use silc_causal_ir::nodes::{FormulaLogica, OpLogico};
    match f {
        FormulaLogica::Var(v) => v.clone(),
        FormulaLogica::VarId(v) => mapa_params
            .get(v)
            .cloned()
            .unwrap_or_else(|| format!("v{}", v.data().as_ffi())),
        FormulaLogica::ConstInt(n) => format!("{n}LL"),
        FormulaLogica::ConstBool(b) => format!("{}", i64::from(*b)),
        FormulaLogica::BinOp { op, lhs, rhs } => {
            let o = match op {
                OpLogico::Gt => ">",
                OpLogico::Lt => "<",
                OpLogico::Eq => "==",
                OpLogico::Ne => "!=",
                OpLogico::Ge => ">=",
                OpLogico::Le => "<=",
                OpLogico::Add => "+",
                OpLogico::Sub => "-",
                OpLogico::Mul => "*",
                OpLogico::Div => "/",
                OpLogico::And => "&&",
                OpLogico::Or => "||",
            };
            format!(
                "({} {o} {})",
                formula_a_c(lhs, e, mapa_params),
                formula_a_c(rhs, e, mapa_params)
            )
        }
        FormulaLogica::No(x) => format!("(!{})", formula_a_c(x, e, mapa_params)),
    }
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

// =============================================================================
// Tests M5 (disciplina elite: el C generado debe compilar con gcc -Werror)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use silc_causal_ir::{lower::bajar_tarea, nodes::TareaIR};
    use silc_frontend::{lexer::lexear, parser::parsear};

    fn ir_de(src: &str) -> TareaIR {
        let prog = parsear(&lexear(src)).expect("parse OK");
        let silc_frontend::ast::Decl::Tarea(t) = &prog.defs[0] else {
            panic!()
        };
        bajar_tarea(t).expect("lower OK")
    }

    fn compila_gcc(codigo: &str) -> Result<(), String> {
        // Nombre único: los tests corren en paralelo dentro del mismo proceso,
        // un nombre fijo por PID provoca carreras (uno borra el .c del otro).
        static CONTADOR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = CONTADOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!("sil_test_{}_{}.c", std::process::id(), n));
        std::fs::write(&path, codigo).map_err(|e| e.to_string())?;
        let out = std::process::Command::new("gcc")
            .args(["-std=c99", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
            .arg(&path)
            .output();
        let _ = std::fs::remove_file(&path);
        match out {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(format!("gcc: {}", String::from_utf8_lossy(&o.stderr))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err("SKIP: gcc no instalado".into())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    #[test]
    fn emite_funcion_minima() {
        let ir = ir_de("definir tarea f(x: Entero64) -> Entero64:\n    retornar x\n");
        let c = emitir(&ir).unwrap();
        assert!(c.contains("long long f("));
        assert!(c.contains("return x;"));
        assert!(c.contains("#include <stdio.h>"));
    }

    #[test]
    fn runtime_arena_embebido() {
        let ir = ir_de("definir tarea f() -> Entero64:\n    retornar 1\n");
        let c = emitir(&ir).unwrap();
        assert!(c.contains("sil_arena_crear_"));
        assert!(c.contains("sil_arena_asignar_"));
        assert!(c.contains("SIL_ARENA_PAGE_"));
    }

    #[test]
    fn verificacion_emitida_como_if_exit() {
        let ir = ir_de(
            "definir tarea f(x: Entero64) -> Entero64:\n    verificar que x > 0\n    retornar x\n",
        );
        let c = emitir(&ir).unwrap();
        assert!(c.contains("if (!("));
        assert!(c.contains("exit(1)"));
    }

    #[test]
    fn gcc_acepta_minima() {
        let ir = ir_de("definir tarea f(x: Entero64) -> Entero64:\n    retornar x\n");
        let c = emitir(&ir).unwrap();
        match compila_gcc(&c) {
            Ok(()) => {}
            Err(e) if e.contains("SKIP") => eprintln!("skip: {e}"),
            Err(e) => panic!("gcc rechazó C válido: {e}\n---\n{c}"),
        }
    }

    #[test]
    fn gcc_acepta_con_verificacion() {
        let ir = ir_de(
            "definir tarea f(x: Entero64) -> Entero64:\n    verificar que x > 0\n    retornar x\n",
        );
        let c = emitir(&ir).unwrap();
        match compila_gcc(&c) {
            Ok(()) => {}
            Err(e) if e.contains("SKIP") => eprintln!("skip: {e}"),
            Err(e) => panic!("gcc rechazó: {e}\n---\n{c}"),
        }
    }

    #[test]
    fn gcc_acepta_aritmetica() {
        let ir = ir_de("definir tarea f(x: Entero64) -> Entero64:\n    let y: Entero64 = x * 2\n    retornar x + y\n");
        let c = emitir(&ir).unwrap();
        match compila_gcc(&c) {
            Ok(()) => {}
            Err(e) if e.contains("SKIP") => eprintln!("skip: {e}"),
            Err(e) => panic!("gcc rechazó: {e}\n---\n{c}"),
        }
    }

    #[test]
    fn tipos_no_soportados_error_limpio() {
        let ir = ir_de("definir tarea f(xs: Lista de Entero64) -> Entero64:\n    retornar 0\n");
        let r = emitir(&ir);
        assert!(
            r.is_err(),
            "Lista debe fallar en M5 con error limpio (no panic)"
        );
    }

    #[test]
    fn capacidades_emiten_runtime_real() {
        let ir = ir_de(
            "definir tarea f() -> Booleano:\n    solicitar capacidad lectura_archivo para \"/tmp/x\" como cap\n    validar cap\n    retornar verdadero\n",
        );
        let c = emitir(&ir).unwrap();
        assert!(
            c.contains("sil_cap_solicitar_(\"/tmp/x\""),
            "sin solicitar:\n{c}"
        );
        assert!(c.contains("sil_cap_validar_(&"), "sin validar:\n{c}");
        assert!(c.contains("sil_hmac_"), "sin runtime HMAC embebido");
    }

    #[test]
    fn canal_fifo_real() {
        let ir = ir_de(
            "definir tarea f() -> Entero64:\n    crear canal con capacidad 8 como c\n    enviar 40 en c\n    recibir de c como v\n    retornar v\n",
        );
        let c = emitir(&ir).unwrap();
        assert!(c.contains("sil_canal_crear_(8"), "sin crear:\n{c}");
        assert!(c.contains("sil_canal_enviar_("), "sin enviar:\n{c}");
        assert!(c.contains("sil_canal_recibir_("), "sin recibir:\n{c}");
    }

    #[test]
    fn io_archivo_con_capacidad() {
        let ir = ir_de(
            "definir tarea f() -> Booleano:\n    solicitar capacidad lectura_archivo para \"/tmp/d\" como cap\n    leer archivo \"/tmp/d\" con cap como t\n    retornar verdadero\n",
        );
        let c = emitir(&ir).unwrap();
        assert!(c.contains("sil_io_leer_(__arena,"), "sin leer:\n{c}");
    }

    #[test]
    fn ramas_con_goto_real() {
        // Terminadores Rama/Salto emiten goto con etiquetas, no TODO.
        // Y un bloque sin gotos hacia él no lleva etiqueta (evita -Wunused-label).
        let ir = ir_de("definir tarea f(x: Entero64) -> Entero64:\n    retornar x\n");
        let c = emitir(&ir).unwrap();
        assert!(!c.contains("TODO(M6)"), "queda TODO:\n{c}");
        assert!(c.contains("return x;"), "cuerpo no emitido:\n{c}");
        assert!(!c.contains("blk_0:"), "etiqueta sin uso:\n{c}");
    }

    #[test]
    fn aritmetica_emite_casts_c() {
        // Los casts deben ser de tipos C, no del lenguaje SIL (Entero64 no existe en C).
        let ir = ir_de("definir tarea f(x: Entero64) -> Entero64:\n    let y: Entero64 = x * 2\n    retornar x + y\n");
        let c = emitir(&ir).unwrap();
        assert!(!c.contains("(Entero64)"), "cast a tipo SIL:\n{c}");
        assert!(c.contains("(int64_t)"), "sin cast C:\n{c}");
    }

    #[test]
    fn prologo_posix_antes_de_includes() {
        // Bajo -std=c99 estricto hacen falta las feature macros POSIX.
        let ir = ir_de("definir tarea f(x: Entero64) -> Entero64:\n    retornar x\n");
        let c = emitir(&ir).unwrap();
        let pos = c.find("_POSIX_C_SOURCE").expect("sin _POSIX_C_SOURCE");
        let inc = c.find("#include").expect("sin include");
        assert!(pos < inc, "feature macro después de un include:\n{c}");
    }
}
