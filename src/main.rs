#![no_std] // Ne pas utiliser les bibliothèques standard de Rust
#![no_main] // Désactiver les points d'entrées standard de Rust

use core::panic::PanicInfo;

// Fonction point d'entrée du système.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {}
}

// Fonction appelée lors de chaque panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}