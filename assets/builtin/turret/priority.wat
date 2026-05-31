(module
  (import "xac:turret" "attack_best" (func $attack_best (param i32) (result i32)))
  (func (export "tick")
    (drop (call $attack_best (i32.const 9812)))))
