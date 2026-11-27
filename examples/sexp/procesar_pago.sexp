; Pago con impuesto y restricciones de hardware.
(task procesar_pago
  (params (monto i64))
  (returns i64)
  (body
    (assume (> monto 0))
    (prove (> monto 0))
    (let impuesto i64 (* monto 19))
    (return (+ monto impuesto))
    (restrictions
      (gestion_memoria arena)
      (tiempo_ejecucion constante))))
