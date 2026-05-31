(module
  (import "xac:drone_port" "dispatch" (func $dispatch (result i32)))
  (func (export "tick")
    (drop (call $dispatch))))
