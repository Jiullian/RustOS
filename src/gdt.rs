use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor};
use x86_64::structures::gdt::SegmentSelector;
use lazy_static::lazy_static;

//index de la pile dans l'IST, utilisé pour le double fault handler
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;


lazy_static! {
    //le TSS contient l'IST
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            //taille de la pile de secours
            const STACK_SIZE: usize = 4096 * 5;
            //la pile doit obligatoirement être en read-write, sinon le bootloader la met en read-only
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };

    //la GDT est necessaire pour charger le TSS dans le cpu
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, Selectors { code_selector, tss_selector })
    };
}

//on stock les selectors retourné lors de l'ajout des entrées au GDT, necessaire pour mettre a jour le cpu.
struct Selectors {
    code_selector: SegmentSelector,
    tss_selector : SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::tables::load_tss;
    use x86_64::instructions::segmentation::{CS, Segment};

    //charge la GDT dans le cpu
    GDT.0.load();
    unsafe {
        //update du registre de segment de code pour pointer vers le nouveau GDT
        CS::set_reg(GDT.1.code_selector);
        //charge le tss pour que le cpu accéde aux piles de secours de l'IST
        load_tss(GDT.1.tss_selector);
    }
}
