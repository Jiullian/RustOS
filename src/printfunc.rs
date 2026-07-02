//! Module d'interface utilisateur du shell avec les commandes possible
//!
//! gère l'affichage du titre au démarrage, la liste des commandes
//! disponibles, et le routage de l'entrée utilisateur vers la bonne commande.

use crate::disk::read_disk_info;
use crate::{print, println};
use x86_64::instructions::hlt;

// Import des commandes FAT disponibles (les autres seront décommentées plus tard)
use crate::fat::ls;

/// Affiche le titre ASCII et le message d'accueil au boot du système.
pub fn title() {
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
    print!("> ");
}

/// Liste des commandes du shell.
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
/// vérification de l'entré utilisateur er execution de la commande qui correspond
pub fn verif_message(input: &str) {
    let input = input.trim();

    // Si l'entrée est vide (appui simple sur Entrée), on réaffiche juste le prompt
    if input.is_empty() {
        print!("> ");
        return;
    }

    let mut command_found = true;

    // ===== Commandes simples (sans arguments) =====

    // ===== Commandes simples =====

    if input == "help" {
        help();
    }
    // Commande : info — affiche les informations de l'image disque
    else if input == "info" {
        read_disk_info();
    }
    // ===== Commandes FAT (sans arguments) =====

    // Commande : ls — liste les fichiers du répertoire racine
    else if input == "ls" {
        ls();
    }
    // Commandes sans argument : affichent un message d'erreur d'utilisation de la commande
    else if input == "touch" {
        println!("Erreur: usage -> touch <nom_fichier> <contenu>");
    }
    else if input == "cat" {
        println!("Erreur: usage -> cat <nom_fichier>");
    }
    else if input == "mkdir" {
        println!("Erreur: usage -> mkdir <nom_dossier>");
    }
    else if input == "rm" {
        println!("Erreur: usage -> rm <nom_fichier>");
    }
    else if input == "rmdir" {
        println!("Erreur: usage -> rmdir <nom_dossier>");
    }

    // ===== Commandes FAT avec arguments =====

    // Commande : touch <nom> <contenu> — crée un fichier avec du contenu
    else if input.starts_with("touch ") {
        // On retire le préfixe "touch " pour récupérer les arguments
        let input_trimmed = &input[6..];

        // On cherche le premier espace pour séparer le nom du fichier et son contenu
        if let Some(space_index) = input_trimmed.find(' ') {
            let _file_name = &input_trimmed[..space_index];
            let _file_content = &input_trimmed[space_index + 1..];

            // TODO: décommenter quand le module fat sera fait
            // unsafe {
            //     touch(file_name, file_content.as_bytes());
            // }
            println!("[fat] commande 'touch' pas encore implementee");
        } else {
            // Pas assez d'arguments fournis
            println!("Erreur: usage -> touch <nom_fichier> <contenu>");
        }
    }
    // Commande : cat <nom> — lit le contenu d'un fichier
    else if input.starts_with("cat ") {
        // On retire le préfixe "cat " pour récupérer le nom du fichier
        let input_trimmed = &input[4..];

        if !input_trimmed.is_empty() {
            let _file_name = input_trimmed.trim();
            // TODO: décommenter quand le module fat sera fait
            // unsafe {
            //     cat(file_name);
            // }
            println!("[fat] commande 'cat' pas encore implementee");
        } else {
            println!("Erreur: usage -> cat <nom_fichier>");
        }
    }
    // Commande : mkdir <nom> — crée un dossier
    else if input.starts_with("mkdir ") {
        let dir_name = input[6..].trim();
        if !dir_name.is_empty() {
            // TODO: décommenter quand le module fat sera fait
            // unsafe {
            //     mkdir(dir_name);
            // }
            println!("[fat] commande 'mkdir' pas encore implementee");
        } else {
            println!("Erreur: usage -> mkdir <nom_dossier>");
        }
    }
    // Commande : rm <nom> — supprime un fichier
    else if input.starts_with("rm ") {
        let name = input[3..].trim();
        if !name.is_empty() {
            // TODO: décommenter quand le module fat sera fait
            // unsafe {
            //     rm(name);
            // }
            println!("[fat] commande 'rm' pas encore implementee");
        } else {
            println!("Erreur: usage -> rm <nom_fichier>");
        }
    }
    // Commande : rmdir <nom> — supprime un dossier
    else if input.starts_with("rmdir ") {
        let name = input[6..].trim();
        if !name.is_empty() {
            // TODO: décommenter quand le module fat sera fait
            // unsafe {
            //     rmdir(name);
            // }
            println!("[fat] commande 'rmdir' pas encore implementee");
        } else {
            println!("Erreur: usage -> rmdir <nom_dossier>");
        }
    }
    // Si aucune commande n'a correspondu, on le signale
    else {
        println!("Commande inconnue : '{}'", input);
    }

    // Affichage systématique du prompt pour la commande suivante
    print!("\n> ");
}
