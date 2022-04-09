
pathvar add "C:\Program Files\qemu"
# cargo build

# qemu-img create -f qcow2 qemu_image 8G

#qemu-system-x86_64 -d trace:help

qemu-system-x86_64 -machine q35 -smp 2 -no-reboot -s -D misc/qemu.log -d int -m 1024M -serial stdio -bios ./misc/ovmf-x64/OVMF_CODE-pure-efi.fd  -drive file=misc/kernel.iso,index=1,media=cdrom -drive file=misc/ovmf-x64/UefiShell.iso,index=2,media=cdrom

#qemu-system-x86_64 -machine q35 -smp 2 -no-reboot -s -D misc/qemu.log -d int -m 1024M -monitor stdio -bios ./misc/ovmf-x64/OVMF_CODE-pure-efi.fd  -drive file=misc/kernel.iso,index=1,media=cdrom -drive file=misc/ovmf-x64/UefiShell.iso,index=2,media=cdrom

# qemu-system-x86_64 -machine q35 -smp 2 -no-reboot -s  -D qemu.log -d int -m 1024M -serial stdio -bios ./ovmf-x64/OVMF_CODE-pure-efi.fd -device driver=e1000,netdev=n0 -netdev user,id=n0,tftp=target/x86_64-unknown-uefi/debug,bootfile=kernel.efi 
#-drive file=misc/nvm.img,if=none,id=nvm -device nvme,serial=deadbeef,drive=nvm
# qemu-system-x86_64 -no-reboot -s -D qemu.log -d int -m 1024 -nographic -bios ./ovmf-x64/OVMF_CODE-pure-efi.fd -device driver=e1000,netdev=n0 -netdev user,id=n0,tftp=target/x86_64-unknown-uefi/debug,bootfile=RustKernel.efi

#qemu-system-x86_64 -machine q35 -smp 2 -m 4G -serial stdio -bios ./ovmf-x64/OVMF_CODE-pure-efi.fd -boot order=d qemu_image -net nic -net user,hostfwd=tcp::22-:22
# -cdrom C:/Users/olive/Downloads/ubuntu-20.04.4-live-server-amd64.iso 
