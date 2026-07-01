//! Gestion de l'image disque FAT32
//!
//! Ce module permet d'interagir avec une image disque FAT32 en mémoire. Il propose
//! des fonctions pour initialiser l'image, lire des informations et manipuler la FAT.
use crate::println;

/// Taille de l'image disque, déterminée à partir du fichier `imgfat32.img`.
pub const DISK_IMAGE_SIZE: usize = include_bytes!("../imgfat32.img").len();

/// Définition de l'image disque sous forme de tableau mutable pour permettre les modifications.
pub static mut DISK_IMAGE: [u8; DISK_IMAGE_SIZE] = [0u8; DISK_IMAGE_SIZE];

/// Initialise l'image disque en chargeant son contenu en mémoire.
pub fn initialize_disk_image() {
    unsafe {
        DISK_IMAGE.copy_from_slice(include_bytes!("../imgfat32.img"));
    }
}

/// Affiche les informations générales de l'image disque.
pub fn read_disk_info() {
    unsafe {
        let disk_size = DISK_IMAGE.len();
        println!("Taille de l'image disque : {} octets", disk_size);
    }
}

/// Calcule les offsets de la table FAT, du répertoire racine et des données.
///
/// # Retour
/// Un tuple `(fat_offset, root_directory_offset, data_offset)` indiquant les offsets respectifs.
pub fn calculate_offsets() -> (usize, usize, usize) {
    unsafe {
        let bytes_per_sector = u16::from_le_bytes([DISK_IMAGE[11], DISK_IMAGE[12]]) as usize;
        let reserved_sectors = u16::from_le_bytes([DISK_IMAGE[14], DISK_IMAGE[15]]) as usize;
        let num_fats = DISK_IMAGE[16] as usize;
        let fat_size_sectors = u32::from_le_bytes([
            DISK_IMAGE[36],
            DISK_IMAGE[37],
            DISK_IMAGE[38],
            DISK_IMAGE[39],
        ]) as usize;

        let fat_offset = reserved_sectors * bytes_per_sector;
        let root_directory_offset = fat_offset + (num_fats * fat_size_sectors * bytes_per_sector);
        let data_offset = root_directory_offset;

        (fat_offset, root_directory_offset, data_offset)
    }
}

/// Recherche un cluster libre dans la table FAT.
pub fn find_free_cluster(fat_offset: usize) -> Option<u32> {
    unsafe {
        for cluster in 2.. {
            let cluster_entry_offset = fat_offset + (cluster as usize * 4);
            let cluster_entry = u32::from_le_bytes([
                DISK_IMAGE[cluster_entry_offset],
                DISK_IMAGE[cluster_entry_offset + 1],
                DISK_IMAGE[cluster_entry_offset + 2],
                DISK_IMAGE[cluster_entry_offset + 3],
            ]) & 0x0FFFFFFF;

            if cluster_entry == 0 {
                return Some(cluster);
            }
        }
    }
    None
}

/// Calcule l'offset d'un cluster spécifique dans la zone de données.
pub fn calculate_cluster_offset(data_offset: usize, cluster: u32) -> usize {
    unsafe {
        let bytes_per_sector =
            u16::from_le_bytes([DISK_IMAGE[11], DISK_IMAGE[12]]) as usize;
        let sectors_per_cluster = DISK_IMAGE[13] as usize;
        let cluster_size = bytes_per_sector * sectors_per_cluster;

        data_offset + ((cluster - 2) as usize * cluster_size)
    }
}

/// Marque un cluster comme étant la fin de la chaîne dans la table FAT.
pub fn mark_cluster_as_end(fat_offset: usize, cluster: u32) {
    unsafe {
        let cluster_entry_offset = fat_offset + (cluster as usize * 4);
        let end_marker: u32 = 0x0FFFFFFF;

        DISK_IMAGE[cluster_entry_offset..cluster_entry_offset + 4]
            .copy_from_slice(&end_marker.to_le_bytes());
    }
}

/// Lie deux clusters dans la table FAT (le premier pointe vers le second).
pub fn link_clusters(fat_offset: usize, cluster: u32, next_cluster: u32) {
    unsafe {
        let cluster_entry_offset = fat_offset + (cluster as usize * 4);
        DISK_IMAGE[cluster_entry_offset..cluster_entry_offset + 4]
            .copy_from_slice(&next_cluster.to_le_bytes());
    }
}
