use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::println;
use lazy_static::lazy_static;
use crate::gdt;

//fonction pour créer IDT avec une entrée pour le breakpoint
lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
            .set_handler_fn(double_fault_handler)
            //pile séparer pour les double faults
            //pour que le handler fonctionne même si la pile principal du kernel déborde
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);

        }

        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

//fonction pour gérer les breakpoints
extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    println!("!EXCEPTION: BREAKPOINT!\n{:#?}", stack_frame);
}


//appelé par le cpu quand une exception survient et que le handler échoue. C'est un handler de dernier recours.
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64) -> ! //retourne ! car aucune reprise n'est possible après un double fault
{
    panic!("EXCEPTION DOUBLE_FAULT\n{:#?}", stack_frame);
}



//set up un call d'instruction pour les tests
#[test_case]
fn test_breakpoint_exception() {
    x86_64::instructions::interrupts::int3();
}