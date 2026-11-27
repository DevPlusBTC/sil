; Canal entre fibras y capacidad solicitada antes de usarla.
(task trabajo
  (params)
  (returns i64)
  (body
    (chan c (capacity 8))
    (send 40 c)
    (recv c v)
    (return (+ v 2))))

(task principal
  (params)
  (returns i64)
  (body
    (request lectura_archivo "/tmp/datos" (as cap))
    (validate cap)
    (spawn trabajo)
    (return 42)))
