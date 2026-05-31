(module
  (import "xac:turret" "attack_nearest" (func $attack_nearest (result i32)))
  (func (export "tick")
    (drop (call $attack_nearest))))
