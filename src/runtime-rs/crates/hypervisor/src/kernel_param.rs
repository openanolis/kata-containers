// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{VM_ROOTFS_DRIVER_BLK, VM_ROOTFS_DRIVER_PMEM};

// Port where the agent will send the logs. Logs are sent through the vsock in cases
// where the hypervisor has no console.sock, i.e dragonball
const VSOCK_LOGS_PORT: &str = "1025";

const KERNEL_KV_DELIMITER: &str = "=";
const KERNEL_PARAM_DELIMITER: &str = " ";

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub key: String,
    pub value: String,
}

impl Param {
    pub fn new(key: &str, value: &str) -> Self {
        Param {
            key: key.to_owned(),
            value: value.to_owned(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct KernelParams {
    params: Vec<Param>,
}

impl KernelParams {
    pub(crate) fn new(debug: bool) -> Self {
        // default kernel params
        let mut params = vec![
            Param {
                key: "reboot".to_string(),
                value: "k".to_string(),
            },
            Param {
                key: "earlyprintk".to_string(),
                value: "ttyS0".to_string(),
            },
            Param {
                key: "initcall_debug".to_string(),
                value: "".to_string(),
            },
            Param {
                key: "panic".to_string(),
                value: "1".to_string(),
            },
            Param {
                key: String::from("systemd.unit"),
                value: String::from("kata-containers.target"),
            },
            Param {
                key: String::from("systemd.mask"),
                value: String::from("systemd-networkd.service"),
            },
            Param {
                key: String::from("systemd.mask"),
                value: String::from("systemd-networkd.socket"),
            },
        ];

        if debug {
            params.push(Param {
                key: "agent.log_vport".to_string(),
                value: VSOCK_LOGS_PORT.to_string(),
            });
        }

        Self { params }
    }

    pub(crate) fn new_rootfs_kernel_params(rootfs_driver: &str) -> Self {
        let params = match rootfs_driver {
            VM_ROOTFS_DRIVER_BLK => {
                vec![
                    Param {
                        key: "root".to_string(),
                        value: "/dev/vda1".to_string(),
                    },
                    Param {
                        key: "rootflags".to_string(),
                        value: "data=ordered,errors=remount-ro ro".to_string(),
                    },
                    Param {
                        key: "rootfstype".to_string(),
                        value: "ext4".to_string(),
                    },
                ]
            }
            VM_ROOTFS_DRIVER_PMEM => {
                vec![
                    Param {
                        key: "root".to_string(),
                        value: "/dev/pmem0p1".to_string(),
                    },
                    Param {
                        key: "rootflags".to_string(),
                        value: "data=ordered,errors=remount-ro,dax ro".to_string(),
                    },
                    Param {
                        key: "rootfstype".to_string(),
                        value: "ext4".to_string(),
                    },
                ]
            }
            _ => vec![],
        };
        Self { params }
    }

    pub(crate) fn append(&mut self, params: &mut KernelParams) {
        self.params.append(&mut params.params);
    }

    pub(crate) fn from_string(params_string: &str) -> Self {
        let mut params = vec![];

        let parameters_vec: Vec<&str> = params_string.split(KERNEL_PARAM_DELIMITER).collect();

        for param in parameters_vec.iter() {
            if param.is_empty() {
                continue;
            }

            let ps: Vec<&str> = param.splitn::<_>(2, KERNEL_KV_DELIMITER).collect();

            if ps.len() == 2 {
                params.push(Param {
                    key: String::from(ps[0]),
                    value: String::from(ps[1]),
                });
            } else {
                params.push(Param {
                    key: String::from(ps[0]),
                    value: String::from(""),
                });
            }
        }

        Self { params }
    }

    pub(crate) fn to_string(&self) -> String {
        let mut parameters: Vec<String> = Vec::new();

        for param in &self.params {
            if param.key.is_empty() && param.value.is_empty() {
                continue;
            } else if param.key.is_empty() {
                parameters.push(param.value.to_string());
            } else if param.value.is_empty() {
                parameters.push(param.key.to_string());
            } else {
                parameters.push(format!(
                    "{}{}{}",
                    param.key, KERNEL_KV_DELIMITER, param.value
                ));
            }
        }

        parameters.join(KERNEL_PARAM_DELIMITER)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_params() {
        let expect_params_string = "k1=v1 k2=v2 k3=v3".to_string();
        let expect_params = KernelParams {
            params: vec![
                Param {
                    key: "k1".to_string(),
                    value: "v1".to_string(),
                },
                Param {
                    key: "k2".to_string(),
                    value: "v2".to_string(),
                },
                Param {
                    key: "k3".to_string(),
                    value: "v3".to_string(),
                },
            ],
        };

        // check kernel params from string
        let kernel_params = KernelParams::from_string(&expect_params_string);
        assert_eq!(kernel_params, expect_params);

        // check kernel params to string
        let kernel_params_string = expect_params.to_string();
        assert_eq!(kernel_params_string, expect_params_string);
    }
}
