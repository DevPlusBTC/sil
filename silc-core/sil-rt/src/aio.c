/* sil_aio.c - Async I/O unificado (selección de plataforma) */

#include "sil_aio.h"

#if defined(_WIN32)
#include "aio_win.c"
#else
#include "aio_linux.c"
#endif