(module
  (import "xac:assembler" "set_recipe" (func $set_recipe (param i32) (result i32)))
  (import "xac:assembler" "can_produce" (func $can_produce (result i32)))
  (import "xac:assembler" "produce" (func $produce (result i32)))
  (func (export "tick")
    (drop (call $set_recipe (i32.const 1)))
    (if (call $can_produce)
      (then
        (drop (call $produce))))))
