(module
  (import "xac:router" "push_dir" (func $push_dir (param i32) (result i32)))
  (func (export "tick")
    (drop (call $push_dir (i32.const 1)))))
