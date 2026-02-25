#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
pub mod serial;
pub mod vga_buffer;
pub mod interrupts;

//trait Testable pour l'automatisation des affichages dans les tests
pub trait Testable {
    fn run(&self) -> ();
}

//implémentation de l'automisation pour tous les types T qui implémente le trait Fn()
impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

// Un custom framework pour les tests du kernel
pub fn test_runner(tests: &[&dyn Testable]) { // <- &[&dyn Fn()] est un slice de "Trait Objects" de la function Fn() trait
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

// Appel panic pour les tests
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

//type pour fermer QEMU dans le code
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

//fonction d'initalisation de la IDT dans le processeur
pub fn init() {
    interrupts::init_idt();
}

//Point d'entrée pour cargo test
//on s'assurer d'initialiser la IDT pour que les tests ne crashent pas
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    init();
    test_main();
    loop {}
}
