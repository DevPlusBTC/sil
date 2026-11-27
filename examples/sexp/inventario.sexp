; Valor de inventario y reposición de stock.
(task valor_producto
  (params (precio i64) (stock i64))
  (returns i64)
  (body
    (assume (> precio 0))
    (assume (< precio 1000000))
    (assume (>= stock 0))
    (assume (< stock 10000))
    (prove (> precio 0))
    (prove (>= stock 0))
    (return (* precio stock))))

(task agregar_stock
  (params (stock i64) (cantidad i64))
  (returns i64)
  (body
    (assume (> cantidad 0))
    (assume (< cantidad 1000))
    (assume (>= stock 0))
    (assume (< stock 10000))
    (prove (> (+ stock cantidad) stock))
    (return (+ stock cantidad))))
