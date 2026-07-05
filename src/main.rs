#![no_std] // Ne pas utiliser les bibliothèques standard de Rust
#![no_main] // Désactiver les points d'entrées standard de Rust
// Attribut de test
#![feature(custom_test_frameworks)]
#![test_runner(RustOS::test_runner)]
#![reexport_test_harness_main = "test_main"]

use RustOS::println;
use RustOS::task::{Task, executor::Executor, keyboard};
use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
extern crate alloc;

entry_point!(kernel_main);

// Fonction point d'entrée du système.
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use RustOS::allocator;
    use RustOS::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    // Initialisation du système (GDT, IDT, interrupts, disque, titre)
    RustOS::init();

    // Initialisation de la mémoire dynamique (heap)
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::init(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    #[cfg(test)]
    test_main();

    let mut executor = Executor::new();
    executor.spawn(Task::new(keyboard::print_keypresses()));
    executor.run();
}

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

// Fonction appelée lors de chaque panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("ERROR ---> {}", info);
    RustOS::hlt_loop()
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    RustOS::test_panic_handler(info)
}

// test unitaire pour le framework
#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
