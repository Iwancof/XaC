(module
  (func $spin (param $n i32)
    (local $i i32)
    (local.set $i (i32.const 0))
    (block $exit
      (loop $loop
        (br_if $exit (i32.ge_s (local.get $i) (local.get $n)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))))
  (func (export "tick") (result i32)
    (i32.const 1)))
