#![no_std] // Ne pas utiliser les bibliothèques standard de Rust
#![no_main] // Désactiver les points d'entrées standard de Rust

use core::panic::PanicInfo;

// Création d'une variable static
// b"Hello World!" => b pour créer une chaîne de caractères d'octets. VGA ne comprend que l'ASCII ET non l'UNICODE
static HELLO: &[u8] = b"Hello World!";

// Fonction point d'entrée du système.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Adresse réservée au tampon texte VGA sur les systèmes x86
    // Caster en pointeur brut vers un octet pour pouvoir manipuler octet par octet
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            // Le buffer VGA fonctionne par paire d'octet
            // L'octet du caractère : son code ASCII (ex : 'H')
            *vga_buffer.offset(i as isize * 2) = byte;
            // L'octet de l'attribut : définition de la couleur et du fond
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb; // 0xb : bleu cyan
        }
    }

    loop {}
}

// Fonction appelée lors de chaque panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}