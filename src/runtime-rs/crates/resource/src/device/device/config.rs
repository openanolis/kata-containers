use hypervisor::IoLimits;
use std::collections::HashMap;

/// DeviceInfo is an embedded type that contains device data common to all types of devices.
#[derive(Default, Debug, Clone)]
pub struct DeviceInfo {
    /// Hostpath is device path on host
    pub host_path: String,

    /// ContainerPath is device path inside container
    pub container_path: String,

    /// Type of device: c, b, u or p
    /// c , u - character(unbuffered)
    /// p - FIFO
    /// b - block(buffered) special file
    /// More info in mknod(1).
    pub dev_type: String,

    /// Major, minor numbers for device.
    pub major: i64,
    pub minor: i64,

    /// FileMode permission bits for the device.
    pub file_mode: u32,

    /// id of the device owner.
    pub uid: u32,
    /// id of the device group.
    pub gid: u32,
    /// ID for the device that is passed to the hypervisor.
    pub id: String,

    /// The Bus::Device.Function ID if the device is already
    /// bound to VFIO driver.
    pub bdf: Option<String>,
    /// DriverOptions is specific options for each device driver
    /// for example, for BlockDevice, we can set DriverOptions["blockDriver"]="virtio-blk"
    pub driver_options: HashMap<String, String>,

    pub io_limits: Option<IoLimits>,
}
