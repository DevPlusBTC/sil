; Verificación con Z3 de varias propiedades a la vez.
(task test_z3_complex
  (params (x i64) (y i64))
  (returns booleano)
  (body
    (assume (> x 0))
    (assume (> y 0))
    (prove (> (+ x y) x))
    (prove (> (+ x y) y))
    (prove (> (* x y) 0))
    (return verdadero)))
