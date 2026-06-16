#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(RustOS::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use alloc::boxed::Box;
use alloc::vec::Vec;
use RustOS::allocator::HEAP_SIZE;

entry_point!(main);

// Test n°1 : Allouer des valeurs simples sur le tas.
#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
    // À la fin de cette fonction il y a "Drop" : la mémoire est libérée automatiquement.
}

// Test n°2 : Utiliser un vecteur qui grandit.
#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        // Le vecteur va demander plus de mémoire au fur et à mesure qu'il grandit.
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

// Test n°3 : Créer beaucoup de variables à la suite.
#[test_case]
fn many_boxes() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
        // Si l'allocateur ne faisait pas de dealloc, ce test provoquerait une erreur
        // Out of Memory bien avant la fin de la boucle.
    }
}
fn main(boot_info: &'static BootInfo) -> ! {
    use RustOS::allocator;
    use RustOS::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    RustOS::init();
    // Récupérer l'adresse où le bootloader a placé la mémoire physique.
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    // Initialiser le Mapper (pour traduire les adresses virtuelles en physiques).
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    // Initialiser l'allocateur de frames (qui trouve des blocs de RAM libres).
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    // Création de la mémoire dynamique.
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    // On exécute les tests.
    test_main();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    RustOS::test_panic_handler(info)
}
