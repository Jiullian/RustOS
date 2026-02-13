use volatile::Volatile;
use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;

// Couleur standard du mode texte VGA
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

// Structure représentant un code couleur complet (fond + premier plan)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

// Structure représentant un caractère à l'écran : code ASCII + code couleur
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

// Dimensions du buffer VGA standard
const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

// Structure représentant le buffer VGA entier
#[repr(transparent)]
struct Buffer {
    // Volatile est utilisé pour empêcher le compilateur d'optimiser les écritures
    // car nous écrivons dans une zone mémoire mappée au matériel (MMIO) sans ca par optimisation il pourrait supprimer ce qu'on veut écrire
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

// Le Writer gère l'écriture dans le buffer VGA
pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    // Écrit un octet (caractère ASCII) dans le buffer
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(), // Gestion du saut de ligne
            byte => {
                // Si on dépasse la largeur, on passe à la ligne suivante
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                // Utilisation de .write() de Volatile pour garantir l'écriture mémoire
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        }
    }

    // Écrit une chaîne de caractères complète
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // Caractère ASCII imprimable ou saut de ligne
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // Caractère non imprimable : on affiche un carré (0xfe)
                _ => self.write_byte(0xfe),
            }
        }
    }

    // Déplace toutes les lignes vers le haut et efface la dernière ligne
    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    // Efface une ligne entière en la remplissant d'espaces
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}

// Implémentation du trait fmt::Write pour supporter les macros de formatage Rust (write!, writeln!...)
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// Création d'une instance globale statique du Writer
// lazy_static : Permet de retarder l'initialisation jusqu'au PREMIER besoin (runtime).
//               Comme le "Writer" a besoin de code complexe pour se créer, on ne peut pas le faire à la compilation.
// Mutex (Spinlock) : Cela empêche plusieurs parties du code d'écrire en même temps et de tout mélanger.
lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        // unsafe : On force la serrure. On dit à Rust "T'inquiète, je sais que 0xb8000 est l'adresse magique de l'écran".
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

// Définition des macros print! et println!
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

// Fonction privée appelée par les macros, cache les détails d'implémentation
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    // On verrouille le mutex pour avoir un accès exclusif au Writer
    // .unwrap() panique si l'écriture échoue (ce qui ne devrait pas arriver ici)
    WRITER.lock().write_fmt(args).unwrap();
}