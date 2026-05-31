(module
  (import "xac:router" "push_any" (func $push_any (result i32)))
  (func (export "tick")
    (drop (call $push_any))))
