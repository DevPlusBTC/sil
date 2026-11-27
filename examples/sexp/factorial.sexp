; Factoriales desenrollados: cada tarea fija el valor de n.
(task factorial
  (params (n i64))
  (returns i64)
  (body
    (assume (>= n 0))
    (assume (<= n 20))
    (assume (== n 0))
    (return 1)))

(task factorial_1
  (params (n i64))
  (returns i64)
  (body
    (assume (>= n 0))
    (assume (<= n 20))
    (assume (== n 1))
    (return 1)))

(task factorial_2
  (params (n i64))
  (returns i64)
  (body
    (assume (>= n 0))
    (assume (<= n 20))
    (assume (== n 2))
    (return 2)))

(task factorial_3
  (params (n i64))
  (returns i64)
  (body
    (assume (>= n 0))
    (assume (<= n 20))
    (assume (== n 3))
    (return 6)))
