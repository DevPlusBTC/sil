/* sil_host.js - Host de referencia para módulos SIL compilados a WASM.
 *
 * Implementa los imports "sil.*" que el backend wasm declara, con la misma
 * semántica que sil-rt: capacidades HMAC-SHA256, canales FIFO, E/S por
 * archivo y red TCP. La clave sale de SIL_CAP_KEY (64 hex chars).
 *
 * La red es síncrona de verdad: un worker hace el TCP y el hilo principal
 * espera con Atomics.wait sobre memoria compartida.
 *
 * Uso:
 *   const { instanciar } = require("./mi_programa.host.js");
 *   const api = await instanciar("./mi_programa.wasm");
 *   api.mi_tarea();
 *
 * Sin SIL_CAP_KEY, solicitar retorna 0 (capacidad inválida) y el programa
 * que la use falla de forma visible en vez de operar sin permiso.
 */
"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const { Worker } = require("node:worker_threads");

const PERM_LEER = 0x01;
const PERM_ESCRIBIR = 0x02;
const PERM_RED = 0x04;
const MAGIC = 0xc4a981e5;

function leerClave() {
  const hex = process.env.SIL_CAP_KEY || "";
  if (!/^[0-9a-fA-F]{64}$/.test(hex)) return null;
  return Buffer.from(hex, "hex");
}

function sha256(datos) {
  return crypto.createHash("sha256").update(datos).digest();
}

function hmac(clave, datos) {
  return crypto.createHmac("sha256", clave).update(datos).digest();
}

function le64(v) {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(v));
  return b;
}

function le32(v) {
  const b = Buffer.alloc(4);
  b.writeUInt32LE(v >>> 0);
  return b;
}

function firmaCap(clave, permisos, expiracion, recursoHash) {
  return hmac(
    clave,
    Buffer.concat([le64(MAGIC), le32(permisos), le64(expiracion), recursoHash])
  );
}

function ahoraNs() {
  return Number(process.hrtime.bigint());
}

/* TCP síncrono vía worker: retorna { ok, datos } (datos solo si espera). */
function tcpSync(destino, enviar, espera) {
  const [host, puerto] = destino.split(":");
  const compartida = new SharedArrayBuffer(65536 + 8);
  const vista = new Int32Array(compartida, 0, 2);
  const codigo = `
    const net = require("node:net");
    const { parentPort, workerData } = require("node:worker_threads");
    const vista = new Int32Array(workerData.compartida, 0, 2);
    const buf = Buffer.from(workerData.enviarHex || "", "hex");
    const s = net.connect(Number(workerData.puerto), workerData.host, () => {
      const escribir = buf.length ? buf : Buffer.alloc(0);
      s.write(escribir, () => {
        if (!workerData.espera) { s.end(); Atomics.store(vista, 0, buf.length); Atomics.notify(vista, 0); }
      });
      if (workerData.espera) {
        s.once("data", (d) => {
          Buffer.from(compartida(8)).set(d.subarray(0, Math.min(d.length, 65536)));
          Atomics.store(vista, 0, d.length);
          Atomics.store(vista, 1, 1);
          Atomics.notify(vista, 0);
          s.end();
        });
      }
    });
    function compartida(off) { return Buffer.from(workerData.compartida, off); }
    s.on("error", () => { Atomics.store(vista, 0, -1); Atomics.notify(vista, 0); });
    s.on("close", () => { Atomics.notify(vista, 0); });
  `;
  const w = new Worker(codigo, {
    eval: true,
    workerData: {
      compartida,
      host,
      puerto,
      enviarHex: enviar ? Buffer.from(enviar).toString("hex") : "",
      espera: !!espera,
    },
  });
  w.unref();
  Atomics.wait(vista, 0, 0, 15000);
  const n = Atomics.load(vista, 0);
  const llego = Atomics.load(vista, 1) === 1;
  w.terminate();
  if (n < 0) return { ok: -1, datos: null };
  if (!espera) return { ok: n, datos: null };
  if (!llego) return { ok: 0, datos: Buffer.alloc(0) };
  return { ok: n, datos: Buffer.from(compartida, 8, n) };
}

class HostSIL {
  constructor() {
    this.memoria = null;
    this.caps = [null]; // índice 0 = inválida
    this.canales = [];
    this.bump = 32768; // allocator para resultados (tras estáticos)
  }

  vista() {
    return Buffer.from(this.memoria.buffer);
  }

  bytes(ptr, len) {
    return this.vista().subarray(ptr, ptr + len);
  }

  texto(ptr, len) {
    return this.bytes(ptr, len).toString("utf8");
  }

  /* Reserva n bytes en la memoria lineal (crece si hace falta). */
  reservar(n) {
    const PAGINA = 65536;
    while (this.bump + n > this.memoria.buffer.byteLength) {
      this.memoria.grow(1);
    }
    const off = this.bump;
    this.bump += n;
    return off;
  }

  empaquetar(ptr, len) {
    return (BigInt(ptr) << 32n) | BigInt(len);
  }

  validar(handle, ptr, len, req) {
    const clave = leerClave();
    const c = this.caps[Number(handle)];
    if (!clave || !c) return 0;
    if (Number(req) !== 0 && (c.permisos & Number(req)) !== Number(req)) return 0;
    if (BigInt(ahoraNs()) > c.expiracion) return 0;
    let rh = c.rh;
    if (!(ptr === 0 && len === 0)) {
      rh = sha256(this.bytes(ptr, len));
      if (!rh.equals(c.rh)) return 0;
    }
    const esp = firmaCap(clave, c.permisos, c.expiracion, rh);
    return esp.equals(c.firma) ? 1 : 0;
  }

  imports() {
    const host = this;
    return {
      sil: {
        sil_cap_solicitar(ptr, len, perm, ttl) {
          const clave = leerClave();
          if (!clave) {
            process.stderr.write("SIL: sin clave (define SIL_CAP_KEY)\n");
            return 0n;
          }
          const rh = sha256(host.bytes(ptr, len));
          const exp = BigInt(ahoraNs()) + (ttl === 0n ? 500000000n : ttl);
          host.caps.push({
            permisos: Number(perm),
            expiracion: exp,
            rh,
            firma: firmaCap(clave, Number(perm), exp, rh),
          });
          return BigInt(host.caps.length - 1);
        },
        sil_cap_validar(handle, ptr, len, req) {
          return host.validar(handle, ptr, len, req);
        },
        sil_canal_crear(cap) {
          host.canales.push({ buf: [], cap: Number(cap) || 64 });
          return BigInt(host.canales.length - 1);
        },
        sil_canal_enviar(h, v) {
          const c = host.canales[Number(h)];
          if (!c || c.buf.length >= c.cap) return 0;
          c.buf.push(v);
          return 1;
        },
        sil_canal_recibir(h) {
          const c = host.canales[Number(h)];
          if (!c || c.buf.length === 0) return 0n;
          return c.buf.shift();
        },
        sil_io_leer(ptr, len, cap) {
          if (!host.validar(cap, ptr, len, BigInt(PERM_LEER))) {
            process.stderr.write("SIL: capacidad invalida para leer\n");
            return 0n;
          }
          let datos;
          try {
            datos = fs.readFileSync(host.texto(ptr, len));
          } catch {
            return 0n;
          }
          if (datos.length === 0) return 0n;
          const off = host.reservar(datos.length);
          host.vista().set(datos, off);
          return host.empaquetar(off, datos.length);
        },
        sil_io_escribir(rptr, rlen, dptr, dlen, cap) {
          if (!host.validar(cap, rptr, rlen, BigInt(PERM_ESCRIBIR))) return -1n;
          try {
            fs.writeFileSync(host.texto(rptr, rlen), host.bytes(dptr, dlen));
            return BigInt(dlen);
          } catch {
            return -1n;
          }
        },
        sil_red_enviar(pptr, plen, dptr, dlen, cap) {
          if (!host.validar(cap, pptr, plen, BigInt(PERM_RED))) return -1n;
          const r = tcpSync(host.texto(pptr, plen), host.bytes(dptr, dlen), false);
          return BigInt(r.ok);
        },
        sil_red_recibir(pptr, plen, cap) {
          if (!host.validar(cap, pptr, plen, BigInt(PERM_RED))) return 0n;
          const r = tcpSync(host.texto(pptr, plen), null, true);
          if (!r.datos || r.datos.length === 0) return 0n;
          const off = host.reservar(r.datos.length);
          host.vista().set(r.datos, off);
          return host.empaquetar(off, r.datos.length);
        },
      },
    };
  }
}

async function instanciar(rutaWasm) {
  const bytes = fs.readFileSync(rutaWasm);
  const host = new HostSIL();
  // La memoria la crea el host y el módulo la importa: disponible antes
  // de que corra el start durante la instanciación.
  host.memoria = new WebAssembly.Memory({ initial: 1 });
  const mod = await WebAssembly.compile(bytes);
  const instancia = await WebAssembly.instantiate(mod, {
    wasi_snapshot_preview1: {
      proc_exit: () => {},
      fd_write: () => 0,
    },
    sil: Object.assign({ memory: host.memoria }, host.imports().sil),
  });
  return instancia.exports;
}

module.exports = { HostSIL, instanciar };
