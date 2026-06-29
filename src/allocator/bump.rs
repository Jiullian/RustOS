use super::{align_up, Locked};
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
// Création d'un bump allocator vide

// allocations est le compteur d'allocations actives.
// Initialisé à 0.
// Le but est de réinitialiser l'allocateur une fois que la dernière allocation a été libérée.
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    // Initialisation du bump allocator avec les limites indiquées
    // UNSAFE : On doit s'assurer que la plage mémoire n'est pas utilisée
    // et qu'elle n'est appelée qu'une seule fois.

    // On doit s'assurer que les adresses suivantes sont valides :
    // heap_start : Zone supérieure de la zone mémoire du tas
    // heap_end : Zone inférieure de la zone mémoire du tas

    // next : Pointe en permanence vers le premier octet inutilisée du tas.
    // Initialisée à la valeur heap_start car au début l'intégralité est inutilisée.
    // A chaque allocation next sera incrémenté de la taille de l'allocation
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock();

        let alloc_start = align_up(bump.next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            ptr::null_mut() // out of memory
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock();

        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}