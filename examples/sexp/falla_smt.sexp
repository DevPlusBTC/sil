; Inválido: la postcondición no sigue de la precondición (SMT debe fallar).
(task insegura
  (params (monto i64))
  (returns i64)
  (body
    (prove (> monto 1000000))
    (return monto)))
