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

//----------------------------------------------------------Structure Entree Repertoire----------------------------------------------------------

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

    /// Récupère le premier cluster du fichier/dossier.
    ///
    /// Dans la table d'entrée de 32 octets :
    /// - La partie haute du cluster (High cluster) est aux octets 20 et 21.
    /// - La partie basse du cluster (Low cluster) est aux octets 26 et 27.
    pub fn first_cluster(&self) -> u32 {
        unsafe {
            let ptr = self as *const DirectoryEntry as *const u8;
            let high = u16::from_le_bytes([*ptr.add(20), *ptr.add(21)]) as u32;
            let low = u16::from_le_bytes([*ptr.add(26), *ptr.add(27)]) as u32;
            (high << 16) | low
        }
    }

    /// Récupère la taille du fichier (octets 28 à 31 de la structure de répertoire)
    pub fn file_size(&self) -> usize {
        unsafe {
            let ptr = self as *const DirectoryEntry as *const u8;
            u32::from_le_bytes([*ptr.add(28), *ptr.add(29), *ptr.add(30), *ptr.add(31)]) as usize
        }
    }
}

//----------------------------------------------------------Commande LS----------------------------------------------------------

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

//----------------------------------------------------------Commande CAT----------------------------------------------------------

/// Commande CAT : lit et affiche à l'écran le contenu d'un fichier texte présent sur le disque.
///
/// Cette fonction recherche un fichier par son nom dans le répertoire racine.
/// Si le fichier est trouvé, elle parcourt la table FAT pour charger et afficher
/// les blocs de données (clusters) successifs associés à ce fichier.
pub fn cat(file_name: &str) {
    unsafe {
        // Récupération des offsets des différentes zones du disque (FAT, répertoire racine, données)
        let (fat_offset, root_offset, data_offset) = calculate_offsets();

        // Calcul de la taille d'un cluster en mémoire (secteurs par cluster × octets par secteur)
        let bps = u16::from_le_bytes([DISK_IMAGE[11], DISK_IMAGE[12]]) as usize;
        let cluster_size = bps * DISK_IMAGE[13] as usize;

        // Extraction de la zone mémoire correspondant au répertoire racine
        let root = &DISK_IMAGE[root_offset..root_offset + cluster_size];

        // Tampon pour stocker et formater le nom du fichier recherché
        let mut buf = [0u8; 12];

        // 1. Recherche du fichier dans le répertoire racine (parcours par blocs de 32 octets)
        for i in (0..cluster_size).step_by(32) {
            // Conversion du bloc de 32 octets en structure DirectoryEntry
            let entry: &DirectoryEntry = &*(root[i..].as_ptr() as *const DirectoryEntry);

            // On vérifie que l'entrée est valide et qu'il s'agit bien d'un fichier (pas un dossier)
            if entry.is_valid() && !entry.is_directory() {
                let name = entry.get_name(&mut buf);

                // Si le nom formaté correspond exactement au nom demandé par l'utilisateur
                if name == file_name {
                    // Récupération du premier cluster du fichier et de sa taille totale en octets
                    let mut current_cluster = entry.first_cluster();
                    let mut remaining_size = entry.file_size();

                    // 2. Lecture des clusters chaînés du fichier (on s'arrête si on atteint le marqueur de fin de chaîne FAT32)
                    while current_cluster < 0x0FFFFFF8 {
                        // Calcul de la position physique du cluster actuel dans la zone de données
                        let offset = calculate_cluster_offset(data_offset, current_cluster);
                        let cluster_data = &DISK_IMAGE[offset..offset + cluster_size];

                        // Détermination du nombre d'octets à lire (la taille du cluster ou le reste du fichier)
                        let to_read = core::cmp::min(remaining_size, cluster_size);
                        let content = &cluster_data[..to_read];

                        // Tentative de conversion des données en chaîne UTF-8 pour l'affichage
                        if let Ok(text) = core::str::from_utf8(content) {
                            print!("{}", text);
                        } else {
                            println!(
                                "\nErreur : Le fichier contient des donnees non lisibles en UTF-8."
                            );
                            return;
                        }

                        // Mise à jour de la taille restante à lire
                        remaining_size -= to_read;
                        if remaining_size == 0 {
                            break; // Le fichier a été entièrement lu et affiché
                        }

                        // Récupération de l'index du cluster suivant dans la table FAT (chaque entrée fait 4 octets)
                        let fat_entry_offset = fat_offset + (current_cluster as usize * 4);
                        current_cluster = u32::from_le_bytes([
                            DISK_IMAGE[fat_entry_offset],
                            DISK_IMAGE[fat_entry_offset + 1],
                            DISK_IMAGE[fat_entry_offset + 2],
                            DISK_IMAGE[fat_entry_offset + 3],
                        ]) & 0x0FFFFFFF; // On applique le masque FAT32 sur 28 bits
                    }
                    println!(); // Saut de ligne final après l'affichage du fichier
                    return;
                }
            }
        }
        // Message d'erreur si le parcours complet n'a rien donné
        println!("Fichier introuvable : '{}'", file_name);
    }
}

//----------------------------------------------------------Completion Automatique----------------------------------------------------------

/// Cherche un fichier commençant par `prefixe` dans le répertoire racine.
/// Si un unique fichier correspond, écrit son nom complet dans `nom_complet_out` et retourne sa longueur.
/// La recherche est insensible à la casse pour faciliter la saisie (ex: "rea" complète vers "README.TXT").
pub fn completer_nom(prefixe: &str, nom_complet_out: &mut [u8]) -> Option<usize> {
    unsafe {
        // On récupère l'offset de départ du répertoire racine dans le disque
        let root_offset = calculate_offsets().1;

        // Calcul de la taille du cluster (bps = octets par secteur * secteurs par cluster)
        let bps = u16::from_le_bytes([DISK_IMAGE[11], DISK_IMAGE[12]]) as usize;
        let cluster_size = bps * DISK_IMAGE[13] as usize;

        // Récupération de la tranche mémoire contenant le répertoire racine
        let root = &DISK_IMAGE[root_offset..root_offset + cluster_size];

        // Tampon pour recevoir temporairement le nom extrait de chaque entrée
        let mut buf = [0u8; 12];

        // Variable pour stocker le nom trouvé si un fichier correspond
        let mut correspondance: Option<[u8; 12]> = None;

        // Compteur du nombre total de fichiers qui correspondent au préfixe
        let mut nb_trouves = 0;

        // Parcours de toutes les entrées de répertoire du répertoire racine (blocs de 32 octets)
        for i in (0..cluster_size).step_by(32) {
            // Interprétation des 32 octets comme une DirectoryEntry
            let entry: &DirectoryEntry = &*(root[i..].as_ptr() as *const DirectoryEntry);

            // On s'intéresse uniquement aux entrées valides et qui désignent des fichiers (pas des dossiers)
            if entry.is_valid() && !entry.is_directory() {
                // Extraction et formatage du nom (ex: "README.TXT")
                let name = entry.get_name(&mut buf);

                // Algorithme de vérification insensible à la casse
                let mut match_prefix = true;

                // Si le préfixe saisi est plus long que le nom du fichier, ce n'est pas une correspondance
                if prefixe.len() > name.len() {
                    match_prefix = false;
                } else {
                    // On compare lettre par lettre en convertissant tout en majuscules (to_ascii_uppercase)
                    for (c1, c2) in prefixe.bytes().zip(name.bytes()) {
                        if c1.to_ascii_uppercase() != c2.to_ascii_uppercase() {
                            match_prefix = false;
                            break; // Dès qu'une lettre diffère, on s'arrête
                        }
                    }
                }

                // Si le préfixe correspond et que le fichier a un nom plus long que la saisie actuelle
                // (évite de compléter si l'utilisateur a déjà tapé le nom exact)
                if match_prefix && name.len() > prefixe.len() {
                    correspondance = Some(buf); // On sauvegarde le nom trouvé
                    nb_trouves += 1; // On incrémente le compteur de correspondances
                }
            }
        }

        // Si et seulement si on a trouvé EXACTEMENT une seule correspondance unique sur le disque
        if nb_trouves == 1 {
            let res = correspondance.unwrap();
            let mut len = 0;

            // Calcul de la longueur du nom de fichier (jusqu'à l'octet nul de fin \0)
            while len < 12 && res[len] != 0 {
                len += 1;
            }

            // Si la taille du nom tient dans le buffer de sortie
            if len <= nom_complet_out.len() {
                // On copie le nom trouvé dans le tampon de sortie
                nom_complet_out[..len].copy_from_slice(&res[..len]);
                return Some(len); // On renvoie la longueur du nom complété
            }
        }

        // Si 0 fichier ou plusieurs fichiers correspondent, on ne fait aucune complétion
        None
    }
}

//----------------------------------------------------------Completion Automatique----------------------------------------------------------
