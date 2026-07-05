use super::{align_up, Locked};
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

// Allocateur de type "Bump"

// C'est un allocateur le plus simple possible. Il alloue de la mémoire de manière
// séquentielle en déplaçant le simplement le pointeur 'next'. La libération individuelle
// d'un bloc n'est pas possible. L'intégralité du tas peut être réinitialisée d'un coup
// lorsque la denrière allocation active est libérée.

pub struct BumpAllocator {
    heap_start: usize,              // Adresse de début du tas (borne inférieure)
    heap_end: usize,                // Adresse de fin du tas (borne supérieure)
    next: usize,                    // Pointeur vers le prochain octet disponible
    allocations: usize,             // Compteur du nombre d'allocations actives
}

impl BumpAllocator {
    // Création d'un 'BumpAllocator' vide et non initialisé

    // Les adresses sont configurées à 0.
    // Cette fonction est 'const' afin de pouvoir initialiser l'allocateur de manière statique
    // au démarrage du noyau.
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }
    // Initialisation du bump allocator avec la plage de mémoire spécifiée

    // # Sécurité ('unsafe')
    // Cette fonction est dangereuse. L'appelant doit garantir :
    // 1. Que la plage mémoire (de heap) (de `heap_start` à `heap_start + heap_size`) est valide et libre.
    // 2. Que cette zone n'est pas utilisée ailleurs dans le système pour éviter toute corruption.
    // 3. Que cette méthode n'est appelée qu'une seule et unique fois.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;             // Au début, le tas est entièrement vide : `next` pointe sur le début.
    }
}

// Implémentation du trait GlobalAlloc pour que Rust puisse utiliser cet allocateur pour les types globaux (Vec, Box, etc.).
// Le trait `GlobalAlloc` requiert des méthodes immuables (`&self`), mais l'allocation nécessite de modifier
// l'état interne (`&mut self`). On utilise `Locked` (souvent un Mutex) pour apporter la mutabilité interne requise.
unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    // Alloue un bloc de mémoire en respectant la taille et l'alignement requis par le `Layout`.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // On acquiert le verrou pour manipuler l'allocateur de manière sûre (évite les conditions de concurrence).
        let mut bump = self.lock();

        // 1. Alignement du pointeur `next` selon les exigences du type (ex: alignement sur 4 ou 8 octets).
        // Si `next` est à 0x1003 et que l'alignement requis est 4, `alloc_start` sera poussé à 0x1004.
        let alloc_start = align_up(bump.next, layout.align());

        // 2. Calcul de l'adresse de fin du bloc après l'allocation.
        // Utilisation de `checked_add` pour éviter un dépassement d'entier (integer overflow) si la taille demandée est invalide.
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),                      // En cas d'overflow, on retourne un pointeur nul (échec).
        };

        // 3. Vérification de la place disponible dans le tas (Out of Memory).
        if alloc_end > bump.heap_end {
            ptr::null_mut() // Plus assez d'espace dans le tas, l'allocation échoue.
        } else {
            // 4. Mise à jour des compteurs internes de l'allocateur.
            bump.next = alloc_end;          // Le prochain bloc commencera là où le bloc actuel se termine.
            bump.allocations += 1;          // Une allocation active de plus.
            alloc_start as *mut u8          // On retourne l'adresse de début castée en pointeur brut vers des octets.
        }
    }

    // Libère un bloc de mémoire.
    // Un allocateur Bump ne peut pas libérer un bloc isolé situé au milieu d'autres blocs.
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock();

        // On décrémente le nombre d'allocations actives.
        bump.allocations -= 1;

        // Si le compteur tombe à 0, cela signifie que TOUTES les structures de données du tas ont été libérées.
        // On peut donc réinitialiser l'allocateur et réutiliser l'intégralité du tas depuis le début.
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}