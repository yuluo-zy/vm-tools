use kvm_bindings::{KVM_IRQ_ROUTING_IRQCHIP, KVM_IRQ_ROUTING_MSI};
use kvm_ioctls::{Cap, Kvm};
use kvm_ioctls::Cap::Irqchip;
use vmm_sys_util::ioctl_io_nr;
use utils::bitmap::Bitmap;
use anyhow::{Context, Result};

pub(crate) type IrqRoute = kvm_bindings::kvm_irq_routing;
pub(crate) type IrqRouteEntry = kvm_bindings::kvm_irq_routing_entry;
type IrqRouteChip = kvm_bindings::kvm_irq_routing_irqchip;
type IrqChip = kvm_bindings::kvm_irq_routing_entry__bindgen_ty_1;
/// 在 KVM（Kernel-based Virtual Machine）虚拟化环境中，`kvm_irq_routing_irqchip`和`kvm_irq_routing_entry`都是描述虚拟中断路由信息的数据结构，但它们的用途和包含的信息略有不同。
//
// 1. `kvm_irq_routing_irqchip`：这个数据结构主要用于描述虚拟中断控制器（irqchip）的中断路由信息。它包含了中断控制器的类型（irqchip）、中断控制器上的引脚号（pin）和全局系统中断号（gsi）等信息。
//
// 2. `kvm_irq_routing_entry`：这个数据结构更为通用，它可以描述各种类型的中断路由信息，不仅包括虚拟中断控制器的中断路由信息，还可以包括MSI（Message Signaled Interrupts）、MSI-X（Message Signaled Interrupts Extension）等其他类型的中断路由信息。它包含了一个联合体，这个联合体中的字段会根据中断类型的不同而变化。例如，对于irqchip类型的中断，这个联合体会包含一个`kvm_irq_routing_irqchip`结构。
//
// 总的来说，`kvm_irq_routing_irqchip`和`kvm_irq_routing_entry`都是用于描述虚拟中断路由信息的，但`kvm_irq_routing_entry`更为通用，能够描述更多类型的中断路由信息。
ioctl_io_nr!(KVM_CHECK_EXTENSION, KVMIO, 0x03);

/// 在x86的64位架构（也被称为x86_64或AMD64）中，IOAPIC_NUM_PINS通常被设定为24，意味着IOAPIC可以处理24个中断请求。这是因为在x86_64架构中，IOAPIC设备通常有24个引脚用于中断请求。需要注意的是，这个数值可能会因硬件和操作系统的实现而有所不同。
#[cfg(target_arch = "x86_64")]
const IOAPIC_NUM_PINS: u32 = 24;
/// PIC_MASTER_PINS通常指的是主PIC（可编程中断控制器）的引脚数量。在x86架构中，PIC是用于处理中断的设备，通常有两个，一个主PIC和一个从PIC。每个PIC通常有8个引脚，用于处理中断请求。因此，通常我们会看到PIC_MASTER_PINS设置为8。这意味着主PIC可以处理8个中断请求。这些中断请求可以来自各种设备，如键盘、鼠标、硬盘等。
/// 在计算机硬件中，PIC是可编程中断控制器（Programmable Interrupt Controller）的简称，它是用来管理中断请求（IRQ）的硬件设备。
//
// 早期的PC中，有两个PIC，一个被称为主PIC，另一个被称为从PIC。他们的作用是接收来自硬件设备（如键盘、鼠标、硬盘等）的中断请求，并把这些请求发送给CPU处理。
//
// 主PIC和从PIC的区别主要在于它们处理中断请求的方式。主PIC直接将中断请求发送给CPU，而从PIC则将中断请求发送给主PIC，由主PIC再将其发送给CPU。这种设计是为了扩展系统可以处理的中断数量。因为每个PIC只有8个中断线（IRQ0-IRQ7），使用主从PIC可以增加到15个（IRQ0-IRQ7由主PIC处理，IRQ8-IRQ15由从PIC处理）。
#[cfg(target_arch = "x86_64")]
const PIC_MASTER_PINS: u32 = 8;
#[cfg(target_arch = "x86_64")]
const PIC_SLACE_PINS: u32 = 8;
#[cfg(target_arch = "aarch64")]
const IOCHIP_NUM_PINS: u32 = 192;
#[cfg(target_arch = "aarch64")]
const KVM_IRQCHIP: u32 = 0;

fn get_maximum_gsi_cnt(kvmfd: &Kvm) -> u32 {
    // 检查特定功能是否可用
    let mut gsi_count = kvmfd.check_extension_int(Cap::IrqRouting);
    if gsi_count < 0 {
        gsi_count = 0;
    }
    gsi_count as u32
}

fn create_irq_route_entry(gsi: u32, irqchip: u32, pin: u32) -> IrqRouteEntry {
    let irq_route_chip = IrqRouteChip{ irqchip, pin };
    let irq_chip = IrqChip{ irqchip: irq_route_chip};
    IrqRouteEntry{
        gsi,
        type_: KVM_IRQ_ROUTING_IRQCHIP,
        flags: 0,
        pad: 0,
        u: irq_chip,
    }
}
pub struct IrqRouteTable{
    pub irq_routes: Vec<IrqRouteEntry>,
    gsi_bitmap: Bitmap<u64>
}

impl Default for IrqRouteTable {
    fn default() -> Self {
        IrqRouteTable {
            irq_routes: vec![],
            gsi_bitmap: Bitmap::<u64>::new(0),
        }
    }
}

impl IrqRouteTable {
    pub fn new(kvmfd: &Kvm) -> Self {
        let gsi_count = get_maximum_gsi_cnt(kvmfd);
        Self {
            irq_routes: vec![],
            gsi_bitmap: Bitmap::<u64>::new(gsi_count as usize),
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn init_irq_route_table(&mut self) {
        /// 这是由于历史原因和硬件兼容性考虑。
        //
        // 在早期的PC系统中，只有PIC（可编程中断控制器）用于处理中断请求。然而，随着硬件的发展，PIC的中断处理能力已经无法满足需求。因此，后来引入了APIC（高级可编程中断控制器），并且在APIC中又引入了IOAPIC，用于处理IO设备的中断请求。APIC和IOAPIC可以处理的中断数量比PIC多得多。
        //
        // 然而，为了保持对旧硬件和旧操作系统的兼容性，新的硬件和操作系统仍然需要支持PIC。因此，你会看到在虚拟环境中，会同时创建虚拟的PIC和虚拟的IOAPIC。
        //
        // 至于中断路由设置，GSIs 0-15设置为既可以路由到PIC也可以路由到IOAPIC，
        // 而GSIs 16-23只路由到IOAPIC，这主要是为了兼容旧的操作系统。因为旧的操作系统可能只知道如何通过PIC处理中断，而不知道如何通过IOAPIC处理中断。
        // 因此，对于这些操作系统，将GSIs 0-15路由到PIC是必要的。而对于新的操作系统，它们知道如何通过IOAPIC处理中断，因此可以利用IOAPIC处理更多的中断请求。

        for i in 0..PIC_MASTER_PINS {
            self.irq_routes.push(create_irq_route_entry(
                i,
                kvm_bindings::KVM_IRQCHIP_PIC_MASTER,
                i,
            ));
        }

        for i in PIC_MASTER_PINS..(PIC_MASTER_PINS + PIC_SLACE_PINS) {
            self.irq_routes.push(create_irq_route_entry(
                i,
                kvm_bindings::KVM_IRQCHIP_PIC_SLAVE,
                i - PIC_MASTER_PINS,
            ));
        }

        for i in 0..IOAPIC_NUM_PINS {
            self.irq_routes.push(create_irq_route_entry(
                i,
                kvm_bindings::KVM_IRQCHIP_IOAPIC,
                i,
            ));
            // This unwrap() will never fail, it is safe.
            self.gsi_bitmap.set(i as usize).unwrap();
        }
    }

    // 在计算机系统中，有多种类型的中断（Interrupt）。以下是其中的一些：
    //
    // 1. 硬件中断（Hardware Interrupt）：这种中断是由硬件设备发送给CPU的，通常是因为设备需要CPU的注意，比如数据已经准备好并可以被处理了。
    //
    // 2. 软件中断（Software Interrupt）：这种中断是由软件产生的，通常是因为程序需要操作系统的服务。
    //
    // 3. 非屏蔽中断（Non-maskable Interrupt，NMI）：这种中断不能被屏蔽（ignore）或者关闭，通常是用于处理一些严重的硬件错误。
    //
    // 4. 中断请求（Interrupt Request，IRQ）：这是一种特殊类型的硬件中断，通常由外部设备发送给CPU。
    //
    // 5. 消息信号中断（Message Signaled Interrupts，MSI）：这是一种更高级的硬件中断，允许设备发送更复杂的中断信息给CPU。MSI中断可以直接指向特定的CPU，从而提高中断处理的效率。
    //
    // 这些中断类型不是相互排斥的，而是可以同时存在和工作的。比如，一个设备可以同时发送IRQ和MSI中断，操作系统和CPU会决定如何处理这些中断。

    pub fn add_msi_route(&mut self, gsi: u32, msi_vector: MsiVector) -> Result<()> {
        let mut kroute = IrqRouteEntry {
            gsi,
            type_: KVM_IRQ_ROUTING_MSI,
            flags: 0,
            pad: 0,
            ..Default::default()
        };
        unsafe {kroute.u.msi.address_lo = msi_vector.msg_addr_lo;
        kroute.u.msi.address_hi = msi_vector.msg_addr_hi;
        kroute.u.msi.data = msi_vector.msg_data;
        }
        self.irq_routes.push(kroute);
        Ok(())
    }
    fn remove_irq_route(&mut self, gsi: u32) {
        while let Some((index, _)) = self
            .irq_routes
            .iter()
            .enumerate()
            .find(|(_, e)| e.gsi == gsi)
        {
            self.irq_routes.remove(index);
        }
    }

    /// Update msi irq route to irq routing table.
    pub fn update_msi_route(&mut self, gsi: u32, msi_vector: MsiVector) -> Result<()> {
        self.remove_irq_route(gsi);
        self.add_msi_route(gsi, msi_vector)
            .with_context(|| "Failed to add msi route")?;

        Ok(())
    }

    pub fn allocate_gsi(&mut self) -> Result<u32> {
        let free_gsi = self.gsi_bitmap.find_next_zero(0).with_context(|| "Failed to get new free gsi")?;
        self.gsi_bitmap.set(free_gsi)?;
        Ok(free_gsi as u32)
    }
    pub fn release_gsi(&mut self, gsi: u32) -> Result<()> {
        self.gsi_bitmap
            .clear(gsi as usize)
            .with_context(|| "Failed to release gsi")?;
        self.remove_irq_route(gsi);
        Ok(())
    }

    /// Get `IrqRouteEntry` by given gsi number.
    /// A gsi number may have several entries. If no gsi number in table, is will
    /// return an empty vector.
    pub fn get_irq_route_entry(&self, gsi: u32) -> Vec<IrqRouteEntry> {
        let mut entries = Vec::new();
        for entry in self.irq_routes.iter() {
            if gsi == entry.gsi {
                entries.push(*entry);
            }
        }

        entries
    }
}

/// Basic data for msi vector.
/// `kvm_irq_routing_entry__bindgen_ty_1`的msi结构是用来描述PCI设备的Message Signaled Interrupts (MSI)路由的信息。
//
// 具体来说，这个结构包含了以下几个重要的字段：
//
// - `address_lo`和`address_hi`：这两个字段组合在一起表示了MSI消息的目标地址。对于x86架构的系统来说，这个地址通常是本地APIC（高级可编程中断控制器）的地址。
//
// - `data`：这个字段表示了MSI消息的数据，它包含了中断向量和目标CPU的信息。
//
// - `flags`：这个字段表示了一些标志位，比如中断触发模式（边沿触发或者电平触发）和中断传递模式（物理模式或者逻辑模式）。
//
// 这个msi结构实际上是对PCI规范中MSI功能的一个抽象。通过这个结构，KVM可以管理虚拟机中的PCI设备的MSI中断，包括中断的路由和传递。
#[derive(Copy, Clone, Default)]
pub struct MsiVector {
    pub msg_addr_lo: u32,
    pub msg_addr_hi: u32,
    pub msg_data: u32,
    pub masked: bool,
    #[cfg(target_arch = "aarch64")]
    pub dev_id: u32,
}

// #[cfg(test)]
// mod tests {
//     use super::super::KVMFds;
//     use super::get_maximum_gsi_cnt;
//
//     #[test]
//     fn test_get_maximum_gsi_cnt() {
//         let kvm_fds = KVMFds::new();
//         if kvm_fds.vm_fd.is_none() {
//             return;
//         }
//         assert!(get_maximum_gsi_cnt(kvm_fds.fd.as_ref().unwrap()) > 0);
//     }
//
//     #[test]
//     fn test_alloc_and_release_gsi() {
//         let kvm_fds = KVMFds::new();
//         if kvm_fds.vm_fd.is_none() {
//             return;
//         }
//         let mut irq_route_table = kvm_fds.irq_route_table.lock().unwrap();
//         irq_route_table.init_irq_route_table();
//
//         #[cfg(target_arch = "x86_64")]
//         {
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 24);
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 25);
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 26);
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 27);
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 28);
//             assert!(irq_route_table.release_gsi(26).is_ok());
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 26);
//         }
//         #[cfg(target_arch = "aarch64")]
//         {
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 192);
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 193);
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 194);
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 195);
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 196);
//             assert!(irq_route_table.release_gsi(195).is_ok());
//             assert_eq!(irq_route_table.allocate_gsi().unwrap(), 195);
//         }
//     }
// }
