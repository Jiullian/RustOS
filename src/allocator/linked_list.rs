use super::align_up;
use core::mem;
use super::Locked;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;
// Représente un bloc de mémoire libre.
struct ListNode {
    size: usize,                            // Taille de ce bloc de mémoire libre
    next: Option<&'static mut ListNode>,    // Pointeur vers le bloc libre suivant.
}

// L'allocateur lui-même. Il ne contient qu'un nœud "tête" factice
// qui pointe vers le premier vrai bloc de mémoire libre.
pub struct LinkedListAllocator {
    head: ListNode,
}

impl ListNode {
    // Crée un nouveau nœud représentant un bloc libre de taille `size`.
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    // Retourne l'adresse mémoire de début de ce bloc.
    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    // Retourne l'adresse mémoire de fin de ce bloc (exclue).
    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

impl LinkedListAllocator {
    // Crée un allocateur vide. La tête a une taille de 0 et aucun suivant.
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    // Initialise l'allocateur avec toute la mémoire du tas (heap).
    // UNSAFE : L'appelant doit garantir que la plage de mémoire est valide et non utilisée.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        // Au démarrage, toute la mémoire du tas est considérée comme un seul grand bloc libre.
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }

    // Ajoute un nouveau bloc de mémoire libre à l'avant de la liste chaînée.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // On s'assure que l'adresse est bien alignée pour pouvoir y stocker un `ListNode`.
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        // Le bloc doit être au moins assez grand pour contenir la structure `ListNode` elle-même.
        assert!(size >= mem::size_of::<ListNode>());

        // On crée un nouveau nœud et on l'insère en tête de liste (juste après `self.head`).
        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        unsafe {
            // On écrit physiquement la structure `ListNode` au début du bloc mémoire libre.
            node_ptr.write(node);
            self.head.next = Some(&mut *node_ptr)
        }
    }

    // Cherche un bloc libre assez grand et avec le bon alignement.
    // Retourne le nœud trouvé, son adresse de début d'allocation, et le retire de la liste.
    fn find_region(&mut self, size: usize, align: usize)
                   -> Option<(&'static mut ListNode, usize)>
    {
        // On parcourt la liste chaînée (stratégie "First Fit" : on prend le premier qui rentre).
        let mut current = &mut self.head;
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // Le bloc convient ! On le retire de la liste chaînée.
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // Le bloc ne convient pas, on passe au suivant.
                current = current.next.as_mut().unwrap();
            }
        }
        None    // Aucun bloc de taille suffisante n'a été trouvé.
    }

    // Vérifie si une région donnée est assez grande pour satisfaire l'allocation.
    fn alloc_from_region(region: &ListNode, size: usize, align: usize)
                         -> Result<usize, ()>
    {
        // On calcule l'adresse de début en respectant l'alignement demandé.
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        // Si la fin de l'allocation dépasse la fin du bloc, c'est trop petit.
        if alloc_end > region.end_addr() {
            return Err(());
        }

        // IMPORTANT : S'il reste de la place, cet espace restant DOIT être assez grand
        // pour stocker un nouveau `ListNode` (pour redevenir un bloc libre).
        // S'il est trop petit pour un `ListNode` mais > 0, on rejette cette région pour éviter
        // de créer un "trou" de mémoire inutilisable.
        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }

    // Ajuste le `Layout` (taille et alignement) pour qu'un bloc alloué puisse TOUJOURS
    // contenir un `ListNode` une fois qu'il sera libéré.
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        // La taille finale est au minimum la taille de `ListNode`.
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}

// Implémentation de l'interface d'allocation globale de Rust
unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // 1. On ajuste la taille et l'alignement requis.
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        // 2. On cherche un bloc libre approprié.
        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;

            // 3. Si le bloc trouvé est plus grand que nécessaire,
            // on fragmente : on remet l'excédent dans la liste des blocs libres.
            if excess_size > 0 {
                unsafe {
                    allocator.add_free_region(alloc_end, excess_size);
                }
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // 1. On recalcule la même taille ajustée qu'à l'allocation.
        let (size, _) = LinkedListAllocator::size_align(layout);

        // 2. On rajoute simplement le bloc mémoire à la liste des régions libres.
        unsafe { self.lock().add_free_region(ptr as usize, size) }
    }
}