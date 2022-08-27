# DBS-CLI

> for more details about args: refer to [`doc:args`](docs/args.md)

# Examples:

```bash
./dbs-cli \
  --kernel-path ~/data/build/dbs/vmlinux.bin \
  --rootfs ~/data/build/dbs/rootfs.dmg \
  --boot_args console=ttyS0 tty0 reboot=k debug panic=1 pci=off root=/dev/vda1 ;
```

For the rootfs from firecracker:

```bash
./dbs-cli \
  --kernel-path ~/data/build/dbs/vmlinux.bin \
  --rootfs ~/data/build/dbs/bionic.rootfs.ext4 \
  --boot_args console=ttyS0 tty0 reboot=k debug panic=1 pci=off root=/dev/vda ;
```


For the rootfs build from kata:

```bash
./dbs-cli \
  --kernel-path ~/data/build/dbs/vmlinux.bin \
  --rootfs ~/data/build/dbs/kata-containers.img \
  --boot_args console=ttyS0 tty0 reboot=k debug panic=1 pci=off root=/dev/vda1 ;
```

# Usage

## 1. Exit vm

> If you want to exit vm, just input `reboot` in vm's console.