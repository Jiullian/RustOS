use linked_list_allocator::LockedHeap;
use bump::BumpAllocator;
use linked_list::LinkedListAllocator;
#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};
pub struct Dummy;

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}
pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024;
pub mod bump;
pub mod linked_list;

fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr
    } else {
        addr - remainder + align
    }
}

// init_heap prend 2 paramètres :
// - mapper : l'outil qui modifie les tables de pages (pour relier virtuel -> physique).
// - frame_allocator : l'outil qui trouve des blocs libres de 4 Ko dans la RAM.
// Retours :
// - Ok(())
// - MapToError
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // Définition de la plage de pages virtuelles à utiliser pour le tas.
    let page_range = {
        // On convertit l'adresse de départ brute en une adresse virtuelle sécurisée.
        let heap_start = VirtAddr::new(HEAP_START as u64);
        // On calcule l'adresse exacte du dernier octet du tas.
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        // On trouve à quelle page (bloc de 4 Ko) appartient l'adresse de départ.
        let heap_start_page = Page::containing_address(heap_start);
        // On fait la même chose pour l'adresse de fin.
        let heap_end_page = Page::containing_address(heap_end);
        // On crée un itérateur qui contient toutes les pages entre le début et la fin.
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // Pour chaque page virtuelle requise par notre tas
    for page in page_range {
        // On demande à notre allocateur une frame (case mémoire physique de 4 Ko).
        // RAM Peine : on renvoie l'erreur FrameAllocationFailed.
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        // Définition des droits d'accès pour cette mémoire
        // PRESENT : Chargée en mémoire
        // WRITABLE : Droit d'écriture
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        // Le mappage effectif : opération unsafe car on touche au matériel (risquée).
        unsafe {
            // On ordonne au mapper de lier la page virtuelle à la frame physique avec nos flags.
            mapper.map_to(page, frame, flags, frame_allocator)?.flush()
        };
    }

    unsafe {
        // ALLOCATOR.lock() : Empêche que deux parties du noyau n'essaient de modifier l'allocateur en même temps.
        // init() : initialisation de la région mémoire.
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}
unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc should be never called")
    }
}