#![no_std] // Ne pas utiliser les bibliothèques standard de Rust
#![no_main] // Désactiver les points d'entrées standard de Rust
// Attribut de test
#![feature(custom_test_frameworks)]
#![test_runner(RustOS::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use RustOS::println;

// Création d'une variable static
// b"Hello World!" => b pour créer une chaîne de caractères d'octets. VGA ne comprend que l'ASCII ET non l'UNICODE
static HELLO: &[u8] = b"Hello World!";

// Fonction point d'entrée du système.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    RustOS::init();

    use x86_64::registers::control::Cr3;

    let (level_4_page_table, _) = Cr3::read();
    println!("Level 4 page table at: {:?}", level_4_page_table.start_address());

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