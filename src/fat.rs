//! Module de gestion du système de fichiers FAT32
//!
//! Ce module permet de manipuler les fichiers et dossiers stockés
//! sur l'image disque FAT32 chargée en mémoire (RAM Disk).
#![allow(static_mut_refs)]

use crate::disk::{
    DISK_IMAGE, calculate_cluster_offset, calculate_offsets, find_free_cluster, link_clusters,
    mark_cluster_as_end,
};
use crate::{print, println};

/*
hexa	Signification
---------------------
0x01	Lecture seule
0x02	Caché
0x04	Système
0x08	Label du volume
0x10	Répertoire (dossier)
0x20	fichier normal
*/

/// Structure représentant une entrée de répertoire FAT32.
///
/// Chaque entrée fait 32 octets dans le répertoire racine.
/// Elle contient le nom du fichier/dossier (11 octets : 8 pour le nom + 3 pour l'extension)
/// et ses attributs (1 octet : lecture seule, caché, système, répertoire, etc.).
#[repr(C, packed)]
pub struct DirectoryEntry {
    pub name: [u8; 11], // Nom du fichier ou dossier (format 8.3)
    pub attributes: u8, // Attributs du fichier (0x10 = dossier, 0x20 = fichier, etc.)
}

impl DirectoryEntry {
    /// Vérifie si l'entrée du répertoire est valide.
    ///
    /// Une entrée est invalide si :
    /// - Le premier octet est 0x00 (fin du répertoire)
    /// - Le premier octet est 0xE5 (entrée supprimée)
    /// - L'attribut est 0x0F (entrée de nom long)
    pub fn is_valid(&self) -> bool {
        self.name[0] != 0x00 && self.name[0] != 0xE5 && self.attributes != 0x0F
    }

    /// Récupère et formate le nom du fichier ou dossier.
    ///
    /// Le format FAT32 stocke les noms en "8.3" : 8 caractères pour le nom et 3 pour l'extension
    /// Cette fonction retire les espaces de remplissage et ajoute un point entre le nom et l'extension si celle-ci existe.
    ///
    /// # Arguments
    /// * `buffer` - Un tampon mutable où le nom formaté sera écrit.
    ///
    /// # Retour
    /// Un slice du tampon contenant le nom formaté (ex: "HELLO.TXT").
    pub fn get_name<'a>(&self, buffer: &'a mut [u8]) -> &'a [u8] {
        // Les 8 premiers octets sont le nom (sans les espaces de fin)
        let name = &self.name[0..8];
        // Les 3 derniers octets sont l'extension (sans les espaces de fin)
        let ext = &self.name[8..11];

        // On itère sur les caractères du nom en ignorant les espaces de fin
        let name = name.iter().take_while(|&&c| c != b' ').cloned();
        let ext = ext.iter().take_while(|&&c| c != b' ').cloned();

        // On remplit le buffer avec le nom
        let mut index = 0;
        for c in name {
            if index < buffer.len() {
                buffer[index] = c;
                index += 1;
            }
        }

        // Si l'extension existe, on ajoute un point puis l'extension
        if ext.clone().count() > 0 {
            if index < buffer.len() {
                buffer[index] = b'.';
                index += 1;
            }
            for c in ext {
                if index < buffer.len() {
                    buffer[index] = c;
                    index += 1;
                }
            }
        }

        // On retourne seulement la partie remplie du buffer
        &buffer[..index]
    }

    /// Vérifie si l'entrée correspond à un dossier.
    ///
    /// Le bit 4 (0x10) de l'attribut indique que c'est un répertoire.
    pub fn is_directory(&self) -> bool {
        self.attributes & 0x10 != 0
    }
}

/// Affiche la liste des fichiers et dossiers présents dans le répertoire racine.
///
/// Cette fonction parcourt toutes les entrées du répertoire racine (chacune fait 32 octets)
/// et affiche le nom de chaque fichier ou dossier valide.
///
/// Le répertoire racine se trouve juste après la table FAT dans l'image disque.
/// On calcule sa position grâce à `calculate_offsets()` puis on lit chaque entrée
/// de 32 octets jusqu'à la fin du cluster.
pub fn ls() {
    unsafe {
        // Récupération de l'offset du répertoire racine (on ignore fat_offset et data_offset ici)
        let root_directory_offset = calculate_offsets().1;

        // Lecture des paramètres du disque pour calculer la taille d'un cluster
        let bytes_per_sector = u16::from_le_bytes([DISK_IMAGE[11], DISK_IMAGE[12]]) as usize;
        let sectors_per_cluster = DISK_IMAGE[13] as usize;
        let cluster_size = bytes_per_sector * sectors_per_cluster;

        // Extraction du répertoire racine depuis l'image disque
        let end_of_root_directory = root_directory_offset + cluster_size;
        let root_directory = &DISK_IMAGE[root_directory_offset..end_of_root_directory];

        // Tampon pour stocker le nom formaté (8 caractères + 1 point + 3 extension = 12 max)
        let mut name_buffer = [0u8; 12];

        // Parcours de chaque entrée de 32 octets dans le répertoire racine
        for i in (0..cluster_size).step_by(32) {
            // Vérification qu'on ne dépasse pas la taille du cluster
            if i + 32 > cluster_size {
                break;
            }

            // Conversion du bloc de 32 octets en structure DirectoryEntry
            // On prend un pointeur brut vers le début de l'entrée et on le cast
            let entry: &DirectoryEntry =
                &*(root_directory[i..i + 32].as_ptr() as *const DirectoryEntry);

            // Si l'entrée est valide, on affiche son nom avec son type
            if entry.is_valid() {
                let name = entry.get_name(&mut name_buffer);
                if entry.is_directory() {
                    println!(
                        "Dossier : {}",
                        core::str::from_utf8(name).unwrap_or("Nom invalide")
                    );
                } else {
                    println!(
                        "Fichier : {}",
                        core::str::from_utf8(name).unwrap_or("Nom invalide")
                    );
                }
            }
        }
    }
}
