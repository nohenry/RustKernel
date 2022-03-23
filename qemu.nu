
pathvar add "C:\Program Files\qemu"
# cargo build

#qemu-system-x86_64 -d trace:help
qemu-system-x86_64 -machine q35 -smp 2 -no-reboot -s -D qemu.log -d int -m 1024M -serial stdio -bios ./ovmf-x64/OVMF_CODE-pure-efi.fd -device driver=e1000,netdev=n0 -netdev user,id=n0,tftp=target/x86_64-unknown-uefi/debug,bootfile=RustKernel.efi -drive file=misc/nvm.img,if=none,id=nvm -device nvme,serial=deadbeef,drive=nvm
# qemu-system-x86_64 -no-reboot -s -D qemu.log -d int -m 1024 -nographic -bios ./ovmf-x64/OVMF_CODE-pure-efi.fd -device driver=e1000,netdev=n0 -netdev user,id=n0,tftp=target/x86_64-unknown-uefi/debug,bootfile=RustKernel.efi
