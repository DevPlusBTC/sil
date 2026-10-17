/* sil_nvm.c - NVM Persistent Arenas unificado (selección de plataforma) */

#include "sil_nvm.h"

#if defined(_WIN32)
#include "nvm_win.c"
#else
#include "nvm_linux.c"
#endif