extern crate alloc;

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

    // Définition du tampon (structure) qui stockera les entrées utilisateur et son historique (tas)
    lazy_static! {
        static ref INPUT_TEXT: Mutex<[u8; 256]> = Mutex::new([0u8; 256]);
        static ref INPUT_LEN: Mutex<usize> = Mutex::new(0);
        static ref HISTORY: Mutex<alloc::vec::Vec<alloc::string::String>> =
            Mutex::new(alloc::vec::Vec::new());
        static ref HISTORY_INDEX: Mutex<Option<usize>> = Mutex::new(None);
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
                        let mut completion_data: Option<(usize, [u8; 12])> = None;

                        {
                            let input_text = INPUT_TEXT.lock();
                            let input_len = INPUT_LEN.lock();

                            // Étape 1 : Analyse de la saisie utilisateur
                            // On convertit les octets saisis en chaîne de caractères UTF-8.
                            if let Ok(input_str) = core::str::from_utf8(&input_text[..*input_len]) {
                                // On extrait le dernier mot saisi en cherchant le dernier espace (' ').
                                let prefixe = if let Some(space_idx) = input_str.rfind(' ') {
                                    &input_str[space_idx + 1..]
                                } else {
                                    input_str
                                };

                                if !prefixe.is_empty() {
                                    let mut completion_buf = [0u8; 12];

                                    // Étape 2 : Recherche disque via fat::completer_nom
                                    // Cette fonction parcourt la racine, cherche les fichiers commençant par *prefixe* (insensible à la casse)
                                    // Si elle trouve UNE SEULE correspondance unique, elle renvoie sa longueur et remplit completion_buf
                                    // S'il y a 0 ou plusieurs fichiers (ex: "T1.TXT" et "T2.TXT" pour "T"), elle renvoie None
                                    if let Some(longueur) =
                                        crate::fat::completer_nom(prefixe, &mut completion_buf)
                                    {
                                        completion_data = Some((prefixe.len(), completion_buf));
                                    }
                                }
                            }
                        }

                        // Étape 3 : Écriture graphique et mémoire de la complétion
                        // Si une correspondance unique a été validée :
                        if let Some((prefix_len, completion_buf)) = completion_data {
                            let mut input_text = INPUT_TEXT.lock();
                            let mut input_len = INPUT_LEN.lock();

                            // On efface le préfixe saisi à l'écran en envoyant le caractère 'Retour arrière' (\x08)
                            // autant de fois que la longueur du préfixe, et on décrémente INPUT_LEN.
                            for _ in 0..prefix_len {
                                if *input_len > 0 {
                                    *input_len -= 1;
                                    print!("\x08"); // Efface un caractère à l'écran VGA
                                }
                            }

                            // On écrit le nom complet trouvé dans notre tampon de saisie RAM (INPUT_TEXT)
                            // et on l'affiche caractère par caractère sur le terminal VGA.
                            let mut completion_len = 0;
                            while completion_len < 12 && completion_buf[completion_len] != 0 {
                                let byte = completion_buf[completion_len];
                                if *input_len < input_text.len() {
                                    input_text[*input_len] = byte;
                                    *input_len += 1;
                                    print!("{}", byte as char);
                                }
                                completion_len += 1;
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

                        // Enregistrement de la commande non vide dans l'historique.
                        // APPEL À L'ALLOCATEUR GLOBAL (FixedSizeBlockAllocator) :
                        // 1. "trimmed.into()" : Convertit &str en String -> Allocation de la commande sur le tas (Heap).
                        // 2. "hist.push()" : Insère la String dans le Vec -> Réallocation potentielle du vecteur sur le tas.
                        let trimmed = input_str.trim();
                        if !trimmed.is_empty() {
                            let mut hist = HISTORY.lock();
                            // Évite de stocker deux commandes identiques de suite
                            if hist.is_empty() || hist.last().unwrap().as_str() != trimmed {
                                hist.push(trimmed.into());
                            }
                        }

                        // Réinitialisation de l'index de navigation dans l'historique
                        *HISTORY_INDEX.lock() = None;

                        // Envoi au processeur de commande du Shell
                        verif_message(input_str);

                        // Réinitialisation de la longueur du tampon pour la prochaine commande
                        *INPUT_LEN.lock() = 0;
                    }
                }
                // Touches spéciales/brutes (Shift, Ctrl, flèches, etc.)
                DecodedKey::RawKey(key) => {
                    match key {
                        pc_keyboard::KeyCode::ArrowUp => {
                            let hist = HISTORY.lock();
                            let mut hist_idx = HISTORY_INDEX.lock();

                            if !hist.is_empty() {
                                // Étape 1 : Calcul de l'index de navigation dans l'historique
                                // Si l'index est None (première pression), on commence par le plus récent : hist.len() - 1.
                                // Sinon, on remonte d'un niveau (idx - 1), avec un minimum à 0 (le plus ancien).
                                let target_idx = match *hist_idx {
                                    None => hist.len() - 1,
                                    Some(idx) => {
                                        if idx > 0 {
                                            idx - 1
                                        } else {
                                            0
                                        }
                                    }
                                };
                                *hist_idx = Some(target_idx);

                                let cmd = &hist[target_idx];
                                let mut input_text = INPUT_TEXT.lock();
                                let mut input_len = INPUT_LEN.lock();

                                // Étape 2 : Effacement de la saisie actuelle
                                // On envoie des codes retour arrière '\x08' pour vider la ligne à l'écran VGA.
                                for _ in 0..*input_len {
                                    print!("\x08");
                                }

                                // Étape 3 : Remplacement par la commande historique
                                // On réinitialise INPUT_LEN et on copie les caractères de la commande historique
                                // dans le tampon RAM de saisie (INPUT_TEXT) tout en les réaffichant à l'écran.
                                *input_len = 0;
                                for &byte in cmd.as_bytes() {
                                    if *input_len < input_text.len() {
                                        input_text[*input_len] = byte;
                                        *input_len += 1;
                                        print!("{}", byte as char);
                                    }
                                }
                            }
                        }
                        pc_keyboard::KeyCode::ArrowDown => {
                            let hist = HISTORY.lock();
                            let mut hist_idx = HISTORY_INDEX.lock();

                            if !hist.is_empty() {
                                // Étape 1 : Calcul de l'index de navigation
                                // La flèche du bas ne fait quelque chose que si on est déjà en train de parcourir l'historique (Some(idx)).
                                if let Some(idx) = *hist_idx {
                                    let mut input_text = INPUT_TEXT.lock();
                                    let mut input_len = INPUT_LEN.lock();

                                    // Effacement de la saisie actuelle à l'écran
                                    for _ in 0..*input_len {
                                        print!("\x08");
                                    }
                                    *input_len = 0;

                                    // Si on peut encore descendre vers une commande plus récente
                                    if idx + 1 < hist.len() {
                                        let target_idx = idx + 1;
                                        *hist_idx = Some(target_idx);
                                        let cmd = &hist[target_idx];

                                        // Copie de la commande et réaffichage à l'écran
                                        for &byte in cmd.as_bytes() {
                                            if *input_len < input_text.len() {
                                                input_text[*input_len] = byte;
                                                *input_len += 1;
                                                print!("{}", byte as char);
                                            }
                                        }
                                    } else {
                                        // Si on a dépassé la commande la plus récente :
                                        // On réinitialise l'index à None, laissant une ligne vide prête pour une nouvelle saisie.
                                        *hist_idx = None;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
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
