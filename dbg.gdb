target remote 10.0.0.77:1234
add-symbol-file ./target/x86_64-unknown-uefi/debug/RustKernel.efi 0x3e24c000
layout split
b src/main.rs:51
set wait=false
