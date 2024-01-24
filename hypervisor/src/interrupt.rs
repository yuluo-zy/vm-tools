use kvm_bindings::KVM_IRQ_ROUTING_IRQCHIP;
use kvm_ioctls::{Cap, Kvm};
use kvm_ioctls::Cap::Irqchip;
use vmm_sys_util::ioctl_io_nr;
use utils::bitmap::Bitmap;

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

#[cfg(target_arch = "x86_64")]
const IOAPIC_NUM_PINS: u32 = 24;
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
    pub fn new()
}
#[cfg(test)]
mod tests {
    use super::super::KVMFds;
    use super::get_maximum_gsi_cnt;

    #[test]
    fn test_get_maximum_gsi_cnt() {
        let kvm_fds = KVMFds::new();
        if kvm_fds.vm_fd.is_none() {
            return;
        }
        assert!(get_maximum_gsi_cnt(kvm_fds.fd.as_ref().unwrap()) > 0);
    }

    #[test]
    fn test_alloc_and_release_gsi() {
        let kvm_fds = KVMFds::new();
        if kvm_fds.vm_fd.is_none() {
            return;
        }
        let mut irq_route_table = kvm_fds.irq_route_table.lock().unwrap();
        irq_route_table.init_irq_route_table();

        #[cfg(target_arch = "x86_64")]
        {
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 24);
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 25);
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 26);
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 27);
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 28);
            assert!(irq_route_table.release_gsi(26).is_ok());
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 26);
        }
        #[cfg(target_arch = "aarch64")]
        {
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 192);
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 193);
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 194);
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 195);
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 196);
            assert!(irq_route_table.release_gsi(195).is_ok());
            assert_eq!(irq_route_table.allocate_gsi().unwrap(), 195);
        }
    }
}
