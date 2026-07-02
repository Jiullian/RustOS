use crate::gdt;
use crate::hlt_loop;
use crate::print;
use crate::println;
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::structures::idt::PageFaultErrorCode;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

//gestionnaire exception page fault

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // print!("."); // Désactivé pour éviter de polluer le terminal
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

//Support saisie clavier
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use crate::printfunc::verif_message;
    use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    lazy_static! {
        // Clavier AZERTY
        static ref KEYBOARD: Mutex<Keyboard<layouts::Azerty, ScancodeSet1>> =
            Mutex::new(Keyboard::new(ScancodeSet1::new(),
                layouts::Azerty, HandleControl::Ignore)
            );
    }

    // Définition du tampon qui stockera les entrées utilisateur
    lazy_static! {
        static ref INPUT_TEXT: Mutex<[u8; 256]> = Mutex::new([0u8; 256]);
        static ref INPUT_LEN: Mutex<usize> = Mutex::new(0);
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() };
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                // Touches de caractères normales
                DecodedKey::Unicode(character) => {
                    // Gestion du retour arrière (Backspace)
                    if character == '\x08' {
                        let mut input_len = INPUT_LEN.lock();
                        if *input_len > 0 {
                            *input_len -= 1;
                            // On efface le caractère à l'écran
                            print!("{}", character);
                        }
                    }
                    // Gestion de la tabulation (Autocomplétion)
                    else if character == '\t' {
                        let mut input_text = INPUT_TEXT.lock();
                        let mut input_len = INPUT_LEN.lock();

                        // Conversion du tampon de saisie actuel en chaîne
                        if let Ok(input_str) = core::str::from_utf8(&input_text[..*input_len]) {
                            // On extrait le dernier mot saisi (le nom de fichier commencé)
                            let prefixe = if let Some(space_idx) = input_str.rfind(' ') {
                                &input_str[space_idx + 1..]
                            } else {
                                input_str
                            };

                            if !prefixe.is_empty() {
                                let mut completion_buf = [0u8; 12];
                                 // Recherche d'une unique correspondance
                                if let Some(longueur) = crate::fat::completer_nom(prefixe, &mut completion_buf) {
                                    if let Ok(nom_complet) = core::str::from_utf8(&completion_buf[..longueur]) {
                                        // 1. Effacer le préfixe du buffer RAM et de l'écran VGA
                                        for _ in 0..prefixe.len() {
                                            if *input_len > 0 {
                                                *input_len -= 1;
                                                print!("\x08"); // Envoie un retour arrière pour effacer à l'écran
                                            }
                                        }

                                        // 2. Écrire le nom complet (avec la bonne casse du disque) en RAM et à l'écran
                                        for &byte in nom_complet.as_bytes() {
                                            if *input_len < input_text.len() {
                                                input_text[*input_len] = byte;
                                                *input_len += 1;
                                                print!("{}", byte as char);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Si ce n'est pas Entrée, Retour arrière ni Tabulation, on stocke le caractère normal
                    else if character != '\n' {
                        let mut input_text = INPUT_TEXT.lock();
                        let mut input_len = INPUT_LEN.lock();

                        if *input_len < input_text.len() {
                            input_text[*input_len] = character as u8;
                            *input_len += 1;
                            // Affichage du caractère à l'écran
                            print!("{}", character);
                        }
                    }

                    // Si c'est Entrée, on valide le message saisi
                    if character == '\n' {
                        // Affichage du retour à la ligne
                        print!("{}", character);

                        let input_text = INPUT_TEXT.lock();
                        let input_str =
                            core::str::from_utf8(&input_text[..*INPUT_LEN.lock()]).unwrap_or("");

                        // Envoi au processeur de commande du Shell
                        verif_message(input_str);

                        // Réinitialisation de la longueur du tampon pour la prochaine commande
                        *INPUT_LEN.lock() = 0;
                    }
                }
                // Touches spéciales/brutes (Shift, Ctrl, flèches, etc.) -> on ne fait rien
                DecodedKey::RawKey(_key) => {}
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

//fonction pour créer IDT avec une entrée pour le breakpoint
lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
            .set_handler_fn(double_fault_handler)
            //pile séparer pour les double faults
            //pour que le handler fonctionne même si la pile principal du kernel déborde
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);

        }
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

//fonction pour gérer les breakpoints
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("!EXCEPTION: BREAKPOINT!\n{:#?}", stack_frame);
}

//appelé par le cpu quand une exception survient et que le handler échoue. C'est un handler de dernier recours.
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! //retourne ! car aucune reprise n'est possible après un double fault
{
    panic!("EXCEPTION DOUBLE_FAULT\n{:#?}", stack_frame);
}

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

//set up un call d'instruction pour les tests
#[test_case]
fn test_breakpoint_exception() {
    x86_64::instructions::interrupts::int3();
}
