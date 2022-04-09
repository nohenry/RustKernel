set confirm off
file
target remote :1234
add-symbol-file D:/Developement/Projects/RustKernel/target/kernel_target/debug/kernel
layout split
b kernel/src/main.rs:70
b kernel::_start
b *0x0217349