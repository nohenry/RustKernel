set confirm off
set logging on
set trace-commands on
target remote localhost:1234
add-symbol-file D:/Developement/Projects/RustKernel/target/kernel_target/debug/kernel
