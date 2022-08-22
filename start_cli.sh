# temporally for debug purpose

cd src/dbs-cli && \
RUST_LOG=debug cargo run --release -- \
  --kernel-path /home/fanqiliang/data/build/dbs/vmlinux.bin \
  --rootfs /home/fanqiliang/data/build/dbs/firecracker/bionic.rootfs.ext4 \
  --boot-args "console=ttyS1 reboot=k panic=1 pci=off root=/dev/vda";