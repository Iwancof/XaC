(module
  (import "xac:drone" "battery_percent" (func $battery_percent (result i32)))
  (import "xac:drone" "logic_fuel_remaining" (func $logic_fuel_remaining (result i64)))
  (import "xac:drone" "has_job" (func $has_job (result i32)))
  (import "xac:drone" "has_pending_job" (func $has_pending_job (result i32)))
  (import "xac:drone" "return_to_port" (func $return_to_port (result i32)))
  (import "xac:drone" "deliver" (func $deliver (result i32)))
  (import "xac:drone" "claim_delivery_job" (func $claim_delivery_job (result i32)))
  (import "xac:drone" "idle" (func $idle (result i32)))
  (func (export "tick")
    (if (i32.lt_s (call $battery_percent) (i32.const 25))
      (then
        (drop (call $return_to_port))
        (return)))
    (if (i64.lt_u (call $logic_fuel_remaining) (i64.const 100))
      (then
        (drop (call $return_to_port))
        (return)))
    (if (call $has_job)
      (then
        (drop (call $deliver))
        (return)))
    (if (call $has_pending_job)
      (then
        (drop (call $claim_delivery_job))
        (return)))
    (drop (call $idle))))
