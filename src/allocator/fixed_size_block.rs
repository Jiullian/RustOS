use alloc::alloc::Layout;
use core::ptr;
use core::{mem, ptr::NonNull};
use super::Locked;
use alloc::alloc::GlobalAlloc;

// Nœud pour la liste chaînée des blocs libres.
struct ListNode {
    next: Option<&'static mut ListNode>,
}

// Les tailles de blocs prédéfinies.
// On utilise des puissances de 2. Toute allocation sera arrondie à la taille supérieure.
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

// L'allocateur à blocs de taille fixe.
pub struct FixedSizeBlockAllocator {
    // Un tableau contenant une liste chaînée pour CHAQUE taille de bloc.
    // list_heads[0] contiendra les blocs libres de 8 octets, list_heads[1] ceux de 16 octets,
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}


impl FixedSizeBlockAllocator {
    // Crée un allocateur vide
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            // Au démarrage, toutes nos listes de blocs sont vides.
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    // Initialise l'allocateur avec la zone mémoire du tas.
    // UNSAFE : L'appelant doit s'assurer que la plage mémoire est valide.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        // On donne l'intégralité du tas au fallback_allocator.
        // C'est lui qui va découper la mémoire vierge au début.
        unsafe { self.fallback_allocator.init(heap_start as *mut u8, heap_size); }
    }

    // Fonction utilitaire pour utiliser l'allocateur de secours de manière simplifiée.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}

// Détermine l'index approprié dans `BLOCK_SIZES` pour un `Layout` donné.
// Retourne `None` si la taille demandée est plus grande que notre plus grand bloc (2048).
fn list_index(layout: &Layout) -> Option<usize> {
    // La taille requise doit satisfaire à la fois la taille demandée et l'alignement.
    let required_block_size = layout.size().max(layout.align());
    // On cherche la première taille dans notre tableau qui est supérieure ou égale au besoin.
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();
        match list_index(&layout) {
            // Cas 1 : La taille correspond à l'un de nos blocs de taille fixe.
            Some(index) => {
                match allocator.list_heads[index].take() {
                    // Chemin rapide : On a déjà un bloc libre de cette taille !
                    Some(node) => {
                        allocator.list_heads[index] = node.next.take();
                        node as *mut ListNode as *mut u8
                    }
                    None => {
                        // On demande à l'allocateur de secours de nous créer un bloc de cette taille exacte.
                        let block_size = BLOCK_SIZES[index];
                        let block_align = block_size;   // On s'assure que l'alignement est au moins égal à la taille.
                        let layout = Layout::from_size_align(block_size, block_align)
                            .unwrap();
                        allocator.fallback_alloc(layout)
                    }
                }
            }
            // Cas 2 : L'allocation est trop grosse (> 2048 octets). On la confie directement au secours.
            None => allocator.fallback_alloc(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        match list_index(&layout) {
            // Cas 1 : C'est un de nos blocs de taille fixe.
            Some(index) => {
                // Au lieu de rendre la mémoire à l'allocateur de secours,
                // on la garde ! On écrit un nouveau nœud `ListNode` au début du bloc libéré
                // et on l'ajoute en tête de la liste correspondant à cette taille.
                let new_node = ListNode {
                    next: allocator.list_heads[index].take(),
                };

                // Vérifications de sécurité : on s'assure que le bloc est assez grand
                // et bien aligné pour stocker un `ListNode`.
                assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
                assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
                let new_node_ptr = ptr as *mut ListNode;
                unsafe {
                    new_node_ptr.write(new_node);
                    allocator.list_heads[index] = Some(&mut *new_node_ptr);
                }
            }
            // Cas 2 : C'était une allocation trop grosse, on la rend à l'allocateur de secours.
            None => {
                let ptr = NonNull::new(ptr).unwrap();
                unsafe {
                    allocator.fallback_allocator.deallocate(ptr, layout);
                }
            }
        }
    }
}