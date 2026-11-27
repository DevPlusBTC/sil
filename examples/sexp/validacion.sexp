; Validación de edad.
(task validar_edad
  (params (edad i64))
  (returns i64)
  (body
    (assume (>= edad 18))
    (assume (<= edad 150))
    (prove (>= edad 18))
    (return 1)))

(task es_mayor_18
  (params (edad i64))
  (returns i64)
  (body
    (assume (>= edad 18))
    (assume (<= edad 150))
    (return 1)))
