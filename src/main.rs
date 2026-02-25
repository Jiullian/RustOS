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
    x86_64::instructions::interrupts::int3();

    #[cfg(test)]
    test_main();

    println!("No crash! ;)");
    loop {}
}

// Fonction appelée lors de chaque panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("ERROR ---> {}", info);
    loop {}
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