/* sil_ebpf.c - eBPF unificado (seleccion de plataforma) */

#include "sil_ebpf.h"

#if defined(_WIN32)
/* Windows: sin soporte kernel eBPF. Las funciones reales estan en
   ebpf_linux.c rama #else (retornan SIL_EBPF_NO_SOPORTADO). */
#include "ebpf_linux.c"
#else
#include "ebpf_linux.c"
#endif
