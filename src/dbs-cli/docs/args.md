The configuration of `dbs-cli` refers to `dragonball`, while the only difference lies on that the `serial_path` argument is disabled.

As a result, dbs-cli connect to vm via the stdio console.

|     arguments      | required |                           default value                            |                                   description                                    |
|:------------------:|:--------:|:------------------------------------------------------------------:|:--------------------------------------------------------------------------------:|
|      `rootfs`      |   true   |                                 -                                  |                            The path to rootfs image.                             |
|   `kernel_path`    |   true   |                                 -                                  | The path of kernel image (Only uncompressed kernel is supported for Dragonball). |
|    `boot_args`     |  false   | `console=ttyS0 tty0 reboot=k debug panic=1 pci=off root=/dev/vda1` |                     The boot arguments passed to the kernel.                     |
|     `is_root`      |  false   |                               `true`                               |               Decide the device to be the root boot device or not.               |
|   `is_read_only`   |  false   |                              `false`                               |                      The driver opened in read-only or not.                      |
|       `vcpu`       |  false   |                                `1`                                 |                           The number of vcpu to start.                           |
|     `max_vcpu`     |  false   |                                `1`                                 |                       The max number of vpu can be added.                        |
|      `cpu_pm`      |  false   |                                `0`                                 |                               vpmu support level.                                |
| `threads_per_core` |  false   |                                `1`                                 |         Threads per core to indicate hyper-threading is enabled or not.          |
|  `cores_per_die`   |  false   |                                `1`                                 |                 Cores per die to guide guest cpu topology init.                  |
| `dies_per_socket`  |  false   |                                `1`                                 |                   Dies per socket to guide guest cpu topology.                   |
|     `sockets`      |  false   |                                `1`                                 |                              The number of sockets.                              |
|     `mem_type`     |  false   |                              `shmem`                               |                Memory type that can be either hugetlbfs or shmem.                |
|     `mem_file`     |  false   |                                 ``                                 |                                Memory file path.                                 |
|   `initrd_path`    |  false   |                               `None`                               |                               The path of initrd.                                |