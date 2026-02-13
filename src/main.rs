#![no_std] // Ne pas utiliser les bibliothèques standard de Rust
#![no_main] // Désactiver les points d'entrées standard de Rust
mod vga_buffer;

use core::panic::PanicInfo;

// Création d'une variable static
// b"Hello World!" => b pour créer une chaîne de caractères d'octets. VGA ne comprend que l'ASCII ET non l'UNICODE
static HELLO: &[u8] = b"Hello World!";

// Fonction point d'entrée du système.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");
    panic!("Panicked just here");
    loop {}
}

// Fonction appelée lors de chaque panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("ERROR ---> {}", info);
    loop {}
}