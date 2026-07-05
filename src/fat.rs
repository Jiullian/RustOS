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

//---------------------------------------------------------------------------------------------------------------------------------
// CONSTANTES & CONFIGURATION FAT32
//---------------------------------------------------------------------------------------------------------------------------------
// Table des attributs de fichiers format FAT32
// - 0x10 : Dossier/Répertoire (Directory)
// - 0x20 : Fichier d'archive normal (Archive)
//---------------------------------------------------------------------------------------------------------------------------------

//---------------------------------------------------------------------------------------------------------------------------------
// FONCTIONS D'AIDE (HELPERS)
//---------------------------------------------------------------------------------------------------------------------------------

/// Formate un nom de fichier ou de dossier saisi par l'utilisateur pour le convertir au format binaire "8.3" de FAT.
///
/// Le format 8.3 de FAT exige que les noms soient représentés sur 11 octets fixes :
/// - Les 8 premiers octets contiennent le nom (rempli d'espaces 0x20 si plus court).
/// - Les 3 octets suivants contiennent l'extension (remplie d'espaces 0x20 si plus courte).
///
/// # Arguments
/// * `nom` - La chaîne de caractères du fichier (ex: "hello.txt" ou "dossier").
///
/// # Retour
/// * `Some([u8; 11])` contenant le nom formaté si valide.
/// * `None` si le nom ou l'extension dépasse les limites du format 8.3.
fn formater_nom_8_3(nom: &str) -> Option<[u8; 11]> {
    let mut formatted = [b' '; 11];

    // Cas 1 : Présence d'un point indiquant une extension (ex: "fichier.txt")
    if let Some((n, e)) = nom.split_once('.') {
        // Validation des longueurs (le nom ne doit pas dépasser 8 caractères et l'extension 3)
        if n.len() > 8 || e.len() > 3 {
            return None;
        }
        // Copie des caractères du nom dans la première partie du tableau (indices 0 à 7)
        formatted[..n.len()].copy_from_slice(n.as_bytes());
        // Copie des caractères de l'extension dans la seconde partie du tableau (indices 8 à 10)
        formatted[8..8 + e.len()].copy_from_slice(e.as_bytes());
    }
    // Cas 2 : Pas de point, nom simple sans extension (ex: "dossier")
    else {
        // Validation (le nom ne doit pas dépasser 8 caractères)
        if nom.len() > 8 {
            return None;
        }
        // Copie du nom simple
        formatted[..nom.len()].copy_from_slice(nom.as_bytes());
    }

    Some(formatted)
}

/// Parcourt séquentiellement un répertoire d'une certaine taille à la recherche d'une entrée libre.
///
/// En FAT32, un emplacement d'entrée de répertoire est libre pour écriture si :
/// - Son premier octet vaut `0x00` : l'entrée n'a jamais été utilisée.
/// - Son premier octet vaut `0xE5` : l'entrée contenait un fichier qui a été supprimé.
///
/// # Arguments
/// * `repertoire` - La tranche de mémoire représentant le répertoire (généralement la racine).
/// * `taille_cluster` - La taille d'un cluster en octets (limite du parcours).
///
/// # Retour
/// * `Some(usize)` - L'offset en octets du début de l'entrée libre trouvée.
/// * `None` - Si le répertoire est plein et qu'aucune entrée n'est libre.
fn trouver_entree_libre(repertoire: &[u8], taille_cluster: usize) -> Option<usize> {
    // Chaque entrée faisant 32 octets, on avance de 32 en 32
    for i in (0..taille_cluster).step_by(32) {
        if repertoire[i] == 0x00 || repertoire[i] == 0xE5 {
            return Some(i);
        }
    }
    None
}

/// Détermine dynamiquement la taille en octets d'un cluster en lisant les paramètres du disque.
///
/// repose sur le BIOS Parameter Block (BPB) de l'image FAT32 :
/// - Les octets 11-12 indiquent le nombre d'octets par secteur
/// - L'octet 13 indique le nombre de secteurs par cluster
/// Taille du cluster = (Octets par secteur) * (Secteurs par cluster)
fn obtenir_taille_cluster() -> usize {
    unsafe {
        // Lecture d'un entier 16 bits en Little Endian aux offsets 11-12
        let bps = u16::from_le_bytes([DISK_IMAGE[11], DISK_IMAGE[12]]) as usize;
        // Lecture de l'octet de secteur par cluster à l'offset 13
        let sectors_per_cluster = DISK_IMAGE[13] as usize;

        bps * sectors_per_cluster
    }
}

/// Récupère une référence partagée sur le répertoire racine en mémoire.
///
/// # Arguments
/// * `cluster_size` - Taille du cluster en octets.
fn obtenir_repertoire_racine(cluster_size: usize) -> &'static [u8] {
    unsafe {
        // Récupération de l'offset de départ du répertoire racine calculé depuis le BPB
        let root_offset = calculate_offsets().1;
        // Retourne la tranche mémoire immuable correspondante
        &DISK_IMAGE[root_offset..root_offset + cluster_size]
    }
}

/// Récupère une référence exclusive (mutable) sur le répertoire racine en mémoire.
///
/// # Arguments
/// * `cluster_size` - Taille du cluster en octets.
fn obtenir_repertoire_racine_mut(cluster_size: usize) -> &'static mut [u8] {
    unsafe {
        // Récupération de l'offset de départ du répertoire racine calculé depuis le BPB
        let root_offset = calculate_offsets().1;
        // Retourne la tranche mémoire mutable pour permettre les modifications de fichiers
        &mut DISK_IMAGE[root_offset..root_offset + cluster_size]
    }
}

/// Initialise et écrit les 32 octets d'une entrée de répertoire FAT32.
///
/// # Arguments
/// * `repertoire` - Répertoire dans lequel on écrit.
/// * `offset` - Position de départ de l'entrée dans le répertoire (multiple de 32).
/// * `nom` - Nom formaté au format 8.3 (11 octets).
/// * `attribut` - Type de fichier (0x20 pour fichier normal, 0x10 pour dossier).
/// * `premier_cluster` - Index du premier cluster de données.
/// * `taille_fichier` - Taille totale en octets (vaut 0 pour un dossier).
fn ecrire_entree_repertoire(
    repertoire: &mut [u8],
    offset: usize,
    nom: &[u8; 11],
    attribut: u8,
    premier_cluster: u32,
    taille_fichier: u32,
) {
    let entry = &mut repertoire[offset..offset + 32];

    // Remplissage des 32 octets selon le format standard de FAT32 :
    entry[0..11].copy_from_slice(nom); // Octets 0-10 : Nom complet au format 8.3
    entry[11] = attribut; // Octet 11 : Attribut du fichier
    entry[12..20].fill(0); // Octets 12-19 : Champs réservés, dates/heures créations à 0
    entry[20..22].copy_from_slice(&((premier_cluster >> 16) as u16).to_le_bytes()); // Octets 20-21 : Partie haute (16 bits) du premier cluster
    entry[22..26].fill(0); // Octets 22-25 : Date et heure de dernière écriture à 0
    entry[26..28].copy_from_slice(&(premier_cluster as u16).to_le_bytes()); // Octets 26-27 : Partie basse (16 bits) du premier cluster
    entry[28..32].copy_from_slice(&taille_fichier.to_le_bytes()); // Octets 28-31 : Taille du fichier (entier 32 bits)
}

/// Lit une entrée (pointeur) dans la table FAT pour savoir quel est le cluster suivant dans la chaîne.
///
/// Chaque entrée de la table FAT fait 32 bits (4 octets) et contient le numéro du prochain cluster.
/// Le format FAT32 n'utilise que 28 bits pour stocker l'adresse, les 4 bits supérieurs sont réservés.
///
/// # Arguments
/// * `cluster` - Le numéro du cluster courant à interroger.
/// * `fat_offset` - L'adresse mémoire de départ de la table FAT.
///
/// # Retour
/// * Le numéro du cluster suivant (28 bits utiles).
fn lire_pointeur_fat(cluster: u32, fat_offset: usize) -> u32 {
    unsafe {
        // Chaque entrée dans la FAT fait 4 octets. L'offset de l'entrée est : fat_offset + cluster * 4
        let offset = fat_offset + (cluster as usize * 4);

        // Lecture de l'entier 32 bits (Little Endian)
        u32::from_le_bytes([
            DISK_IMAGE[offset],
            DISK_IMAGE[offset + 1],
            DISK_IMAGE[offset + 2],
            DISK_IMAGE[offset + 3],
        ]) & 0x0FFFFFFF // Masquage pour ne garder que les 28 bits inférieurs de la FAT32
    }
}

/// Parcourt le répertoire racine pour localiser un fichier ou dossier par son nom exact.
///
/// # Arguments
/// * `nom_cible` - Le nom recherché (ex: "NOTE.TXT").
///
/// # Retour
/// * `Some((offset, DirectoryEntry))` si trouvé : contient sa position en octets et sa structure copiée.
/// * `None` si l'élément n'existe pas.
fn chercher_entree(nom_cible: &str) -> Option<(usize, DirectoryEntry)> {
    let cluster_size = obtenir_taille_cluster();
    let root = obtenir_repertoire_racine(cluster_size);
    let mut buf = [0u8; 12]; // Buffer pour extraire le nom lisible de l'entrée

    for i in (0..cluster_size).step_by(32) {
        // Lecture sécurisée via pointeur brut pour interpréter les 32 octets comme une structure DirectoryEntry
        let entry: &DirectoryEntry = unsafe { &*(root[i..].as_ptr() as *const DirectoryEntry) };

        if entry.is_valid() {
            let name = entry.get_name(&mut buf);
            // Comparaison insensible à la casse
            if name == nom_cible {
                return Some((i, *entry));
            }
        }
    }
    None
}

/// Libère toute la chaîne de clusters associée à un fichier ou un répertoire dans la table FAT.
///
/// Parcourt la chaîne de pointeurs dans la FAT et remplace chaque pointeur de cluster par `0x00000000`
/// (indiquant que le cluster physique est libéré et réutilisable pour de futurs fichiers).
///
/// # Arguments
/// * `premier_cluster` - Le premier cluster du fichier à partir duquel commencer la libération.
/// * `fat_offset` - L'offset de départ de la table FAT en mémoire.
unsafe fn liberer_chaine_clusters(premier_cluster: u32, fat_offset: usize) {
    let mut current_cluster = premier_cluster;

    // Sous FAT32, les numéros de clusters valides de données commencent à 2.
    // Une valeur supérieure ou égale à 0x0FFFFFF8 indique la fin officielle de la chaîne (EOF).
    while current_cluster >= 2 && current_cluster < 0x0FFFFFF8 {
        // 1. Lire et sauvegarder le pointeur vers le cluster suivant avant de modifier l'entrée courante
        let next_cluster = lire_pointeur_fat(current_cluster, fat_offset);

        // 2. Calculer la position de l'entrée du cluster actuel dans la table FAT
        let fat_entry_offset = fat_offset + (current_cluster as usize * 4);

        // 3. Remplacer l'entrée par 0 pour marquer le cluster comme libre
        DISK_IMAGE[fat_entry_offset..fat_entry_offset + 4].copy_from_slice(&0u32.to_le_bytes());

        // 4. Passer au cluster suivant
        current_cluster = next_cluster;
    }
}

//---------------------------------------------------------------------------------------------------------------------------------
// STRUCTURE D'ENTREE DE REPERTOIRE (DIRECTORY ENTRY)
//---------------------------------------------------------------------------------------------------------------------------------

/// Structure de données représentant physiquement une entrée de répertoire de 32 octets en FAT32.
/// L'attribut `#[repr(C, packed)]` garantit que Rust respecte l'alignement exact des octets
/// requis par la spécification matérielle de FAT32.
#[derive(Clone, Copy)]
#[allow(dead_code)]
#[repr(C, packed)]
pub struct DirectoryEntry {
    pub name: [u8; 11], // Octets 0-10 : Nom (8 caractères) et Extension (3 caractères)
    pub attributes: u8, // Octet 11 : Attributs (Fichier, Répertoire, Caché, Système...)
    pub nt_res: u8,     // Octet 12 : Réservé pour Windows NT (à 0)
    pub creation_time_tenths: u8, // Octet 13 : Millisecondes de création du fichier
    pub creation_time: u16, // Octets 14-15 : Heure de création
    pub creation_date: u16, // Octets 16-17 : Date de création
    pub last_access_date: u16, // Octets 18-19 : Date de dernier accès
    pub first_cluster_high: u16, // Octets 20-21 : Partie haute (16 bits de poids fort) du premier cluster
    pub write_time: u16,         // Octets 22-23 : Heure de dernière modification
    pub write_date: u16,         // Octets 24-25 : Date de dernière modification
    pub first_cluster_low: u16, // Octets 26-27 : Partie basse (16 bits de poids faible) du premier cluster
    pub file_size: u32,         // Octets 28-31 : Taille du fichier en octets
}

impl DirectoryEntry {
    /// Indique si l'entrée est active et valide (ni vide, ni supprimée, ni une entrée LFN).
    ///
    /// - Le premier octet ne doit pas valoir `0x00` (entrée vide/fin de répertoire).
    /// - Le premier octet ne doit pas valoir `0xE5` (fichier supprimé).
    /// - Les attributs ne doivent pas valoir `0x0F` (utilisé pour les noms longs LFN).
    pub fn is_valid(&self) -> bool {
        self.name[0] != 0x00 && self.name[0] != 0xE5 && self.attributes != 0x0F
    }

    /// Décode le format brut de 11 octets "8.3" pour produire un nom de fichier lisible en Rust.
    /// Retire les espaces de rembourrage et ajoute un point "." avant l'extension s'il y en a une.
    ///
    /// # Arguments
    /// * `buf` - Un tampon mutable de 12 octets utilisé pour copier le texte découpé.
    pub fn get_name<'a>(&self, buf: &'a mut [u8]) -> &'a str {
        let name_field = self.name;
        let mut i = 0;

        // 1. Extraction du nom de fichier (les 8 premiers octets)
        // On s'arrête dès qu'on rencontre un espace de rembourrage (0x20 / b' ')
        for &c in name_field[0..8].iter().take_while(|&&c| c != b' ') {
            buf[i] = c;
            i += 1;
        }

        // 2. Extraction de l'extension (les 3 octets restants)
        let ext = &name_field[8..11];
        if ext[0] != b' ' {
            buf[i] = b'.'; // Insertion du point séparateur
            i += 1;
            for &c in ext.iter().take_while(|&&c| c != b' ') {
                buf[i] = c;
                i += 1;
            }
        }

        // Conversion finale du tampon en &str UTF-8 (ou renvoie "?" en cas d'erreur de décodage)
        core::str::from_utf8(&buf[..i]).unwrap_or("?")
    }

    /// Retourne true si cette entrée est un sous-répertoire.
    pub fn is_directory(&self) -> bool {
        self.attributes & 0x10 != 0
    }

    /// Reconstitue l'index complet (32 bits) du premier cluster en combinant
    /// la partie haute (first_cluster_high) et la partie basse (first_cluster_low).
    pub fn first_cluster(&self) -> u32 {
        ((self.first_cluster_high as u32) << 16) | (self.first_cluster_low as u32)
    }

    /// Retourne la taille du fichier.
    pub fn file_size(&self) -> usize {
        self.file_size as usize
    }
}

//---------------------------------------------------------------------------------------------------------------------------------
// COMMANDE LS
//---------------------------------------------------------------------------------------------------------------------------------

/// Commande LS : Liste et affiche à l'écran tous les fichiers et dossiers présents dans le répertoire racine.
pub fn ls() {
    let cluster_size = obtenir_taille_cluster();
    let root = obtenir_repertoire_racine(cluster_size);
    let mut buf = [0u8; 12];
    let mut col = 0; // Compteur de colonnes pour l'affichage tabulaire

    // Parcours de toutes les entrées de 32 octets du répertoire racine
    for i in (0..cluster_size).step_by(32) {
        let entry: &DirectoryEntry = unsafe { &*(root[i..].as_ptr() as *const DirectoryEntry) };

        if entry.is_valid() {
            let name = entry.get_name(&mut buf);

            // Retour à la ligne si la colonne dépasse la largeur de la console (80 caractères)
            if col + 14 > 80 {
                println!();
                col = 0;
            }

            // Formatage esthétique : dossiers en bleu clair, fichiers normaux en blanc
            if entry.is_directory() {
                set_color(Color::LightBlue, Color::Black);
                print!("{}", name);
                reset_color();
            } else {
                print!("{}", name);
            }

            // Espacement fixe de 14 caractères par élément pour l'alignement
            for _ in 0..(14 - name.len()) {
                print!(" ");
            }
            col += 14;
        }
    }
    // Si la dernière ligne a du texte, on ajoute un saut de ligne final
    if col > 0 {
        println!();
    }
}

//---------------------------------------------------------------------------------------------------------------------------------
// COMMANDE CAT
//---------------------------------------------------------------------------------------------------------------------------------

/// Commande CAT : Lit et affiche à la console le contenu textuel d'un fichier spécifié.
///
/// # Arguments
/// * `file_name` - Le nom du fichier à lire (ex: "EXEMPLE.TXT").
pub fn cat(file_name: &str) {
    // 1. Recherche du fichier par son nom dans la racine
    if let Some((_, entry)) = chercher_entree(file_name) {
        // On s'assure que c'est un fichier et non un dossier
        if !entry.is_directory() {
            let (fat_offset, _, data_offset) = calculate_offsets();
            let cluster_size = obtenir_taille_cluster();

            let mut current_cluster = entry.first_cluster();
            let mut remaining_size = entry.file_size();

            // 2. Lecture séquentielle de la chaîne de clusters du fichier
            while current_cluster < 0x0FFFFFF8 {
                // Calcul de l'adresse du cluster courant dans la zone de données
                let offset = calculate_cluster_offset(data_offset, current_cluster);

                // Détermination du nombre d'octets restants à lire dans ce cluster
                let to_read = core::cmp::min(remaining_size, cluster_size);

                unsafe {
                    let content = &DISK_IMAGE[offset..offset + to_read];
                    // Tentative d'affichage textuel si le fichier est encodé en UTF-8
                    if let Ok(text) = core::str::from_utf8(content) {
                        print!("{}", text);
                    } else {
                        println!("\nErreur : Le fichier contient des donnees non-UTF8.");
                        return;
                    }
                }

                // Décrémentation de la taille restante
                remaining_size -= to_read;
                if remaining_size == 0 {
                    break;
                }

                // Récupération de l'adresse du cluster suivant dans la table FAT
                current_cluster = lire_pointeur_fat(current_cluster, fat_offset);
            }
            println!(); // Saut de ligne esthétique final
            return;
        }
    }
    println!("Fichier introuvable : '{}'", file_name);
}

//---------------------------------------------------------------------------------------------------------------------------------
// SYSTEME DE COMPLETION AUTOMATIQUE (TAB)
//---------------------------------------------------------------------------------------------------------------------------------

/// Recherche un fichier dans la racine dont le début du nom correspond au préfixe saisi, pour l'autocomplétion.
///
/// # Arguments
/// * `prefixe` - Le début du nom saisi par l'utilisateur (ex: "EX").
/// * `nom_complet_out` - Buffer d'écriture pour renvoyer le nom complété s'il est unique.
///
/// # Retour
/// * `Some(usize)` - Longueur du nom complété en octets en cas de correspondance UNIQUE.
/// * `None` - Si aucun ou plusieurs fichiers correspondent.
pub fn completer_nom(prefixe: &str, nom_complet_out: &mut [u8]) -> Option<usize> {
    let cluster_size = obtenir_taille_cluster();
    let root = obtenir_repertoire_racine(cluster_size);
    let mut correspondance: Option<[u8; 12]> = None;
    let mut nb_trouves = 0;

    for i in (0..cluster_size).step_by(32) {
        let entry: &DirectoryEntry = unsafe { &*(root[i..].as_ptr() as *const DirectoryEntry) };

        // On ne complète que les fichiers valides
        if entry.is_valid() && !entry.is_directory() {
            let mut buf = [0u8; 12];
            let name = entry.get_name(&mut buf);
            let mut match_prefix = true;

            if prefixe.len() > name.len() {
                match_prefix = false;
            } else {
                // Comparaison caractère par caractère insensible à la casse
                for (c1, c2) in prefixe.bytes().zip(name.bytes()) {
                    if c1.to_ascii_uppercase() != c2.to_ascii_uppercase() {
                        match_prefix = false;
                        break;
                    }
                }
            }

            // On exclut les correspondances parfaites pour ne compléter que ce qui est inachevé
            if match_prefix && name.len() > prefixe.len() {
                correspondance = Some(buf);
                nb_trouves += 1;
            }
        }
    }

    // L'autocomplétion n'est validée que si une seule et unique correspondance est trouvée sur le disque
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

//---------------------------------------------------------------------------------------------------------------------------------
// COMMANDE TOUCH
//---------------------------------------------------------------------------------------------------------------------------------

/// Commande TOUCH : Crée un nouveau fichier avec du contenu dans le répertoire racine.
///
/// # Arguments
/// * `name` - Nom du fichier (ex: "NOTE.TXT").
/// * `data` - Tableau d'octets contenant les données à y écrire.
pub unsafe fn touch(name: &str, data: &[u8]) {
    let (fat_offset, _, data_offset) = calculate_offsets();
    let cluster_size = obtenir_taille_cluster();

    // 1. Validation et formatage du nom au format FAT 8.3
    let formatted_name = match formater_nom_8_3(name) {
        Some(n) => n,
        None => {
            println!("Erreur : Le nom de fichier est trop long.");
            return;
        }
    };

    // 2. Recherche d'une entrée libre (32 octets) dans le répertoire racine
    let root_directory = obtenir_repertoire_racine_mut(cluster_size);
    let entry_offset = match trouver_entree_libre(root_directory, cluster_size) {
        Some(offset) => offset,
        None => {
            println!("Erreur : Le repertoire racine est plein.");
            return;
        }
    };

    // 3. Recherche du premier cluster vide dans la table FAT pour stocker le fichier
    let first_cluster = match find_free_cluster(fat_offset) {
        Some(cluster) => cluster,
        None => {
            println!("Erreur : Espace disque insuffisant (plus de cluster libre).");
            return;
        }
    };

    // 4. Écriture des données sur le disque en gérant le chaînage si le fichier dépasse 1 cluster
    let mut current_cluster = first_cluster;
    let mut remaining_data = data;
    loop {
        let offset = calculate_cluster_offset(data_offset, current_cluster);
        let cluster_data = &mut DISK_IMAGE[offset..offset + cluster_size];

        // Quantité de données à copier dans le cluster courant
        let to_write = core::cmp::min(remaining_data.len(), cluster_size);
        cluster_data[..to_write].copy_from_slice(&remaining_data[..to_write]);

        remaining_data = &remaining_data[to_write..];

        // Si toutes les données ont été écrites, on ferme la chaîne FAT pour ce fichier
        if remaining_data.is_empty() {
            mark_cluster_as_end(fat_offset, current_cluster);
            break;
        }

        // Sinon, on cherche un cluster supplémentaire libre
        let next_cluster = match find_free_cluster(fat_offset) {
            Some(cluster) => cluster,
            None => {
                println!("Erreur : Espace insuffisant pour terminer l'ecriture.");
                return;
            }
        };

        // On lie le cluster courant au nouveau cluster dans la FAT
        link_clusters(fat_offset, current_cluster, next_cluster);
        current_cluster = next_cluster;
    }

    // 5. Enregistrement final de la fiche descriptive (Directory Entry) du fichier dans la racine
    ecrire_entree_repertoire(
        root_directory,
        entry_offset,
        &formatted_name,
        0x20, // Attribut : archive (fichier normal)
        first_cluster,
        data.len() as u32,
    );
    println!("Fichier '{}' cree avec succes.", name);
}

//---------------------------------------------------------------------------------------------------------------------------------
// COMMANDE MKDIR
//---------------------------------------------------------------------------------------------------------------------------------

/// Commande MKDIR : Crée un sous-dossier vide dans le répertoire racine.
///
/// Un dossier vide en FAT32 est un cluster contenant deux entrées spéciales pré-initialisées :
/// - L'entrée "." (offset 0) : pointe vers son propre cluster de départ.
/// - L'entrée ".." (offset 32) : pointe vers le dossier parent (le répertoire racine, index 0).
///
/// # Arguments
/// * `name` - Nom du dossier (ex: "DOCS").
pub unsafe fn mkdir(name: &str) {
    let (fat_offset, _, data_offset) = calculate_offsets();
    let cluster_size = obtenir_taille_cluster();

    // 1. Validation et formatage du nom au format FAT (8 caractères max)
    let formatted_name = match formater_nom_8_3(name) {
        Some(n) => n,
        None => {
            println!("Erreur : Le nom de dossier est trop long (max 8 caracteres).");
            return;
        }
    };

    // 2. Recherche d'une entrée libre dans la racine
    let root_directory = obtenir_repertoire_racine_mut(cluster_size);
    let entry_offset = match trouver_entree_libre(root_directory, cluster_size) {
        Some(offset) => offset,
        None => {
            println!("Erreur : Le repertoire racine est plein.");
            return;
        }
    };

    // 3. Recherche d'un cluster disponible pour stocker le contenu du dossier
    let first_cluster = match find_free_cluster(fat_offset) {
        Some(cluster) => cluster,
        None => {
            println!("Erreur : Aucun cluster libre.");
            return;
        }
    };

    // 4. Initialisation du nouveau cluster (remplissage à zéro)
    let offset = calculate_cluster_offset(data_offset, first_cluster);
    let cluster_data = &mut DISK_IMAGE[offset..offset + cluster_size];
    cluster_data.fill(0);

    // 5. Écriture des entrées indispensables "." et ".." à l'intérieur du dossier
    ecrire_entree_repertoire(cluster_data, 0, b".          ", 0x10, first_cluster, 0); // "." pointe vers lui-même
    ecrire_entree_repertoire(cluster_data, 32, b"..         ", 0x10, 0, 0); // ".." pointe vers le parent (racine = 0)

    // 6. Fermeture de la chaîne de clusters dans la table FAT pour ce dossier
    mark_cluster_as_end(fat_offset, first_cluster);

    // 7. Écriture de la fiche du dossier (Directory Entry) dans le répertoire racine
    ecrire_entree_repertoire(
        root_directory,
        entry_offset,
        &formatted_name,
        0x10, // Attribut : Dossier (0x10)
        first_cluster,
        0, // La taille d'un dossier est toujours égale à 0 sous FAT
    );

    println!("Dossier '{}' cree avec succes.", name);
}

//---------------------------------------------------------------------------------------------------------------------------------
// COMMANDE RM
//---------------------------------------------------------------------------------------------------------------------------------

/// Commande RM : Supprime un fichier du répertoire racine et libère son espace disque.
///
/// # Arguments
/// * `name` - Nom du fichier à supprimer.
pub unsafe fn rm(name: &str) {
    // 1. Recherche du fichier par son nom dans la racine
    if let Some((offset, entry)) = chercher_entree(name) {
        // Sécurité : on refuse de supprimer des dossiers via la commande 'rm'
        if !entry.is_directory() {
            let (fat_offset, _, _) = calculate_offsets();

            // 2. Libérer toute la chaîne de clusters occupée par le fichier dans la table FAT
            liberer_chaine_clusters(entry.first_cluster(), fat_offset);

            // 3. Marquer l'entrée comme supprimée en écrivant l'octet spécial 0xE5 au début de son nom
            let root_directory = obtenir_repertoire_racine_mut(obtenir_taille_cluster());
            root_directory[offset] = 0xE5;

            println!("Fichier '{}' supprime avec succes.", name);
            return;
        }
    }
    println!("Fichier introuvable : '{}'", name);
}

//---------------------------------------------------------------------------------------------------------------------------------
// COMMANDE RMDIR
//---------------------------------------------------------------------------------------------------------------------------------

/// Commande RMDIR : Supprime un sous-dossier s'il est vide et libère son espace.
///
/// # Arguments
/// * `name` - Nom du dossier à supprimer.
pub unsafe fn rmdir(name: &str) {
    // Sécurité de base : impossible de supprimer les raccourcis de navigation "." et ".."
    if name == "." || name == ".." {
        println!("Erreur : Impossible de supprimer '.' ou '..'.");
        return;
    }

    // 1. Recherche du dossier dans la racine
    if let Some((offset, entry)) = chercher_entree(name) {
        // On s'assure qu'il s'agit bien d'un dossier
        if entry.is_directory() {
            let (fat_offset, _, data_offset) = calculate_offsets();
            let cluster_size = obtenir_taille_cluster();
            let dir_cluster = entry.first_cluster();

            // 2. VÉRIFICATION DE SÉCURITÉ : Le dossier doit être VIDE.
            // On parcourt son cluster de données. S'il contient autre chose que "." et "..", on annule.
            let dir_offset = calculate_cluster_offset(data_offset, dir_cluster);
            let dir_data = &DISK_IMAGE[dir_offset..dir_offset + cluster_size];
            let mut check_buf = [0u8; 12];

            for j in (0..cluster_size).step_by(32) {
                let sub_entry: &DirectoryEntry =
                    &*(dir_data[j..].as_ptr() as *const DirectoryEntry);
                if sub_entry.is_valid() {
                    let sub_name = sub_entry.get_name(&mut check_buf);
                    // Si on trouve une entrée valide différente de "." et "..", le dossier n'est pas vide !
                    if sub_name != "." && sub_name != ".." {
                        println!("Erreur : Le dossier '{}' n'est pas vide.", name);
                        return;
                    }
                }
            }

            // 3. Libérer le cluster réservé au dossier dans la table FAT
            liberer_chaine_clusters(dir_cluster, fat_offset);

            // 4. Marquer le dossier comme supprimé (0xE5) dans la racine
            let root_directory = obtenir_repertoire_racine_mut(cluster_size);
            root_directory[offset] = 0xE5;

            println!("Dossier '{}' supprime avec succes.", name);
            return;
        }
    }
    println!("Dossier introuvable : '{}'", name);
}
