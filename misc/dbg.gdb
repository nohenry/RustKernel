target remote 10.0.0.77:1234
add-symbol-file ./target/kernel_target/debug/kernel
layout split
b kernel/src/main.rs:90
b kernel::_start
b *0x21c794