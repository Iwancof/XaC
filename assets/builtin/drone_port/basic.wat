(module
  (import "xac:drone_port" "charge_docked_drones" (func $charge_docked_drones (result i32)))
  (import "xac:drone_port" "create_delivery_job" (func $create_delivery_job (param i32 i32 i32) (result i32)))
  (import "xac:drone_port" "dispatch_idle_drones" (func $dispatch_idle_drones (result i32)))
  (import "xac:common" "stock_count" (func $stock_count (param i32) (result i32)))
  (func (export "tick")
    (drop (call $charge_docked_drones))
    (if (i32.gt_s (call $stock_count (i32.const 2)) (i32.const 50))
      (then
        (drop (call $create_delivery_job (i32.const 2) (i32.const 10) (i32.const 0)))
      ))
    (drop (call $dispatch_idle_drones))))
