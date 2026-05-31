(module
  (import "xac:drill" "output_blocked" (func $output_blocked (result i32)))
  (import "xac:drill" "mine" (func $mine (result i32)))
  (func (export "tick")
    (if (i32.eqz (call $output_blocked))
      (then
        (drop (call $mine))))))
