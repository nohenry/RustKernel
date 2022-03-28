target remote 10.0.0.77:1234
add-symbol-file ./target/x86_64-unknown-uefi/debug/kernel.efi 0x3e6f8002
layout split
b kernel/src/main.rs:90
set wait=false
