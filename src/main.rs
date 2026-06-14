#![no_std] // Ne pas utiliser les bibliothèques standard de Rust
#![no_main] // Désactiver les points d'entrées standard de Rust
// Attribut de test
#![feature(custom_test_frameworks)]
#![test_runner(RustOS::test_runner)]
#![reexport_test_harness_main = "test_main"]

use RustOS::println;
use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use x86_64::structures::paging::PageTable;
// Création d'une variable static
// b"Hello World!" => b pour créer une chaîne de caractères d'octets. VGA ne comprend que l'ASCII ET non l'UNICODE
static HELLO: &[u8] = b"Hello World!";

entry_point!(kernel_main);

// Fonction point d'entrée du système.
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use RustOS::memory;
    use x86_64::{
        VirtAddr,
        structures::paging::{Page, Translate},
    };

    println!("Hello World{}", "!");
    RustOS::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::init(&boot_info.memory_map) };

    // pointage factice d'une page virtuelle libre (très très éloignée)
    let page = Page::containing_address(VirtAddr::new(0xdeadbeaf000));
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    //  on écrit `New!` (en little endian) et on l'affiche
    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e) };

    #[cfg(test)]
    test_main();

    println!("No crash! ;)");
    RustOS::hlt_loop();
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
