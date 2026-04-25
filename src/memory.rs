use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PhysFrame, Size4KiB,
    },
};

// initialise une nouvelle structure `OffsetPageTable`
// fourni par la librairie x86_64 pour gérer nos tables de pages
// on lui passe l'Offset de notre RAM et il faitles soustractions/additions d'adresses
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

// fonctionqui récupère la racine de la mémoire virtuelle de la table
// renvoie une référence mutable (&mut) vers cette table pour qu'on puisse la modifier (et donc créer de la RAM)
// `unsafe` : L'appelant doit absolument garantir que `physical_memory_offset` est parfait
// sinon notre OS essaierait de lire des zones totalement aléatoires de la vraie RAM et crasherait
pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr> {
    translate_addr_inner(addr, physical_memory_offset)
}

// traduction "Mémoire Virtuelle -> Physique".
// le nom `_inner` montre qu'on met le dans une fonction à part pour exclure le code dangereux (unsafe)
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr> {
    use x86_64::registers::control::Cr3;
    use x86_64::structures::paging::page_table::FrameError;

    // d'abord on lit quel est l'index de la racine (Niveau 4) dans le registre matériel du processeur `CR3`
    let (level_4_table_frame, _) = Cr3::read();

    // découpe notre adresse virtuelle en 4 sous-index (Niveau 4, Nv 3, Nv 2, Nv 1).
    let table_indexes = [
        addr.p4_index(),
        addr.p3_index(),
        addr.p2_index(),
        addr.p1_index(),
    ];
    let mut frame = level_4_table_frame;

    // boucle qui "descend" les niveaux de la table de pages un par un (L4 -> L3 -> L2 -> L1)
    for &index in &table_indexes {
        // on convertit l'adresse brute physique de notre table en adresse virtuelle (via l'offset)
        // ca permet à notre code rust de lire son propre comportement sans déclencher de Page Fault
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe { &*table_ptr };

        // on va chercher la "ligne" correspondante dans la table actuelle, et on met à jour
        // `frame` pour pointer vers la table de niveau inférieur (par exemple on passe du L3 au L2)
        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }

    Some(frame.start_address() + u64::from(addr.page_offset()))
}

// crée un mapping liant une page virtuelle avec l'adresse 0xb8000
// 0xb8000 c'est là qu'est connecté l'écran sur l'ordinateur, ca nous permet d'afficher des pixel
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000)); // Cible: L'écran
    let flags = Flags::PRESENT | Flags::WRITABLE; // Droit de lecture et d'écriture requis

    let map_to_result = unsafe { mapper.map_to(page, frame, flags, frame_allocator) };

    map_to_result.expect("map_to failed").flush();
}

// un allocateur "Bouchon" (Dummy) en attendant de lier la vraie RAM qui renvoie toujours none
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

// gestionnaire de RAM
// il va chercher ses sources dans la Memory Map (la carte physique de l'ordinateur)
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    // initialise notre donneur de trames à partir de la carte (la trame c'est un bloc de 4096 octets)
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    // retourne un "itérateur" pour générer des blocs libres à la chaîne
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // on passe en revue la totalité de la RAM de l'ordinateur
        let regions = self.memory_map.iter();

        // on filtre les zones dangereuses/système pour ne garder que la section `Usable`
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);

        // on récupère les limites  de ces zones libres (ex: de l'octet n° 8000 au 12000)
        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());

        // découpe grossièrement ces plages d'octets en tranches empilées de 4096 octets
        // (4096 octets est la norme  matérielle de pagination x86_64)
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));

        // emballe numériquement ces nombres en  types sécurisés `PhysFrame` utilisables par x86
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

// dès que x86_64 construit un Level 3, Level 2 ou Level 1 de Page Table, il passe par là et appelle `allocate_frame()`
unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        // on demande de piocher le n-ième bloc
        let frame = self.usable_frames().nth(self.next);

        // on incrémente le curseur de +1
        // si on ne faisait pas ça le prochain programme qui appellerait cette fonction écraserait la mémoire allouée du premier car il recevrait le même bloc
        self.next += 1;
        frame
    }
}
