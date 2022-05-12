# Device

## Device Manager

Currently we have following device manager:
1. address space manager: abstracts virtual machine's physical management and provide mapping for guest virtual memory and MMIO ranges of emulated virtual devices, pass-through devices and vcpu.
2. config manager: provides abstractions for configuration information.
3. console manager: provides management for all console devices
4. resource manager: provides resource management for legacy_irq_pool, msi_irq_pool, pio_pool, mmio_pool, mem_pool, kvm_mem_slot_pool
5. vsock device manager: provides configuration info for virtio-vsock and management for all vsock devices
   

## Device supported
virtio-vsock

