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

//----------------------------------------------------------Fonctions d'aide (Helpers)----------------------------------------------------------

/// Formate un nom de fichier/dossier classique vers le format binaire standard 8.3 de FAT.
///
/// Si le nom comporte un point (ex: "FICHIER.TXT"), on valide que le nom a 8 caractères max
/// et l'extension 3 caractères max.
/// Si aucun point n'est présent (dossier ou fichier sans extension), le nom doit faire 8 caractères max.
/// Le tableau retourné est complété par des espaces (0x20).
fn formater_nom_8_3(nom: &str) -> Option<[u8; 11]> {
    let mut formatted = [b' '; 11];

    // On sépare le nom et l'extension s'il y a un point
    if let Some((n, e)) = nom.split_once('.') {
        if n.len() > 8 || e.len() > 3 {
            return None; // Trop long
        }
        formatted[..n.len()].copy_from_slice(n.as_bytes());
        formatted[8..8 + e.len()].copy_from_slice(e.as_bytes());
    } else {
        // Nom simple sans extension
        if nom.len() > 8 {
            return None; // Trop long
        }
        formatted[..nom.len()].copy_from_slice(nom.as_bytes());
    }
    Some(formatted)
}

/// Recherche un emplacement libre (une entrée vide 0x00 ou supprimée 0xE5) dans un répertoire.
/// Chaque entrée de répertoire faisant 32 octets, on avance de 32 en 32.
fn trouver_entree_libre(repertoire: &[u8], taille_cluster: usize) -> Option<usize> {
    for i in (0..taille_cluster).step_by(32) {
        if repertoire[i] == 0x00 || repertoire[i] == 0xE5 {
            return Some(i);
        }
    }
    None
}

/// Calcule et retourne la taille d'un cluster sur le disque (en octets).
fn obtenir_taille_cluster() -> usize {
    unsafe {
        let bps = u16::from_le_bytes([DISK_IMAGE[11], DISK_IMAGE[12]]) as usize;
        let sectors_per_cluster = DISK_IMAGE[13] as usize;
        bps * sectors_per_cluster
    }
}

/// Retourne une référence immuable sur le répertoire racine du disque.
fn obtenir_repertoire_racine(cluster_size: usize) -> &'static [u8] {
    unsafe {
        let root_offset = calculate_offsets().1;
        &DISK_IMAGE[root_offset..root_offset + cluster_size]
    }
}

/// Retourne une référence mutable sur le répertoire racine du disque.
fn obtenir_repertoire_racine_mut(cluster_size: usize) -> &'static mut [u8] {
    unsafe {
        let root_offset = calculate_offsets().1;
        &mut DISK_IMAGE[root_offset..root_offset + cluster_size]
    }
}

/// Écrit de manière générique une structure DirectoryEntry de 32 octets dans un répertoire.
fn ecrire_entree_repertoire(
    repertoire: &mut [u8],
    offset: usize,
    nom: &[u8; 11],
    attribut: u8,
    premier_cluster: u32,
    taille_fichier: u32,
) {
    let entry = &mut repertoire[offset..offset + 32];
    entry[0..11].copy_from_slice(nom); // Nom formaté 8.3
    entry[11] = attribut; // Attribut (dossier ou fichier)
    entry[12..20].fill(0); // Réinitialisation des temps/dates par défaut
    entry[20..22].copy_from_slice(&((premier_cluster >> 16) as u16).to_le_bytes()); // Cluster partie haute
    entry[22..26].fill(0); // Date/Heure écriture à 0
    entry[26..28].copy_from_slice(&(premier_cluster as u16).to_le_bytes()); // Cluster partie basse
    entry[28..32].copy_from_slice(&taille_fichier.to_le_bytes()); // Taille en octets
}

//----------------------------------------------------------Structure Entree Repertoire----------------------------------------------------------

/// Structure qui représente une entrée dans le répertoire FAT32.
/// Chaque entrée fait exactement 32 octets sur le disque.
#[derive(Clone, Copy)]
#[allow(dead_code)]
#[repr(C, packed)]
pub struct DirectoryEntry {
    pub name: [u8; 11],
    pub attributes: u8,
    pub nt_res: u8,
    pub creation_time_tenths: u8,
    pub creation_time: u16,
    pub creation_date: u16,
    pub last_access_date: u16,
    pub first_cluster_high: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_low: u16,
    pub file_size: u32,
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
        // Pour éviter les warnings de référence non alignée sur une structure packed,
        // on copie le champ name dans une variable locale sur la pile.
        let name_field = self.name;
        let mut i = 0;

        // Copie des 8 premiers caractères du nom (on s'arrête aux espaces)
        for &c in name_field[0..8].iter().take_while(|&&c| c != b' ') {
            buf[i] = c;
            i += 1;
        }

        // Si une extension existe (les 3 derniers octets ne sont pas vides),
        // on ajoute un point puis les caractères de l'extension
        let ext = &name_field[8..11];
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

    /// Récupère le premier cluster du fichier/dossier.
    pub fn first_cluster(&self) -> u32 {
        ((self.first_cluster_high as u32) << 16) | (self.first_cluster_low as u32)
    }

    /// Récupère la taille du fichier
    pub fn file_size(&self) -> usize {
        self.file_size as usize
    }
}

//----------------------------------------------------------Commande LS----------------------------------------------------------

/// Commande LS : affiche le contenu du répertoire racine de la FAT32.
/// Les dossiers sont affichés en bleu et les fichiers en blanc.
pub fn ls() {
    unsafe {
        let cluster_size = obtenir_taille_cluster();
        let root = obtenir_repertoire_racine(cluster_size);

        let mut buf = [0u8; 12];
        let mut col: usize = 0;

        // Parcours de chaque entrée de 32 octets dans le répertoire
        for i in (0..cluster_size).step_by(32) {
            let entry: &DirectoryEntry = &*(root[i..].as_ptr() as *const DirectoryEntry);

            if entry.is_valid() {
                let name = entry.get_name(&mut buf);

                // Si le nom ne tient plus sur la ligne (80 col max), on saute une ligne
                if col + 14 > 80 {
                    println!();
                    col = 0;
                }

                // Si c'est un dossier, on l'affiche en Bleu Clair
                if entry.is_directory() {
                    set_color(Color::LightBlue, Color::Black);
                    print!("{}", name);
                    reset_color();
                } else {
                    // Sinon en Blanc
                    print!("{}", name);
                }

                // Alignement horizontal : chaque colonne fait 14 caractères
                for _ in 0..(14 - name.len()) {
                    print!(" ");
                }
                col += 14;
            }
        }

        if col > 0 {
            println!();
        }
    }
}

//----------------------------------------------------------Commande CAT----------------------------------------------------------

/// Commande CAT : lit et affiche à l'écran le contenu d'un fichier texte présent dans la racine.
pub fn cat(file_name: &str) {
    unsafe {
        let (fat_offset, _, data_offset) = calculate_offsets();
        let cluster_size = obtenir_taille_cluster();
        let root = obtenir_repertoire_racine(cluster_size);

        let mut buf = [0u8; 12];

        // Recherche séquentielle du fichier dans le répertoire racine
        for i in (0..cluster_size).step_by(32) {
            let entry: &DirectoryEntry = &*(root[i..].as_ptr() as *const DirectoryEntry);

            if entry.is_valid() && !entry.is_directory() {
                let name = entry.get_name(&mut buf);

                if name == file_name {
                    let mut current_cluster = entry.first_cluster();
                    let mut remaining_size = entry.file_size();

                    // Boucle de lecture des clusters chaînés du fichier
                    while current_cluster < 0x0FFFFFF8 {
                        let offset = calculate_cluster_offset(data_offset, current_cluster);
                        let cluster_data = &DISK_IMAGE[offset..offset + cluster_size];

                        // Quantité de données à lire dans ce cluster
                        let to_read = core::cmp::min(remaining_size, cluster_size);
                        let content = &cluster_data[..to_read];

                        if let Ok(text) = core::str::from_utf8(content) {
                            print!("{}", text);
                        } else {
                            println!(
                                "\nErreur : Le fichier contient des donnees non lisibles en UTF-8."
                            );
                            return;
                        }

                        remaining_size -= to_read;
                        if remaining_size == 0 {
                            break;
                        }

                        let fat_entry_offset = fat_offset + (current_cluster as usize * 4);
                        current_cluster = u32::from_le_bytes([
                            DISK_IMAGE[fat_entry_offset],
                            DISK_IMAGE[fat_entry_offset + 1],
                            DISK_IMAGE[fat_entry_offset + 2],
                            DISK_IMAGE[fat_entry_offset + 3],
                        ]) & 0x0FFFFFFF;
                    }
                    println!();
                    return;
                }
            }
        }
        println!("Fichier introuvable : '{}'", file_name);
    }
}

//----------------------------------------------------------Completion Automatique----------------------------------------------------------

/// Cherche un fichier commençant par `prefixe` dans le répertoire racine.
/// Si un unique fichier correspond, écrit son nom complet dans `nom_complet_out` et retourne sa longueur.
pub fn completer_nom(prefixe: &str, nom_complet_out: &mut [u8]) -> Option<usize> {
    unsafe {
        let cluster_size = obtenir_taille_cluster();
        let root = obtenir_repertoire_racine(cluster_size);

        let mut correspondance: Option<[u8; 12]> = None;
        let mut nb_trouves = 0;

        for i in (0..cluster_size).step_by(32) {
            let entry: &DirectoryEntry = &*(root[i..].as_ptr() as *const DirectoryEntry);

            if entry.is_valid() && !entry.is_directory() {
                // Déclaration et initialisation à chaque itération pour éviter les résidus
                let mut buf = [0u8; 12];
                let name = entry.get_name(&mut buf);

                let mut match_prefix = true;

                if prefixe.len() > name.len() {
                    match_prefix = false;
                } else {
                    for (c1, c2) in prefixe.bytes().zip(name.bytes()) {
                        if c1.to_ascii_uppercase() != c2.to_ascii_uppercase() {
                            match_prefix = false;
                            break;
                        }
                    }
                }

                if match_prefix && name.len() > prefixe.len() {
                    correspondance = Some(buf);
                    nb_trouves += 1;
                }
            }
        }

        if nb_trouves == 1 {
            let res = correspondance.unwrap();
            let mut len = 0;
            while len < 12 && res[len] != 0 {
                len += 1;
            }
            if len <= nom_complet_out.len() {
                nom_complet_out[..len].copy_from_slice(&res[..len]);
                return Some(len);
            }
        }
        None
    }
}

//----------------------------------------------------------Commande TOUCH----------------------------------------------------------

/// Commande TOUCH : Cree un fichier avec du contenu dans le repertoire racine de la FAT32.
pub unsafe fn touch(name: &str, data: &[u8]) {
    let (fat_offset, _, data_offset) = calculate_offsets();
    let cluster_size = obtenir_taille_cluster();

    // 1. Formatage du nom saisi au format standard FAT 8.3
    let formatted_name = match formater_nom_8_3(name) {
        Some(n) => n,
        None => {
            println!("Erreur : Le nom de fichier est invalide ou trop long (max 8.3).");
            return;
        }
    };

    // 2. Recherche d'une entree libre (0x00 ou 0xE5) dans le repertoire racine
    let root_directory = obtenir_repertoire_racine_mut(cluster_size);
    let entry_offset = match trouver_entree_libre(root_directory, cluster_size) {
        Some(offset) => offset,
        None => {
            println!("Erreur : Impossible de creer le fichier, le repertoire racine est plein.");
            return;
        }
    };

    // 3. Recherche d'un cluster libre dans la table FAT pour stocker le fichier
    let first_cluster = match find_free_cluster(fat_offset) {
        Some(cluster) => cluster,
        None => {
            println!("Erreur : Plus d'espace disponible sur le disque (plus de cluster libre).");
            return;
        }
    };

    // 4. Ecriture des donnees dans les clusters chaines
    let mut current_cluster = first_cluster;
    let mut remaining_data = data;
    loop {
        let offset = calculate_cluster_offset(data_offset, current_cluster);
        let cluster_data = &mut DISK_IMAGE[offset..offset + cluster_size];

        let to_write = core::cmp::min(remaining_data.len(), cluster_size);
        cluster_data[..to_write].copy_from_slice(&remaining_data[..to_write]);
        remaining_data = &remaining_data[to_write..];

        if remaining_data.is_empty() {
            mark_cluster_as_end(fat_offset, current_cluster);
            break;
        }

        let next_cluster = match find_free_cluster(fat_offset) {
            Some(cluster) => cluster,
            None => {
                println!("Erreur : Espace disque insuffisant pour ecrire la suite du fichier.");
                return;
            }
        };

        link_clusters(fat_offset, current_cluster, next_cluster);
        current_cluster = next_cluster;
    }

    // 5. Ecriture de la fiche d'entree (Directory Entry) de 32 octets dans la racine
    ecrire_entree_repertoire(
        root_directory,
        entry_offset,
        &formatted_name,
        0x20, // Attribut : Archive (Fichier standard)
        first_cluster,
        data.len() as u32,
    );

    println!("Fichier '{}' cree avec succes.", name);
}

//----------------------------------------------------------Commande MKDIR----------------------------------------------------------

/// Commande MKDIR : Cree un dossier vide dans le repertoire racine de la FAT32.
pub unsafe fn mkdir(name: &str) {
    let (fat_offset, _, data_offset) = calculate_offsets();
    let cluster_size = obtenir_taille_cluster();

    // 1. Formatage du nom de dossier au format FAT (8 caracteres max)
    let formatted_name = match formater_nom_8_3(name) {
        Some(n) => n,
        None => {
            println!("Erreur : Le nom du dossier est trop long (max 8 caracteres).");
            return;
        }
    };

    // 2. Recherche d'une entree libre dans le repertoire racine
    let root_directory = obtenir_repertoire_racine_mut(cluster_size);
    let entry_offset = match trouver_entree_libre(root_directory, cluster_size) {
        Some(offset) => offset,
        None => {
            println!("Erreur : Impossible de creer le dossier, le repertoire racine est plein.");
            return;
        }
    };

    // 3. Recherche d'un cluster libre dans la FAT pour contenir le dossier
    let first_cluster = match find_free_cluster(fat_offset) {
        Some(cluster) => cluster,
        None => {
            println!("Erreur : Plus de cluster libre disponible pour le dossier.");
            return;
        }
    };

    // 4. Initialisation du nouveau cluster de dossier (rempli de zeros)
    let offset = calculate_cluster_offset(data_offset, first_cluster);
    let cluster_data = &mut DISK_IMAGE[offset..offset + cluster_size];
    cluster_data.fill(0);

    // 5. Ecriture de l'entree "." (le dossier lui-meme, premier cluster)
    ecrire_entree_repertoire(cluster_data, 0, b".          ", 0x10, first_cluster, 0);

    // 6. Ecriture de l'entree ".." (le repertoire pointe vers le parent, racine = 0)
    ecrire_entree_repertoire(cluster_data, 32, b"..         ", 0x10, 0, 0);

    // 7. On ferme la chaine de clusters pour ce dossier
    mark_cluster_as_end(fat_offset, first_cluster);

    // 8. Creation de la fiche d'entree (Directory Entry) du dossier dans la racine (32 octets)
    ecrire_entree_repertoire(
        root_directory,
        entry_offset,
        &formatted_name,
        0x10, // Attribut : Dossier (0x10)
        first_cluster,
        0,
    );

    println!("Dossier '{}' cree avec succes.", name);
}
