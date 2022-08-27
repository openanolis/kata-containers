# DBS-CLI

> for more details about args: refer to [`doc:args`](docs/args.md)

# 1. Examples:

```bash
./dbs-cli \
  --kernel-path ~/data/build/dbs/vmlinux.bin \
  --rootfs ~/data/build/dbs/rootfs.dmg \
  --boot-args console=ttyS0 tty0 reboot=k debug panic=1 pci=off root=/dev/vda1 ;
```

For the rootfs from firecracker:

```bash
./dbs-cli \
  --kernel-path ~/data/build/dbs/vmlinux.bin \
  --rootfs ~/data/build/dbs/bionic.rootfs.ext4 \
  --boot-args console=ttyS0 tty0 reboot=k debug panic=1 pci=off root=/dev/vda ;
```


For the rootfs build from kata:

```bash
./dbs-cli \
  --kernel-path ~/data/build/dbs/vmlinux.bin \
  --rootfs ~/data/build/dbs/kata-containers.img \
  --boot-args console=ttyS0 tty0 reboot=k debug panic=1 pci=off root=/dev/vda1 ;
```

# Usage

## 1. Exit vm

> If you want to exit vm, just input `reboot` in vm's console.

# Acknowledgement
Part of the code is based on the [Cloud Hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) project, [`crosvm`](https://github.com/google/crosvm) project and [Firecracker](https://github.com/firecracker-microvm/firecracker) project. They are all rust written virtual machine managers with advantages on safety and security.

`Dragonball sandbox` is designed to be a VMM that is customized for Kata Containers and we will focus on optimizing container workloads for Kata ecosystem. The focus on the Kata community is what differentiates us from other rust written virtual machines.

# License

`Dragonball` is licensed under [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0.