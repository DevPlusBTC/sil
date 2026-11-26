## Qué cambia

<!-- Una o dos frases. Si el resumen necesita "y", probablemente son dos PRs. -->

## Por qué

<!-- El problema real y su evidencia. Si corrige un defecto, cita el fichero:línea
     del código base y cómo se reprodujo (comando + salida). -->

## Cómo se verificó

<!-- Pega el comando y su salida. Evidencia, no promesas. -->

```bash
$ scripts/pre-pr.sh
# ... resumen (OK / FALLO / OMITIDO) ...
```

- [ ] `scripts/pre-pr.sh` sin fallos (o con omisiones **declaradas abajo**)
- [ ] Si añadí o cambié tests: `python3 scripts/test_counts.py` ejecutado y commiteado
- [ ] Si toqué `sil-rt` o el runtime emitido de `silc-backend`: `scripts/run_c_tests.sh` en verde
- [ ] Si añadí una capacidad: existe un test que la **ejecuta** y falla si se rompe
- [ ] Si añadí un directorio nuevo: está en los filtros `paths` de `.github/workflows/rust.yml`

**Gates omitidos y por qué** (si no hubo, escribe «ninguno»):

## Alcance y límites

<!-- Qué NO cubre este PR. Sé explícito con lo que no se pudo compilar o probar:
     es la diferencia entre un PR auditable y uno optimista. -->

## Riesgos

<!-- Cambios de comportamiento, compatibilidad ABI/API, migraciones, rendimiento. -->

- [ ] Sin cambios incompatibles (API pública / ABI C / formato de fichero)
- [ ] Si hay cambio incompatible: está documentado en `CHANGELOG.md` y en `docs/SPEC.md`

## Checklist de estilo

- [ ] Commits en formato `<tipo>(<scope>): <imperativo>`
- [ ] Rama corta, un solo tema
- [ ] Documentación actualizada (`README`, `docs/`, `CHANGELOG.md`) si aplica
- [ ] Sin `#[allow]`, `#pragma` ni `--no-verify` para silenciar gates
