; Verificación con Z3: x > 0 implica 2x > x.
(task test_z3
  (params (x i64))
  (returns booleano)
  (body
    (assume (> x 0))
    (prove (> (* x 2) x))
    (return verdadero)))
