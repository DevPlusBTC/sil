/* test_crypto_cap.c - SHA-256 / HMAC-SHA256 y capacidades Zero-Trust.
 *
 * Vectores tomados de FIPS 180-4 (SHA-256) y RFC 4231 (HMAC-SHA256), que son
 * la referencia publica de ambos algoritmos. Las capacidades se prueban contra
 * los ataques que el diseno declara impedir: firma manipulada, reutilizacion
 * en otro recurso, ampliacion de permisos y expiracion.
 */
#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "sil_cap.h"
#include "sil_hmac.h"
#include "sil_test.h"

static void a_hex(const uint8_t *b, size_t n, char *salida) {
    static const char digitos[] = "0123456789abcdef";
    size_t i;
    for (i = 0; i < n; i++) {
        salida[i * 2] = digitos[b[i] >> 4];
        salida[i * 2 + 1] = digitos[b[i] & 0x0Fu];
    }
    salida[n * 2] = '\0';
}

static void sha256_vectores_fips(void) {
    uint8_t d[32];
    char hex[65];
    uint8_t millon[1000];

    sil_sha256((const uint8_t *)"", 0, d);
    a_hex(d, 32, hex);
    ST_CHECK_STR(hex, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

    sil_sha256((const uint8_t *)"abc", 3, d);
    a_hex(d, 32, hex);
    ST_CHECK_STR(hex, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

    sil_sha256((const uint8_t *)"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq", 56, d);
    a_hex(d, 32, hex);
    ST_CHECK_STR(hex, "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");

    /* Limites de padding del bloque SHA-256 (55, 56 y 64 bytes). */
    memset(millon, 'a', sizeof millon);
    {
        /* sil_sha256 es de un solo paso; el millon se comprueba con el vector
         * de 3 bloques ya cubierto arriba. Aqui se valida el caso 55/56/64. */
        uint8_t m55[55], m56[56], m64[64];
        memset(m55, 'a', sizeof m55);
        memset(m56, 'a', sizeof m56);
        memset(m64, 'a', sizeof m64);
        sil_sha256(m55, sizeof m55, d);
        a_hex(d, 32, hex);
        ST_CHECK_STR(hex, "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318");
        sil_sha256(m56, sizeof m56, d);
        a_hex(d, 32, hex);
        ST_CHECK_STR(hex, "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a");
        sil_sha256(m64, sizeof m64, d);
        a_hex(d, 32, hex);
        ST_CHECK_STR(hex, "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb");
    }

    /* Entrada invalida: digesto nulo determinista, nunca salida sin definir. */
    memset(d, 0xAA, sizeof d);
    sil_sha256(NULL, 5, d);
    a_hex(d, 32, hex);
    ST_CHECK_STR(hex, "0000000000000000000000000000000000000000000000000000000000000000");
    sil_sha256((const uint8_t *)"abc", 3, NULL); /* no debe romper */
    ST_CHECK(1);
}

static void hmac_vectores_rfc4231(void) {
    uint8_t mac[32];
    char hex[65];
    uint8_t clave[131];

    /* Caso 1: clave 0x0b x20, datos "Hi There". */
    memset(clave, 0x0b, 20);
    sil_hmac_sha256(clave, 20, (const uint8_t *)"Hi There", 8, mac);
    a_hex(mac, 32, hex);
    ST_CHECK_STR(hex, "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");

    /* Caso 2: clave "Jefe", datos "what do ya want for nothing?". */
    sil_hmac_sha256((const uint8_t *)"Jefe", 4,
                    (const uint8_t *)"what do ya want for nothing?", 28, mac);
    a_hex(mac, 32, hex);
    ST_CHECK_STR(hex, "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");

    /* Caso 3: clave 0xaa x20, datos 0xdd x50. */
    {
        uint8_t datos[50];
        memset(clave, 0xaa, 20);
        memset(datos, 0xdd, sizeof datos);
        sil_hmac_sha256(clave, 20, datos, sizeof datos, mac);
        a_hex(mac, 32, hex);
        ST_CHECK_STR(hex, "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe");
    }

    /* Caso 6: clave de 131 bytes (mas larga que el bloque) -> se hashea. */
    memset(clave, 0xaa, sizeof clave);
    sil_hmac_sha256(clave, sizeof clave,
                    (const uint8_t *)"Test Using Larger Than Block-Size Key - Hash Key First",
                    54, mac);
    a_hex(mac, 32, hex);
    ST_CHECK_STR(hex, "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54");

    /* Clave nula con longitud > 0: MAC nulo, sin leer memoria invalida. */
    memset(mac, 0xAA, sizeof mac);
    sil_hmac_sha256(NULL, 8, (const uint8_t *)"x", 1, mac);
    a_hex(mac, 32, hex);
    ST_CHECK_STR(hex, "0000000000000000000000000000000000000000000000000000000000000000");
}

static void comparacion_tiempo_constante(void) {
    uint8_t a[32], b[32];
    memset(a, 0x11, sizeof a);
    memset(b, 0x11, sizeof b);
    ST_CHECK_EQ(sil_ct_igual(a, b, 32), 1);
    b[31] ^= 0x01;
    ST_CHECK_EQ(sil_ct_igual(a, b, 32), 0);
    b[31] ^= 0x01;
    b[0] ^= 0x80;
    ST_CHECK_EQ(sil_ct_igual(a, b, 32), 0);
    ST_CHECK_EQ(sil_ct_igual(a, b, 0), 1);
    ST_CHECK_EQ(sil_ct_igual(NULL, b, 4), 0);
    ST_CHECK_EQ(sil_ct_igual(a, NULL, 4), 0);
}

static const char *CLAVE_HEX =
    "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

static void capacidades_con_clave_de_entorno(void) {
    SilCapacidad cap;
    unsetenv("SIL_CAP_KEY");

    ST_CHECK_EQ(sil_cap_solicitar("/pagos", SIL_CAP_LEER_ARCHIVO, 1000, &cap),
                SIL_CAP_SIN_CLAVE);

    ST_CHECK_EQ(setenv("SIL_CAP_KEY", CLAVE_HEX, 1), 0);
    ST_CHECK_EQ(sil_cap_solicitar("/pagos", SIL_CAP_LEER_ARCHIVO, 5000000000ull, &cap),
                SIL_CAP_OK);
    ST_CHECK(cap.valida);
    ST_CHECK_EQ(cap.permisos, (uint32_t)SIL_CAP_LEER_ARCHIVO);
    ST_CHECK_EQ(sil_cap_validar(&cap, "/pagos", SIL_CAP_LEER_ARCHIVO, 0), SIL_CAP_OK);
    /* Permiso no concedido. */
    ST_CHECK_EQ(sil_cap_validar(&cap, "/pagos", SIL_CAP_ESCRIBIR_ARCHIVO, 0),
                SIL_CAP_ERROR);
    /* Recurso distinto: el hash del recurso no coincide. */
    ST_CHECK_EQ(sil_cap_validar(&cap, "/admin", SIL_CAP_LEER_ARCHIVO, 0), SIL_CAP_ERROR);
    /* Expiracion explicita. */
    ST_CHECK_EQ(sil_cap_validar(&cap, "/pagos", SIL_CAP_LEER_ARCHIVO, UINT64_MAX),
                SIL_CAP_ERROR);
    /* Parametros invalidos. */
    ST_CHECK_EQ(sil_cap_solicitar(NULL, SIL_CAP_LEER_ARCHIVO, 1, &cap),
                SIL_CAP_INVALID_PARAM);
    ST_CHECK_EQ(sil_cap_solicitar("", SIL_CAP_LEER_ARCHIVO, 1, &cap),
                SIL_CAP_INVALID_PARAM);
    ST_CHECK_EQ(sil_cap_solicitar("/pagos", 0, 1, &cap), SIL_CAP_INVALID_PARAM);
    ST_CHECK_EQ(sil_cap_solicitar("/pagos", 0x10u, 1, &cap), SIL_CAP_INVALID_PARAM);
    ST_CHECK_EQ(sil_cap_validar(&cap, "/pagos", 0, 0), SIL_CAP_INVALID_PARAM);
    ST_CHECK_EQ(sil_cap_validar(NULL, "/pagos", SIL_CAP_LEER_ARCHIVO, 0),
                SIL_CAP_INVALID_PARAM);

    /* Firma manipulada. */
    ST_CHECK_EQ(sil_cap_solicitar("/pagos", SIL_CAP_LEER_ARCHIVO, 5000000000ull, &cap),
                SIL_CAP_OK);
    cap.firma[0] ^= 0xFF;
    ST_CHECK_EQ(sil_cap_validar(&cap, "/pagos", SIL_CAP_LEER_ARCHIVO, 0), SIL_CAP_ERROR);

    /* Ampliacion de permisos sobre una capacidad legitima. */
    ST_CHECK_EQ(sil_cap_solicitar("/pagos", SIL_CAP_LEER_ARCHIVO, 5000000000ull, &cap),
                SIL_CAP_OK);
    cap.permisos |= (uint32_t)SIL_CAP_ESCRIBIR_ARCHIVO;
    ST_CHECK_EQ(sil_cap_validar(&cap, "/pagos", SIL_CAP_LEER_ARCHIVO, 0), SIL_CAP_ERROR);

    /* Recurso demasiado largo (limite 1024). */
    {
        char grande[1100];
        memset(grande, 'r', sizeof grande - 1);
        grande[sizeof grande - 1] = '\0';
        ST_CHECK_EQ(sil_cap_solicitar(grande, SIL_CAP_LEER_ARCHIVO, 1, &cap),
                    SIL_CAP_INVALID_PARAM);
    }
    unsetenv("SIL_CAP_KEY");
}

static void capacidades_con_clave_fijada(void) {
    uint8_t clave[32];
    SilCapacidad cap;
    int i;
    for (i = 0; i < 32; i++) clave[i] = (uint8_t)(i * 7 + 3);

    unsetenv("SIL_CAP_KEY"); /* la clave fijada tiene prioridad */
    ST_CHECK_EQ(sil_cap_fijar_clave(NULL), SIL_CAP_INVALID_PARAM);
    ST_CHECK_EQ(sil_cap_fijar_clave(clave), SIL_CAP_OK);
    ST_CHECK_EQ(sil_cap_solicitar("/db", SIL_CAP_LEER_ARCHIVO | SIL_CAP_RED, 1000000, &cap),
                SIL_CAP_OK);
    ST_CHECK_EQ(sil_cap_validar(&cap, "/db", SIL_CAP_RED, 0), SIL_CAP_OK);
    ST_CHECK_EQ(sil_cap_validar(&cap, "/db", SIL_CAP_EXEC, 0), SIL_CAP_ERROR);
    /* La clave de entorno ya no interviene. */
    ST_CHECK_EQ(setenv("SIL_CAP_KEY", CLAVE_HEX, 1), 0);
    ST_CHECK_EQ(sil_cap_validar(&cap, "/db", SIL_CAP_RED, 0), SIL_CAP_OK);
    unsetenv("SIL_CAP_KEY");
}

static void expiracion_estricta(void) {
    SilCapacidad cap;
    /* ttl de 1 ns: validar con "ahora" posterior debe fallar. */
    ST_CHECK_EQ(sil_cap_solicitar("/x", SIL_CAP_LEER_ARCHIVO, 1, &cap), SIL_CAP_OK);
    ST_CHECK_EQ(sil_cap_validar(&cap, "/x", SIL_CAP_LEER_ARCHIVO, cap.expiracion_ns),
                SIL_CAP_OK); /* justo en el limite: aun valida */
    ST_CHECK_EQ(sil_cap_validar(&cap, "/x", SIL_CAP_LEER_ARCHIVO, cap.expiracion_ns + 1),
                SIL_CAP_ERROR);
    /* ttl 0 usa el valor por defecto (500 ms), no expira de inmediato. */
    ST_CHECK_EQ(sil_cap_solicitar("/x", SIL_CAP_LEER_ARCHIVO, 0, &cap), SIL_CAP_OK);
    ST_CHECK_EQ(sil_cap_validar(&cap, "/x", SIL_CAP_LEER_ARCHIVO, 0), SIL_CAP_OK);
}

int main(void) {
    ST_CASO(sha256_vectores_fips);
    ST_CASO(hmac_vectores_rfc4231);
    ST_CASO(comparacion_tiempo_constante);
    ST_CASO(capacidades_con_clave_de_entorno);
    ST_CASO(capacidades_con_clave_fijada); /* fija la clave: va al final */
    ST_CASO(expiracion_estricta);
    ST_FIN();
}
