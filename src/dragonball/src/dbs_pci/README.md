# dbs-pci

## Introduction

dbs-pci is a crate for emulating pci device.

There are several components in dbs-pci crate building together to emulate pci device behaviour :

1. device mod: mainly provide the trait for `PciDevice`, providing the ability to get id, write PCI configuration space, read PCI configuration space and `as_any` to downcast the trait object to the actual device type.

2. configuration mod: simulate PCI device configuration header and manage PCI Bar configuration. The PCI Specification defines the organization of the 256-byte Configuration Space registers and imposes a specific template for the space. The first 64 byets of configuration space are standardardized as configuration space header.

3. bus mod: simulate PCI buses, to simplify the implementation, PCI hierarchy is not supported. So all PCI devices are directly connected to the PCI root bus. PciBus has bus id, pci devices attached and pci bus ioport, iomem resource use condition.