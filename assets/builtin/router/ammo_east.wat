(module
  (import "xac:router" "output_item_available" (func $output_item_available (param i32 i32) (result i32)))
  (import "xac:router" "push_item_dir" (func $push_item_dir (param i32 i32) (result i32)))
  (func (export "tick")
    (if (call $output_item_available (i32.const 2) (i32.const 1))
      (then
        (drop (call $push_item_dir (i32.const 2) (i32.const 1)))))))
