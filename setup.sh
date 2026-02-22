#!/bin/bash

##########
# COLORS #
##########

declare -A color
declare -A effect

color['reset']="\e[0m"
color['red']="\e[31m"
color['green']="\e[32m"
color['blue']="\e[34m"
color['cyan']="\e[36m"
color['yellow']="\e[33m"

effect['bold']="\e[1m"

echo -e "${effect['bold']}${color['cyan']}Installation des dépendances nécessaires${color['reset']}"
sudo apt update && sudo apt install curl -y

echo -e "${effect['bold']}${color['yellow']}Installation linker CC...${color['reset']}"
sudo apt install build-essential

echo -e "${effect['bold']}${color['yellow']}Installation rustup...${color['reset']}"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

echo -e "${effect['bold']}${color['yellow']}Ajout du path...${color['reset']}"
source $HOME/.cargo/env

echo -e "${effect['bold']}${color['yellow']}Installation de la bonne version de nightly pour le projet...${color['reset']}"
rustup install nightly-2025-01-21

echo -e "${effect['bold']}${color['yellow']}Installation des llvm-tools...${color['reset']}"
rustup component add llvm-tools-preview

echo -e "${effect['bold']}${color['yellow']}ajout du rust-src...${color['reset']}"
rustup component add rust-src --toolchain nightly-2025-01-21-x86_64-unknown-linux-gnu

echo -e "${effect['bold']}${color['yellow']}Installation de bootimage...${color['reset']}"
cargo install bootimage

echo -e "${effect['bold']}${color['yellow']}Installation de qemu...${color['reset']}"
sudo apt install qemu-system-x86

echo -e "${effect['bold']}${color['yellow']}Donne les droits d'execution au fichier run.sh...${color['reset']}"
chmod +x run.sh

echo -e "${effect['bold']}${color['green']}Installation fini${color['reset']}"

echo -e "${effect['bold']}${color['red']}[IMPORTANT] - redemarrer le terminal avant d'executer les indications suivante :${color['reset']}"

echo -e "${effect['bold']}${color['yellow']}Pour construire le projet:${color['reset']}"
echo -e "${effect['bold']}${color['blue']}cargo bootimage${color['reset']}"
echo -e "${effect['bold']}${color['yellow']}Pour l'exécuter :${color['reset']}"
echo -e "${effect['bold']}${color['blue']}qemu-system-x86_64 -drive format=raw,file=target/x86_64/debug/bootimage-RustOS.bin${color['reset']}"
echo -e "${effect['bold']}${color['yellow']}Ou :${color['reset']}"
echo -e "${effect['bold']}${color['blue']}./run.sh${color['reset']}"