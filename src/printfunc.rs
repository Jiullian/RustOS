//! Module d'interface utilisateur du shell.
//!
//! Ce module gère l'affichage du titre au démarrage, la liste des commandes
//! disponibles, et le routage de l'entrée utilisateur vers la bonne commande.
//! Les commandes FAT (ls, touch, cat, mkdir, rm, rmdir) nécessitent le module
//! `fat` qui sera implémenté séparément.

use crate::disk::read_disk_info;
use crate::{print, println};
use x86_64::instructions::hlt;

// Note : les imports suivants seront activés quand le module `fat` sera implémenté.
// use crate::fat::{ls, touch, cat, mkdir, rm, rmdir};

/// Affiche le titre ASCII "ROS" et le message d'accueil au démarrage du système.
pub fn title() {
    // Art ASCII représentant "ROS"
    println!(r"  _____    ____    _____ ");
    println!(r" |  __ \  / __ \  / ____|");
    println!(r" | |__) || |  | || (___  ");
    println!(r" |  _  / | |  | | \___ \ ");
    println!(r" | | \ \ | |__| | ____) |");
    println!(r" |_|  \_\ \____/ |_____/ ");

    // Petite pause visuelle après l'affichage du titre
    for _ in 0..5 {
        hlt();
    }

    println!("        RustOS Shell Terminal      ");

    // Pause plus longue avant le message d'aide
    for _ in 0..10 {
        hlt();
    }

    // Indication pour l'utilisateur
    println!("'help' -> Pour voir toutes les commandes\n");
}

/// Affiche la liste de toutes les commandes disponibles dans le shell.
fn help() {
    println!("\nListe des commandes :");
    println!("help   -> Affiche ce message");
    println!("info   -> Affiche les details de l'image disque");
    println!("ls     -> Liste les fichiers et repertoires");
    println!("touch  -> Cree un fichier (usage: touch <nom> <contenu>)");
    println!("cat    -> Lit le contenu d'un fichier (usage: cat <nom>)");
    println!("mkdir  -> Cree un dossier (usage: mkdir <nom>)");
    println!("rm     -> Supprime un fichier (usage: rm <nom>)");
    println!("rmdir  -> Supprime un dossier (usage: rmdir <nom>)");
}

/// Vérifie l'entrée utilisateur et exécute la commande correspondante.
///
/// # Arguments
/// * `input` - La chaîne saisie par l'utilisateur dans le shell.
///
/// Les commandes FAT (ls, touch, cat, mkdir, rm, rmdir) sont préparées
/// mais nécessitent l'implémentation du module `fat` pour fonctionner.
pub fn verif_message(input: &str) {
    // ===== Commandes simples (sans arguments) =====

    // Commande : help — affiche la liste des commandes
    if input == "help" {
        help();
        print!("\n> ");
    }

    // Commande : info — affiche les informations de l'image disque
    if input == "info" {
        read_disk_info();
        print!("\n> ");
    }

    // ===== Commandes FAT=====

    // Commande : ls — liste les fichiers (sans argument)
    if input == "ls" {
        // TODO: décommenter quand le module fat sera prêt
        // ls();
        println!("[fat] commande 'ls' pas encore implementee");
        print!("\n> ");
    }

    // Commandes sans argument : affichent un message d'erreur d'usage
    if input == "touch" {
        println!("Erreur: usage -> touch <nom_fichier> <contenu>");
        print!("\n> ");
    }
    if input == "cat" {
        println!("Erreur: usage -> cat <nom_fichier>");
        print!("\n> ");
    }
    if input == "mkdir" {
        println!("Erreur: usage -> mkdir <nom_dossier>");
        print!("\n> ");
    }
    if input == "rm" {
        println!("Erreur: usage -> rm <nom_fichier>");
        print!("\n> ");
    }
    if input == "rmdir" {
        println!("Erreur: usage -> rmdir <nom_dossier>");
        print!("\n> ");
    }

    // ===== Commandes FAT avec arguments =====

    // Commande : touch <nom> <contenu> — crée un fichier avec du contenu
    if input.starts_with("touch ") {
        // On retire le préfixe "touch " pour récupérer les arguments
        let input_trimmed = &input[6..];

        // On cherche le premier espace pour séparer le nom du fichier et son contenu
        if let Some(space_index) = input_trimmed.find(' ') {
            let _file_name = &input_trimmed[..space_index];
            let _file_content = &input_trimmed[space_index + 1..];

            // TODO: décommenter quand le module fat sera prêt
            // unsafe {
            //     touch(file_name, file_content.as_bytes());
            //     print!("\n> ");
            // }
            println!("[fat] commande 'touch' pas encore implementee");
            print!("\n> ");
        } else {
            // Pas assez d'arguments fournis
            println!("Erreur: usage -> touch <nom_fichier> <contenu>");
            print!("\n> ");
        }
    }

    // Commande : cat <nom> — lit le contenu d'un fichier
    if input.starts_with("cat ") {
        // On retire le préfixe "cat " pour récupérer le nom du fichier
        let input_trimmed = &input[4..];

        if !input_trimmed.is_empty() {
            let _file_name = input_trimmed.trim();
            // TODO: décommenter quand le module fat sera prêt
            // unsafe {
            //     cat(file_name);
            //     print!("\n> ");
            // }
            println!("[fat] commande 'cat' pas encore implementee");
            print!("\n> ");
        } else {
            println!("Erreur: usage -> cat <nom_fichier>");
            print!("\n> ");
        }
    }

    // Commande : mkdir <nom> — crée un dossier
    if input.starts_with("mkdir ") {
        let dir_name = input[6..].trim();
        if !dir_name.is_empty() {
            // TODO: décommenter quand le module fat sera prêt
            // unsafe {
            //     mkdir(dir_name);
            //     print!("\n> ");
            // }
            println!("[fat] commande 'mkdir' pas encore implementee");
            print!("\n> ");
        } else {
            println!("Erreur: usage -> mkdir <nom_dossier>");
            print!("\n> ");
        }
    }

    // Commande : rm <nom> — supprime un fichier
    if input.starts_with("rm ") {
        let name = input[3..].trim();
        if !name.is_empty() {
            // TODO: décommenter quand le module fat sera prêt
            // unsafe {
            //     rm(name);
            //     print!("\n> ");
            // }
            println!("[fat] commande 'rm' pas encore implementee");
            print!("\n> ");
        } else {
            println!("Erreur: usage -> rm <nom_fichier>");
            print!("\n> ");
        }
    }

    // Commande : rmdir <nom> — supprime un dossier
    if input.starts_with("rmdir ") {
        let name = input[6..].trim();
        if !name.is_empty() {
            // TODO: décommenter quand le module fat sera prêt
            // unsafe {
            //     rmdir(name);
            //     print!("\n> ");
            // }
            println!("[fat] commande 'rmdir' pas encore implementee");
            print!("\n> ");
        } else {
            println!("Erreur: usage -> rmdir <nom_dossier>");
            print!("\n> ");
        }
    }
}
