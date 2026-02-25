use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::println;
use lazy_static::lazy_static;

//fonction pour créer IDT avec une entrée pour le breakpoint
lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

//ajout d'un breakpoint pour le debug
extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    println!("!EXCEPTION: BREAKPOINT!\n{:#?}", stack_frame);
}

//set up un call d'instruction pour les tests
#[test_case]
fn test_breakpoint_exception() {
    x86_64::instructions::interrupts::int3();
}