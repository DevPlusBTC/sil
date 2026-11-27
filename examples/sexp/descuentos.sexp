; Descuento porcentual con precondiciones acotadas.
(task descuento_porcentaje
  (params (precio i64) (pct i64))
  (returns i64)
  (body
    (assume (> precio 0))
    (assume (< precio 10000))
    (assume (>= pct 0))
    (assume (<= pct 50))
    (return (/ (* precio (- 100 pct)) 100))))
