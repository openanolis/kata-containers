The configuration of `dbs-cli` refers to `dragonball`, while the only difference lies on that the `serial_path` argument is disabled.

As a result, dbs-cli connect to vm via the stdio console.

|     arguments      | required |                           default value                            |                                   description                                    |
|:------------------:|:--------:|:------------------------------------------------------------------:|:--------------------------------------------------------------------------------:|
|      `rootfs`      |   true   |                                 -                                  |                            The path to rootfs image.                             |
|   `kernel-path`    |   true   |                                 -                                  | The path of kernel image (Only uncompressed kernel is supported for Dragonball). |
|     `log-file`     |  false   |                          `"dbs-cli.log"`                           |                               The path to log file                               |
|    `log-level`     |  false   |                              `"Info"`                              |                                The logging level.                                |
|    `boot-args`     |  false   | `console=ttyS0 tty0 reboot=k debug panic=1 pci=off root=/dev/vda1` |                     The boot arguments passed to the kernel.                     |
|     `is-root`      |  false   |                               `true`                               |               Decide the device to be the root boot device or not.               |
|   `is-read-only`   |  false   |                              `false`                               |                      The driver opened in read-only or not.                      |
|       `vcpu`       |  false   |                                `1`                                 |                           The number of vcpu to start.                           |
|     `max-vcpu`     |  false   |                                `1`                                 |                       The max number of vpu can be added.                        |
|      `cpu-pm`      |  false   |                                `0`                                 |                               vpmu support level.                                |
| `threads-per-core` |  false   |                                `1`                                 |         Threads per core to indicate hyper-threading is enabled or not.          |
|  `cores-per-die`   |  false   |                                `1`                                 |                 Cores per die to guide guest cpu topology init.                  |
| `dies-per-socket`  |  false   |                                `1`                                 |                   Dies per socket to guide guest cpu topology.                   |
|     `sockets`      |  false   |                                `1`                                 |                              The number of sockets.                              |
|     `mem-type`     |  false   |                              `shmem`                               |                Memory type that can be either hugetlbfs or shmem.                |
|     `mem-file`     |  false   |                                 ``                                 |                                Memory file path.                                 |
|   `initrd-path`    |  false   |                               `None`                               |                               The path of initrd.                                |