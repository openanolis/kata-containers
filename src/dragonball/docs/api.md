# API

We provide plenty API for Kata runtime to interact with `Dragonball` virtual machine manager.
This document provides the introduction for each of them.

## ConfigureBootSource
Configure the boot source of the microVM using `BootSourceConfig`. This action can only be called before the microVM has booted.

### Boot Source Config
1. kernel_image_path: Path of the kernel image.
2. initrd_path: Path of the initrd (could be None)
3. boot_args: Boot arguments passed to the kernel (could be None)
