//! Module de gestion du système de fichiers FAT32
//!
//! Manipulation des fichiers et dossiers stockés sur l'image disque FAT32 en mémoire.
#![allow(static_mut_refs)]

use crate::disk::{
    DISK_IMAGE, calculate_cluster_offset, calculate_offsets, find_free_cluster, link_clusters,
    mark_cluster_as_end,
};
use crate::vga_buffer::{Color, reset_color, set_color};
use crate::{print, println};

// Table des attributs FAT32 (chaque entrée a un octet d'attribut) :
// 0x01 = Lecture seule | 0x02 = Caché | 0x04 = Système
// 0x08 = Label du volume | 0x10 = Dossier | 0x20 = Fichier normal

/// Structure qui représente une entrée dans le répertoire FAT32.
/// Chaque entrée fait 32 octets sur le disque, mais on ne lit ici que les 12 premiers :
/// - 11 octets pour le nom au format "8.3" (ex: "HELLO   TXT" = "HELLO.TXT")
/// - 1 octet pour les attributs (permet de savoir si c'est un fichier ou un dossier)
#[repr(C, packed)]
pub struct DirectoryEntry {
    pub name: [u8; 11],
    pub attributes: u8,
}

impl DirectoryEntry {
    /// Vérifie si l'entrée est valide (utilisable).
    /// On ignore les entrées vides (0x00), supprimées (0xE5) et les noms longs (0x0F).
    pub fn is_valid(&self) -> bool {
        self.name[0] != 0x00 && self.name[0] != 0xE5 && self.attributes != 0x0F
    }

    /// Convertit le nom brut "8.3" en nom lisible.
    /// Exemple : les 11 octets "HELLO   TXT" deviennent la chaîne "HELLO.TXT"
    /// On retire les espaces de remplissage et on ajoute un point avant l'extension.
    pub fn get_name<'a>(&self, buf: &'a mut [u8]) -> &'a str {
        let mut i = 0;

        // Copie des 8 premiers caractères du nom (on s'arrête aux espaces)
        for &c in self.name[0..8].iter().take_while(|&&c| c != b' ') {
            buf[i] = c;
            i += 1;
        }

        // Si une extension existe (les 3 derniers octets ne sont pas vides),
        // on ajoute un point puis les caractères de l'extension
        let ext: &[u8] = &self.name[8..11];
        if ext[0] != b' ' {
            buf[i] = b'.';
            i += 1;
            for &c in ext.iter().take_while(|&&c| c != b' ') {
                buf[i] = c;
                i += 1;
            }
        }

        // Conversion des octets en chaîne de caractères UTF-8
        core::str::from_utf8(&buf[..i]).unwrap_or("?")
    }

    /// Vérifie si l'entrée est un dossier en testant le bit 0x10 de l'attribut.
    pub fn is_directory(&self) -> bool {
        self.attributes & 0x10 != 0
    }
}

/// Commande LS : affiche le contenu du répertoire racine de l'image disque.
/// Les dossiers sont affichés en bleu et les fichiers en blanc
pub fn ls() {
    unsafe {
        // On récupère l'offset du répertoire racine dans l'image disque
        let root_offset = calculate_offsets().1;

        // On calcule la taille d'un cluster (= nombre de secteurs × taille d'un secteur)
        let bps = u16::from_le_bytes([DISK_IMAGE[11], DISK_IMAGE[12]]) as usize;
        let cluster_size = bps * DISK_IMAGE[13] as usize;

        // On extrait le répertoire racine (un cluster complet)
        let root = &DISK_IMAGE[root_offset..root_offset + cluster_size];

        let mut buf = [0u8; 12]; // Tampon pour formater le nom (12 caractères max)
        let mut col: usize = 0; // Position actuelle sur la ligne (pour l'alignement)

        // On parcourt le répertoire racine entrée par entrée (chaque entrée = 32 octets)
        for i in (0..cluster_size).step_by(32) {
            // On cast les 32 octets bruts en notre structure DirectoryEntry
            let entry: &DirectoryEntry = &*(root[i..].as_ptr() as *const DirectoryEntry);

            if entry.is_valid() {
                let name = entry.get_name(&mut buf);

                // Si le nom ne tient plus sur la ligne (80 colonnes VGA), on passe à la suivante
                if col + 14 > 80 {
                    println!();
                    col = 0;
                }

                // Affichage coloré : bleu pour les dossiers, blanc pour les fichiers
                if entry.is_directory() {
                    set_color(Color::LightBlue, Color::Black);
                    print!("{}", name);
                    reset_color();
                } else {
                    print!("{}", name);
                }

                // On remplit avec des espaces pour aligner sur des colonnes de 14 caractères
                for _ in 0..(14 - name.len()) {
                    print!(" ");
                }
                col += 14;
            }
        }

        // Retour à la ligne final après le dernier élément affiché
        if col > 0 {
            println!();
        }
    }
}
