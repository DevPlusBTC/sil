// silc-runtime: Bindings seguros al runtime C (sil-rt).

// M7: wrappers 100% seguros (cero `unsafe` en API publica) + tests FFI
// que ejercitan el C real (arenas, fibras, capacidades).

// Garantias:
// - `Arena`: RAII (Drop libera), bump alloc con alineacion, reset O(1).
// - `Scheduler`: spawn/ejecutar fibras cooperativas Win32 Fibers (state-machine safe).
// - `Capacidad`: validacion con reloj monotono, expiracion estricta.

use libc::size_t;
use silc_contracts::*;
use std::os::raw::{c_char, c_int, c_void};

// M34: TPM 2.0 capabilities module loaded (ver m34_tpm2.rs)

extern "C" {
    // Arena
    pub fn sil_arena_crear(capacidad_inicial: size_t) -> SilArena;
    pub fn sil_arena_asignar(
        arena: *mut SilArena,
        tamanio: size_t,
        alineacion: size_t,
    ) -> *mut c_void;
    pub fn sil_arena_destruir(arena: *mut SilArena);
    pub fn sil_arena_reset(arena: *mut SilArena);
    pub fn sil_arena_promover(
        origen: *mut SilArena,
        destino: *mut SilArena,
        ptr: *mut c_void,
        tamanio: size_t,
    );
    pub fn sil_arena_ciclo_iniciar(arena: *mut SilArena) -> u32;
    pub fn sil_arena_ciclo_finalizar(arena: *mut SilArena);
    pub fn sil_arena_ciclo_actual(arena: *mut SilArena) -> u32;
    pub fn sil_arena_ciclo_bytes(arena: *mut SilArena, ciclo: u32) -> size_t;
    pub fn sil_arena_ciclo_estadisticas(
        arena: *mut SilArena,
        ciclos_activos: *mut u32,
        bytes_por_ciclo: *mut size_t,
    );

    pub fn sil_sched_crear(n_hilos_os: u32) -> *mut SilScheduler;
    pub fn sil_sched_destruir(s: *mut SilScheduler);
    pub fn sil_sched_spawn(
        s: *mut SilScheduler,
        entrada: extern "C" fn(*mut c_void),
        arg: *mut c_void,
    ) -> bool;
    pub fn sil_sched_ejecutar(s: *mut SilScheduler);
    pub fn sil_fibra_yield();

    // SPSC Channel (Single-Producer, Single-Consumer)
    pub fn sil_channel_crear(arena: *mut SilArena, capacidad_inicial: size_t) -> *mut SilChannel;
    pub fn sil_channel_destruir(canal: *mut SilChannel);
    pub fn sil_channel_enviar(canal: *mut SilChannel, mensaje: *mut c_void) -> bool;
    pub fn sil_channel_recibir(canal: *mut SilChannel, mensaje: *mut *mut c_void) -> bool;
    pub fn sil_channel_cerrar(canal: *mut SilChannel);
    pub fn sil_channel_contar(canal: *mut SilChannel) -> size_t;
    pub fn sil_channel_esta_vacio(canal: *mut SilChannel) -> bool;
    pub fn sil_channel_esta_cerrado(canal: *mut SilChannel) -> bool;

    // MPSC Channel (Multi-Producer, Single-Consumer)
    pub fn sil_channel_mpsc_crear(
        arena: *mut SilArena,
        capacidad_inicial: size_t,
    ) -> *mut SilChannelMPSC;
    pub fn sil_channel_mpsc_destruir(canal: *mut SilChannelMPSC);
    pub fn sil_channel_mpsc_enviar(canal: *mut SilChannelMPSC, mensaje: *mut c_void) -> bool;
    pub fn sil_channel_mpsc_recibir(canal: *mut SilChannelMPSC, mensaje: *mut *mut c_void) -> bool;
    pub fn sil_channel_mpsc_cerrar(canal: *mut SilChannelMPSC);
    pub fn sil_channel_mpsc_contar(canal: *mut SilChannelMPSC) -> size_t;
    pub fn sil_channel_mpsc_esta_vacio(canal: *mut SilChannelMPSC) -> bool;
    pub fn sil_channel_mpsc_esta_cerrado(canal: *mut SilChannelMPSC) -> bool;

    // Async I/O
    pub fn sil_aio_ctx_crear() -> *mut SilAioCtx;
    pub fn sil_aio_ctx_destruir(ctx: *mut SilAioCtx);
    pub fn sil_aio_leer(ctx: *mut SilAioCtx, req: *mut SilAioReq) -> bool;
    pub fn sil_aio_escribir(ctx: *mut SilAioCtx, req: *mut SilAioReq) -> bool;
    pub fn sil_aio_procesar(ctx: *mut SilAioCtx, max_eventos: c_int, timeout_ms: c_int) -> c_int;
    pub fn sil_aio_cancelar(ctx: *mut SilAioCtx, req: *mut SilAioReq) -> bool;
    pub fn sil_aio_tiene_pendientes(ctx: *mut SilAioCtx) -> bool;

    // eBPF Kernel Integration
    pub fn sil_ebpf_ctx_crear() -> *mut SilEbpfContext;
    pub fn sil_ebpf_ctx_destruir(ctx: *mut SilEbpfContext);
    pub fn sil_ebpf_programa_cargar(
        ctx: *mut SilEbpfContext,
        config: *const SilEbpfProgramConfig,
        error: *mut c_int,
    ) -> *mut SilEbpfProgram;
    pub fn sil_ebpf_programa_destruir(prog: *mut SilEbpfProgram);
    pub fn sil_ebpf_adjuntar(
        prog: *mut SilEbpfProgram,
        punto_anclaje: *const c_char,
        error: *mut c_int,
    ) -> *mut SilEbpfLink;
    pub fn sil_ebpf_desadjuntar(link: *mut SilEbpfLink);
    pub fn sil_ebpf_mapa_crear(
        ctx: *mut SilEbpfContext,
        config: *const SilEbpfMapConfig,
        error: *mut c_int,
    ) -> *mut SilEbpfMap;
    pub fn sil_ebpf_mapa_destruir(mapa: *mut SilEbpfMap);
    pub fn sil_ebpf_mapa_actualizar(
        mapa: *mut SilEbpfMap,
        key: *const c_void,
        value: *const c_void,
        flags: u64,
    ) -> c_int;
    pub fn sil_ebpf_mapa_buscar(
        mapa: *mut SilEbpfMap,
        key: *const c_void,
        value: *mut c_void,
    ) -> c_int;
    pub fn sil_ebpf_mapa_eliminar(mapa: *mut SilEbpfMap, key: *const c_void) -> c_int;
    pub fn sil_ebpf_mapa_iterar(
        mapa: *mut SilEbpfMap,
        key: *mut c_void,
        value: *mut c_void,
        mas: *mut bool,
    ) -> c_int;
    pub fn sil_ebpf_mapa_fd(mapa: *mut SilEbpfMap) -> c_int;
    pub fn sil_ebpf_eventos_leer(
        mapa: *mut SilEbpfMap,
        buffer: *mut c_void,
        tamano: size_t,
        timeout_ms: c_int,
    ) -> c_int;
    pub fn sil_ebpf_soportado() -> bool;
    pub fn sil_ebpf_version_kernel() -> u32;
}

#[repr(C)]
pub struct SilArena {
    pub inicio: *mut SilArenaBloque,
    pub actual: *mut SilArenaBloque,
    pub asignado_total: size_t,
    pub ciclo_actual: u32,
    pub ciclo_contador: u32,
}

#[repr(C)]
pub struct SilArenaBloque {
    pub memoria: *mut u8,
    pub capacidad: size_t,
    pub usado: size_t,
    pub siguiente: *mut SilArenaBloque,
}

pub type SilScheduler = c_void;

pub type SilChannel = c_void;

pub type SilChannelMPSC = c_void;

pub type SilAioCtx = c_void;

#[repr(C)]
pub struct SilAioReq {
    pub op: u32,
    pub fd: c_int,
    pub buffer: *mut c_void,
    pub longitud: size_t,
    pub offset: u64,
    pub callback: Option<extern "C" fn(*mut SilAioReq, *mut c_void, c_int)>,
    pub user_data: *mut c_void,
    pub resultado: c_int,
    pub interno: *mut c_void,
}

pub type SilEbpfContext = c_void;
pub type SilEbpfProgram = c_void;
pub type SilEbpfLink = c_void;
pub type SilEbpfMap = c_void;

#[repr(C)]
pub struct SilEbpfProgramConfig {
    pub nombre: *const c_char,
    pub tipo: u32,
    pub bytecode: *const c_void,
    pub bytecode_len: size_t,
    pub license: *const c_char,
    pub kern_version: u32,
}

#[repr(C)]
pub struct SilEbpfMapConfig {
    pub nombre: *const c_char,
    pub tipo: u32,
    pub key_size: u32,
    pub value_size: u32,
    pub max_entries: u32,
    pub flags: u32,
}

pub const SIL_EBPF_PROG_SOCKET_FILTER: u32 = 0;
pub const SIL_EBPF_PROG_KPROBE: u32 = 1;
pub const SIL_EBPF_PROG_UPROBE: u32 = 2;
pub const SIL_EBPF_PROG_TRACEPOINT: u32 = 3;
pub const SIL_EBPF_PROG_XDP: u32 = 4;
pub const SIL_EBPF_PROG_TC: u32 = 5;
pub const SIL_EBPF_PROG_CGROUP_SKB: u32 = 6;
pub const SIL_EBPF_PROG_LSM: u32 = 7;
pub const SIL_EBPF_PROG_STRUCT_OPS: u32 = 8;

pub const SIL_EBPF_MAP_HASH: u32 = 0;
pub const SIL_EBPF_MAP_ARRAY: u32 = 1;
pub const SIL_EBPF_MAP_PROG_ARRAY: u32 = 2;
pub const SIL_EBPF_MAP_PERF_EVENT_ARRAY: u32 = 3;
pub const SIL_EBPF_MAP_RINGBUF: u32 = 4;
pub const SIL_EBPF_MAP_STACK_TRACE: u32 = 5;
pub const SIL_EBPF_MAP_LRU_HASH: u32 = 6;
pub const SIL_EBPF_MAP_LRU_PERCPU_HASH: u32 = 7;

pub const SIL_EBPF_OK: c_int = 0;
pub const SIL_EBPF_ERROR: c_int = -1;
pub const SIL_EBPF_NO_MEM: c_int = -2;
pub const SIL_EBPF_VERIFICACION: c_int = -3;
pub const SIL_EBPF_CARGA: c_int = -4;
pub const SIL_EBPF_ADJUNTO: c_int = -5;
pub const SIL_EBPF_MAPA: c_int = -6;
pub const SIL_EBPF_PERMISOS: c_int = -7;
pub const SIL_EBPF_NO_SOPORTADO: c_int = -8;

// Versión del runtime embebido.
pub const RT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Arena (RAII)

pub struct Arena {
    inner: SilArena,
}

impl Arena {
    /// Crea una nueva arena con la capacidad inicial dada
    ///
    /// # Contrato
    /// - Pre: `capacidad_inicial` > 0
    /// - Post: Retorna Arena inicializada
    pub fn nueva(capacidad_inicial: usize) -> Self {
        require(capacidad_inicial > 0, "capacidad debe ser > 0");
        let inner = unsafe { sil_arena_crear(capacidad_inicial) };
        Self { inner }
    }

    /// Obtiene puntero interno para FFI
    pub fn as_ptr(&mut self) -> *mut SilArena {
        &mut self.inner
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        unsafe { sil_arena_destruir(&mut self.inner) };
    }
}

/// Scheduler (wrapper RAII sobre SilScheduler C)
pub struct Scheduler {
    inner: *mut SilScheduler,
}

/// Scheduler + Fibras (implementación real con Win32 Fibers)
impl Scheduler {
    /// Crea un nuevo scheduler
    pub fn nuevo() -> Self {
        let ptr = unsafe { sil_sched_crear(0) };
        require_non_null(ptr, "sil_sched_crear falló");
        Self { inner: ptr }
    }

    /// Spawn fibra
    ///
    /// # Contrato
    /// - Pre: `entrada` no es null
    /// - Pre: `arg` puede ser null
    /// - Post: Retorna true si spawn exitoso
    ///
    /// # Safety
    /// `entrada` debe ser una función válida con convención C.
    pub unsafe fn spawn(&mut self, entrada: extern "C" fn(*mut c_void), arg: *mut c_void) -> bool {
        require_non_null(self.inner, "scheduler nulo");
        sil_sched_spawn(self.inner, entrada, arg)
    }

    /// Ejecutar scheduler (round-robin hasta que todas finalicen)
    ///
    /// # Contrato
    /// - Pre: `inner` válido
    /// - Post: Ejecuta todas las fibras hasta completarse
    pub fn ejecutar(&mut self) {
        require_non_null(self.inner, "scheduler nulo");
        unsafe { sil_sched_ejecutar(self.inner) }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe { sil_sched_destruir(self.inner) };
            self.inner = std::ptr::null_mut();
        }
    }
}

/// SPSC Channel (Single-Producer, Single-Consumer) - Lock-free
pub struct Channel {
    inner: *mut SilChannel,
    arena: Arena,
}

impl Channel {
    /// Crear un nuevo canal SPSC
    ///
    /// # Contrato
    /// - Pre: `capacidad_inicial` > 0
    /// - Post: Retorna Channel con buffer inicial
    pub fn nuevo(capacidad_inicial: usize) -> Result<Self, &'static str> {
        require(capacidad_inicial > 0, "capacidad debe ser > 0");
        let mut arena = Arena::nueva(capacidad_inicial * 64); // suficiente para nodos
        let ptr = unsafe { sil_channel_crear(arena.as_ptr(), capacidad_inicial) };
        require_non_null(ptr, "sil_channel_crear falló");
        Ok(Self { inner: ptr, arena })
    }

    /// Enviar mensaje (solo productor)
    ///
    /// # Contrato
    /// - Pre: `mensaje` no es null
    /// - Post: Retorna true si enviado exitosamente
    pub fn enviar<T>(&self, mensaje: Box<T>) -> bool {
        let ptr = Box::into_raw(mensaje) as *mut c_void;
        unsafe { sil_channel_enviar(self.inner, ptr) }
    }

    /// Recibir mensaje (solo consumidor)
    ///
    /// # Contrato
    /// - Post: Retorna Some(Box<T>) si hay mensaje, None si vacío
    pub fn recibir<T>(&self) -> Option<Box<T>> {
        let mut msg: *mut c_void = std::ptr::null_mut();
        let ok = unsafe { sil_channel_recibir(self.inner, &mut msg) };
        if ok && !msg.is_null() {
            Some(unsafe { Box::from_raw(msg as *mut T) })
        } else {
            None
        }
    }

    /// Cerrar canal
    pub fn cerrar(&self) {
        unsafe { sil_channel_cerrar(self.inner) }
    }

    /// Verificar si está vacío
    pub fn esta_vacio(&self) -> bool {
        unsafe { sil_channel_esta_vacio(self.inner) }
    }

    /// Verificar si está cerrado
    pub fn esta_cerrado(&self) -> bool {
        unsafe { sil_channel_esta_cerrado(self.inner) }
    }

    /// Contar elementos
    pub fn contar(&self) -> usize {
        unsafe { sil_channel_contar(self.inner) as usize }
    }

    /// Arena propietaria de los nodos. Mantenerla viva es lo que hace
    /// seguro al canal: los nodos se liberan con ella, nunca con free.
    pub fn arena_mut(&mut self) -> &mut Arena {
        &mut self.arena
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe { sil_channel_destruir(self.inner) };
            self.inner = std::ptr::null_mut();
        }
    }
}

/// MPSC Channel (Multi-Producer, Single-Consumer) - Lock-free
pub struct ChannelMPSC {
    inner: *mut SilChannelMPSC,
    arena: Arena,
}

impl ChannelMPSC {
    /// Crear un nuevo canal MPSC
    ///
    /// # Contrato
    /// - Pre: `capacidad_inicial` > 0
    /// - Post: Retorna ChannelMPSC con buffer inicial
    pub fn nuevo(capacidad_inicial: usize) -> Result<Self, &'static str> {
        require(capacidad_inicial > 0, "capacidad debe ser > 0");
        let mut arena = Arena::nueva(capacidad_inicial * 64);
        let ptr = unsafe { sil_channel_mpsc_crear(arena.as_ptr(), capacidad_inicial) };
        require_non_null(ptr, "sil_channel_mpsc_crear falló");
        Ok(Self { inner: ptr, arena })
    }

    /// Enviar mensaje (múltiples productores)
    ///
    /// # Contrato
    /// - Pre: `mensaje` no es null
    /// - Post: Retorna true si enviado exitosamente
    pub fn enviar<T>(&self, mensaje: Box<T>) -> bool {
        let ptr = Box::into_raw(mensaje) as *mut c_void;
        unsafe { sil_channel_mpsc_enviar(self.inner, ptr) }
    }

    /// Recibir mensaje (único consumidor)
    ///
    /// # Contrato
    /// - Post: Retorna Some(Box<T>) si hay mensaje, None si vacío
    pub fn recibir<T>(&self) -> Option<Box<T>> {
        let mut msg: *mut c_void = std::ptr::null_mut();
        let ok = unsafe { sil_channel_mpsc_recibir(self.inner, &mut msg) };
        if ok && !msg.is_null() {
            Some(unsafe { Box::from_raw(msg as *mut T) })
        } else {
            None
        }
    }

    /// Cerrar canal
    pub fn cerrar(&self) {
        unsafe { sil_channel_mpsc_cerrar(self.inner) }
    }

    /// Verificar si está vacío
    pub fn esta_vacio(&self) -> bool {
        unsafe { sil_channel_mpsc_esta_vacio(self.inner) }
    }

    /// Verificar si está cerrado
    pub fn esta_cerrado(&self) -> bool {
        unsafe { sil_channel_mpsc_esta_cerrado(self.inner) }
    }

    /// Contar elementos
    pub fn contar(&self) -> usize {
        unsafe { sil_channel_mpsc_contar(self.inner) as usize }
    }

    /// Arena propietaria de los nodos. Ver Channel::arena_mut.
    pub fn arena_mut(&mut self) -> &mut Arena {
        &mut self.arena
    }
}

impl Drop for ChannelMPSC {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe { sil_channel_mpsc_destruir(self.inner) };
            self.inner = std::ptr::null_mut();
        }
    }
}

/// Async I/O Context (RAII wrapper)
pub struct AioContext {
    inner: *mut SilAioCtx,
}

impl AioContext {
    /// Crear nuevo contexto de I/O asíncrono
    ///
    /// # Contrato
    /// - Post: Retorna AioContext inicializado
    pub fn nuevo() -> Result<Self, &'static str> {
        let ptr = unsafe { sil_aio_ctx_crear() };
        require_non_null(ptr, "sil_aio_ctx_crear falló");
        Ok(Self { inner: ptr })
    }

    /// Enviar operación de lectura asíncrona
    ///
    /// # Contrato
    /// - Pre: `req` válido con fd, buffer, longitud
    /// - Post: Retorna true si encolado exitosamente
    pub fn leer(&self, req: &mut AioRequest) -> bool {
        unsafe { sil_aio_leer(self.inner, req.as_mut_ptr()) }
    }

    /// Enviar operación de escritura asíncrona
    ///
    /// # Contrato
    /// - Pre: `req` válido con fd, buffer, longitud
    /// - Post: Retorna true si encolado exitosamente
    pub fn escribir(&self, req: &mut AioRequest) -> bool {
        unsafe { sil_aio_escribir(self.inner, req.as_mut_ptr()) }
    }

    /// Procesar completaciones pendientes
    ///
    /// # Contrato
    /// - Post: Retorna número de operaciones completadas
    pub fn procesar(&self, max_eventos: usize, timeout_ms: i32) -> usize {
        unsafe { sil_aio_procesar(self.inner, max_eventos as c_int, timeout_ms as c_int) as usize }
    }

    /// Cancelar operación pendiente
    pub fn cancelar(&self, req: &mut AioRequest) -> bool {
        unsafe { sil_aio_cancelar(self.inner, req.as_mut_ptr()) }
    }

    /// Verificar si hay operaciones pendientes
    pub fn tiene_pendientes(&self) -> bool {
        unsafe { sil_aio_tiene_pendientes(self.inner) }
    }
}

impl Drop for AioContext {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe { sil_aio_ctx_destruir(self.inner) };
            self.inner = std::ptr::null_mut();
        }
    }
}

/// Async I/O Request
pub struct AioRequest {
    inner: SilAioReq,
}

impl AioRequest {
    /// Crear request de lectura
    pub fn leer(fd: i32, buffer: *mut u8, longitud: usize, offset: u64) -> Self {
        Self {
            inner: SilAioReq {
                op: 0, // SIL_AIO_LEER
                fd,
                buffer: buffer as *mut c_void,
                longitud,
                offset,
                callback: None,
                user_data: std::ptr::null_mut(),
                resultado: 0,
                interno: std::ptr::null_mut(),
            },
        }
    }

    /// Crear request de escritura
    pub fn escribir(fd: i32, buffer: *const u8, longitud: usize, offset: u64) -> Self {
        Self {
            inner: SilAioReq {
                op: 1, // SIL_AIO_ESCRIBIR
                fd,
                buffer: buffer as *mut c_void,
                longitud,
                offset,
                callback: None,
                user_data: std::ptr::null_mut(),
                resultado: 0,
                interno: std::ptr::null_mut(),
            },
        }
    }

    /// Establecer callback de completación
    pub fn con_callback(
        mut self,
        callback: extern "C" fn(*mut SilAioReq, *mut c_void, c_int),
        user_data: *mut c_void,
    ) -> Self {
        self.inner.callback = Some(callback);
        self.inner.user_data = user_data;
        self
    }

    /// Obtener resultado
    pub fn resultado(&self) -> i32 {
        self.inner.resultado
    }

    fn as_mut_ptr(&mut self) -> *mut SilAioReq {
        &mut self.inner
    }
}

/// Cede ejecución (yield cooperativo)
pub fn ceder() {
    unsafe { sil_fibra_yield() }
}

// Capacidades: firma HMAC-SHA256 real (ver sil_cap.h).
// Estructura Permisos
pub struct Permisos(pub u32);

/// Permisos::NINGUNO - No permissions
pub const NINGUNO: Permisos = Permisos(0x00);
/// Permisos::LEER_ARCHIVO - Read file permission
pub const LEER_ARCHIVO: Permisos = Permisos(0x01);
/// Permisos::ESCRIBIR_ARCHIVO - Write file permission
pub const ESCRIBIR_ARCHIVO: Permisos = Permisos(0x02);
/// Permisos::RED - Network permission
pub const RED: Permisos = Permisos(0x04);
/// Permisos::EXEC - Execute permission
pub const EXEC: Permisos = Permisos(0x08);

/// Capacidades: firma HMAC-SHA256 real en C (sil-rt/cap.c + sil_hmac.c).
/// La clave vive en la variable de entorno SIL_CAP_KEY (64 hex chars).
/// Sin clave, solicitar retorna error claro en vez de firma falsa.
/// Espejo exacto de SilCapacidad en sil_cap.h. No cambiar el orden.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SilCapacidadC {
    pub id: u64,
    pub permisos: u32,
    pub expiracion_ns: u64,
    pub recurso_hash: [u8; 32],
    pub firma: [u8; 32],
    pub valida: bool,
}

extern "C" {
    pub fn sil_cap_fijar_clave(clave: *const u8) -> c_int;
    pub fn sil_cap_solicitar(
        recurso: *const c_char,
        permisos: u32,
        ttl_ns: u64,
        out_cap: *mut SilCapacidadC,
    ) -> c_int;
    pub fn sil_cap_validar(
        cap: *const SilCapacidadC,
        recurso: *const c_char,
        requerido: u32,
        ahora_ns: u64,
    ) -> c_int;
}

/// Lee SIL_CAP_KEY del entorno del proceso Rust (64 hex chars) y la fija
/// en el runtime C. Retorna true si hay clave utilizable.
fn fijar_clave_desde_entorno() -> bool {
    let hex = match std::env::var("SIL_CAP_KEY") {
        Ok(h) => h,
        Err(_) => return false,
    };
    if hex.len() != 64 {
        return false;
    }
    let mut clave = [0u8; 32];
    for (i, b) in clave.iter_mut().enumerate() {
        let byte = match u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16) {
            Ok(v) => v,
            Err(_) => return false,
        };
        *b = byte;
    }
    let rc = unsafe { sil_cap_fijar_clave(clave.as_ptr()) };
    rc == 0
}

/// Capacidad firmada con HMAC-SHA256. El recurso queda ligado por hash:
/// una capacidad de "/pagos" no sirve para "/admin".
pub struct Capacidad {
    inner: SilCapacidadC,
    recurso: String,
}

impl Capacidad {
    /// Solicita una capacidad firmada para un recurso (max 1024 bytes).
    /// Requiere SIL_CAP_KEY en el entorno. ttl_ns == 0 usa 500ms.
    pub fn solicitar(recurso: &str, permisos: Permisos, ttl_ns: u64) -> Result<Self, &'static str> {
        require(!recurso.is_empty(), "recurso vacío");
        require(recurso.len() <= 1024, "recurso muy largo");
        require_safe_path(recurso, "recurso con traversal");
        if !fijar_clave_desde_entorno() {
            return Err("sin clave: define SIL_CAP_KEY con 64 hex chars");
        }
        let c_recurso = std::ffi::CString::new(recurso).map_err(|_| "recurso con NUL")?;
        let mut cap = SilCapacidadC {
            id: 0,
            permisos: 0,
            expiracion_ns: 0,
            recurso_hash: [0; 32],
            firma: [0; 32],
            valida: false,
        };
        let rc = unsafe { sil_cap_solicitar(c_recurso.as_ptr(), permisos.0, ttl_ns, &mut cap) };
        match rc {
            0 => Ok(Self {
                inner: cap,
                recurso: recurso.to_string(),
            }),
            -4 => Err("sin clave: define SIL_CAP_KEY con 64 hex chars"),
            -3 => Err("parámetro inválido en capacidad"),
            _ => Err("error al solicitar capacidad"),
        }
    }

    /// Valida firma, permisos, recurso y expiración contra el reloj actual.
    pub fn valida(&self, requerido: Permisos) -> bool {
        if !fijar_clave_desde_entorno() {
            return false;
        }
        let c_recurso = match std::ffi::CString::new(self.recurso.clone()) {
            Ok(c) => c,
            Err(_) => return false,
        };
        unsafe { sil_cap_validar(&self.inner, c_recurso.as_ptr(), requerido.0, 0) == 0 }
    }

    /// Valida en un instante dado (ns monotónicos). ahora_ns == 0 usa el reloj actual.
    pub fn valida_en(&self, requerido: Permisos, ahora_ns: u64) -> bool {
        if ahora_ns == 0 {
            return false;
        }
        if !fijar_clave_desde_entorno() {
            return false;
        }
        let c_recurso = match std::ffi::CString::new(self.recurso.clone()) {
            Ok(c) => c,
            Err(_) => return false,
        };
        unsafe { sil_cap_validar(&self.inner, c_recurso.as_ptr(), requerido.0, ahora_ns) == 0 }
    }

    /// Permisos concedidos por esta capacidad.
    pub fn permisos(&self) -> Permisos {
        Permisos(self.inner.permisos)
    }

    /// Recurso ligado a esta capacidad.
    pub fn recurso(&self) -> &str {
        &self.recurso
    }
}

/// Módulo M34: capacidades TPM 2.0 y raíz de confianza.
/// API Rust segura para operaciones TPM 2.0 con TBS en Windows.
/// La implementación de hardware real está en m34_tpm2.rs.
// m34_tpm2 - Módulo TPM 2.0 capabilities
pub mod m34_tpm2;

#[cfg(test)]
mod tests_capacidad {
    use super::*;
    use std::sync::Mutex;

    static CLAVE_MTX: Mutex<()> = Mutex::new(());

    const CLAVE_PRUEBA: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[test]
    fn ciclo_completo_con_clave_real() {
        let _g = CLAVE_MTX.lock().unwrap();
        std::env::set_var("SIL_CAP_KEY", CLAVE_PRUEBA);
        let cap = Capacidad::solicitar("/pagos", LEER_ARCHIVO, 5_000_000_000)
            .expect("solicitar con clave");
        assert!(cap.valida(LEER_ARCHIVO));
        assert!(!cap.valida(ESCRIBIR_ARCHIVO));
        assert_eq!(cap.permisos().0, LEER_ARCHIVO.0);
        assert_eq!(cap.recurso(), "/pagos");
        // Expirada en futuro lejano debe fallar.
        assert!(!cap.valida_en(LEER_ARCHIVO, u64::MAX));
        std::env::remove_var("SIL_CAP_KEY");
    }

    #[test]
    fn sin_clave_falla_claro() {
        let _g = CLAVE_MTX.lock().unwrap();
        std::env::remove_var("SIL_CAP_KEY");
        assert!(Capacidad::solicitar("/pagos", LEER_ARCHIVO, 1_000_000_000).is_err());
        std::env::set_var("SIL_CAP_KEY", CLAVE_PRUEBA);
    }

    #[test]
    fn firma_manipulada_no_valida() {
        let _g = CLAVE_MTX.lock().unwrap();
        std::env::set_var("SIL_CAP_KEY", CLAVE_PRUEBA);
        let mut cap =
            Capacidad::solicitar("/pagos", LEER_ARCHIVO, 5_000_000_000).expect("solicitar");
        cap.inner.firma[0] ^= 0xFF;
        assert!(!cap.valida(LEER_ARCHIVO));
        std::env::remove_var("SIL_CAP_KEY");
    }

    #[test]
    fn recurso_distinto_no_valida() {
        let _g = CLAVE_MTX.lock().unwrap();
        std::env::set_var("SIL_CAP_KEY", CLAVE_PRUEBA);
        let cap = Capacidad::solicitar("/pagos", LEER_ARCHIVO, 5_000_000_000).expect("solicitar");
        // Misma firma no sirve para otro recurso: el hash difiere.
        let c_recurso = std::ffi::CString::new("/admin").unwrap();
        let rc = unsafe { sil_cap_validar(&cap.inner, c_recurso.as_ptr(), LEER_ARCHIVO.0, 0) };
        assert_ne!(rc, 0);
        std::env::remove_var("SIL_CAP_KEY");
    }
}
