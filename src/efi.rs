use core::sync::atomic::AtomicPtr;

use crate::kprintln;

pub type Char16 = u16;
pub type Handle = usize;

const BUFFER_TOO_SMALL: usize = 5 | (1 << 63);

#[repr(C)]
struct TableHeader {
    signature: u64,
    revision: u32,
    size: u32,
    crc: u32,
    res: u32,
}

#[repr(C)]
pub struct SystemTable {
    header: TableHeader,
    vendor: *const Char16,
    revision: u32,
    console_in_handle: Handle,
    console_in: *const u8,
    console_out_handle: Handle,
    console_out: Handle,
    console_error_handle: Handle,
    console_error: *const u8,
    runtime_services: *const u8,
    boot_services: *const BootServices,
    entry_count: usize,
    configuration_table: *const  ConfigurationTable,
}

impl SystemTable {
    pub fn config_tables(&self) -> ConfigurationTableIterator {
        ConfigurationTableIterator::new(self.configuration_table, self.entry_count)
    }
}

struct ConfigurationTable {
    guid: guid::GUID,
    ptr: *const ()
}

pub struct ConfigurationTableIterator {
    configuration_base: *const ConfigurationTable,
    size: usize,
    index: usize,
}

impl  ConfigurationTableIterator {
    fn new(configuration_base: *const ConfigurationTable, size: usize) -> ConfigurationTableIterator {
        ConfigurationTableIterator {
            configuration_base,
            size,
            index: 0
        }
    }
}

impl Iterator for ConfigurationTableIterator {
    type Item = (&'static guid::GUID, *const ());

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.size {
            let ret = unsafe {
                let table = &*self.configuration_base.offset(self.index as _);
                Some((&table.guid, table.ptr))
            };
            self.index += 1;
            ret
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct BootServices {
    header: TableHeader,

    /*
    Task Priority Services
    */
    raise_tpl: Handle,
    restore_tple: Handle,

    /*
    Memory Services
     */
    allocate_pages: Handle,
    free_pages: Handle,
    get_memory_map: fn(&mut usize, *mut u8, &mut usize, &mut usize, &mut u32) -> usize,
    // fn(&mut usize, &mut [MemoryDescriptor], &mut usize, &mut usize, &mut u32) -> usize,
    allocate_pool: Handle,
    free_pool: Handle,

    /*
    Event & Timer Services
     */
    create_event: Handle,
    set_timer: Handle,
    wait_for_event: Handle,
    signal_event: Handle,
    close_event: Handle,
    check_event: Handle,

    /*
    Protocol Handler Services
     */
    install_protocol_interface: Handle,
    reinstall_protocol_interface: Handle,
    uninstall_protocol_interface: Handle,
    handle_protocol: fn(Handle, *const guid::GUID, *mut *const LoadedImage) -> usize,
    reserved: usize,
    register_protocol_notify: Handle,
    locate_handle: Handle,
    locate_device_path: Handle,
    install_configuration_table: Handle,

    /*
    Image services
     */
    image_load: Handle,
    start_image: Handle,
    exit: Handle,
    image_unload: Handle,
    exit_boot_services: fn(Handle, usize) -> usize,
}

#[repr(C)]
pub struct LoadedImage {
    revision: u32,
    parent_handle: Handle,
    system_table: *const SystemTable,

    device_handle: Handle,
    file_path: Handle,
    reserved: *const (),

    load_options_size: u32,
    load_options: *const (),

    image_base: *const (),
    image_size: usize,
    image_code_type: MemoryType,
    image_data_type: MemoryType,
    unload: Handle,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryType {
    Reserved,
    LoaderCode,
    LoaderData,
    BootServicesCode,
    BootServicesData,
    RuntimeServicesCode,
    RuntimeServicesData,
    Conventional,
    Unusable,
    ACPIReclaim,
    ACPINVS,
    MemoryMappedIO,
    MemoryMappedIOPortSpace,
    PalCode,
    PersistentMemory,
    MaxMemoryType,
}

impl MemoryType {
    pub fn is_usable(&self) -> bool {
        match self {
            // Self::BootServicesCode
            // | Self::BootServicesData
            // | Self::PersistentMemory
            Self::Conventional => true,
            _ => false,
        }
    }
}

impl MemoryType {
    fn as_u8(&self) -> u32 {
        *self as u32
    }
}

impl Default for MemoryType {
    fn default() -> Self {
        MemoryType::Reserved
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryDescriptor {
    pub memory_type: MemoryType,
    pub physical_address: usize,
    pub virtual_address: usize,
    pub size: usize,
    pub attributes: u64,
    r1: u64,
    // r2: u32,
}

pub type MemoryMap = &'static [MemoryDescriptor];

// #[repr(C)]
// pub struct SimpleTextOutputProtocol {
//     reset: fn(*mut Self),
//     output_string: fn(*mut Self, *const u16),
//     test_string: fn(&Self),
//     query_mode: fn(&Self),
//     set_mode: fn(&Self),
//     set_attribute: fn(&Self),
//     clear_screen: fn(&Self),
//     set_cursor_position: fn(&Self),
//     enable_cursor: fn(&Self),
//     mode: *const u8,
// }

pub static GLOBAL_SYSTEM_TABLE: AtomicPtr<SystemTable> = AtomicPtr::new(core::ptr::null_mut());

pub unsafe fn register_global_system_table(
    table: *mut SystemTable,
) -> Result<*mut SystemTable, *mut SystemTable> {
    GLOBAL_SYSTEM_TABLE.compare_exchange(
        core::ptr::null_mut(),
        table,
        core::sync::atomic::Ordering::SeqCst,
        core::sync::atomic::Ordering::SeqCst,
    )
}

// pub fn output(string: &str) {
//     let buff = ['a' as char16 ; 5];
//     let table = GLOBAL_SYSTEM_TABLE.load(core::sync::atomic::Ordering::SeqCst);

//     if table.is_null() {
//         return;
//     }

//     let out = unsafe { (*table).console_out };

//     unsafe {
//         ((*out).output_string)(out, buff.as_ptr());
//     }
// }
pub static mut DESCRIPTORS: [MemoryDescriptor; 1024] = [MemoryDescriptor {
    attributes: 0,
    memory_type: MemoryType::Reserved,
    physical_address: 0,
    r1: 0,
    size: 0,
    virtual_address: 0,
}; 1024];

pub fn get_memory_map(image_handle: Handle) -> MemoryMap {
    let table = GLOBAL_SYSTEM_TABLE.load(core::sync::atomic::Ordering::SeqCst);

    unsafe {
        let mut size = core::mem::size_of_val(&DESCRIPTORS);
        let mut key = 0;
        let mut mdesc_size = 0;
        let mut mdesc_version = 0;

        let result = ((*(*table).boot_services).get_memory_map)(
            &mut size,
            DESCRIPTORS.as_mut_ptr() as *mut u8,
            &mut key,
            &mut mdesc_size,
            &mut mdesc_version,
        );

        assert!(result == 0, " {:x?} {:x}", result, BUFFER_TOO_SMALL);

        let mut conventional = 0;
        let mut all = 0;

        for desc in &DESCRIPTORS {
            if desc.physical_address == 0 && desc.virtual_address == 0 && desc.size == 0 {
                break;
            }

            if desc.memory_type.is_usable() {
                all += desc.size * 4096;
            }
            if let MemoryType::Conventional = desc.memory_type {
                conventional += desc.size * 4096;
            }

            kprintln!(
                "{:016x} {:016x} {:?}",
                desc.physical_address,
                desc.size * 4096,
                desc.memory_type
            );
            // kprintln!("{:x?}", desc);
        }

        let result = ((*(*table).boot_services).exit_boot_services)(image_handle, key);
        assert!(result == 0, "Unable to exit boot services! {:x}", result);
        kprintln!("Exited boot services!");
        return &DESCRIPTORS;
    }
}

pub fn get_image_base(image_handle: Handle) -> usize {
    let table = GLOBAL_SYSTEM_TABLE.load(core::sync::atomic::Ordering::SeqCst);

    let mut loaded_image: *const LoadedImage = core::ptr::null();
    unsafe {
        let res = ((*(*table).boot_services).handle_protocol)(
            image_handle,
            &guid::LOADED_IMAGE_PROTOCOL,
            &mut loaded_image,
        );
        if res != 0 {
            kprintln!("An error occured! {:x}", res);
        }
        (*loaded_image).image_base as _
    }
}

pub mod guid {
    
    pub use macros::create_guid;

    #[derive(PartialEq)]
    pub struct GUID {
        a: u32,
        /// The middle field of the timestamp.
        b: u16,
        /// The high field of the timestamp multiplexed with the version number.
        c: u16,
        /// Contains, in this order:
        /// - The high field of the clock sequence multiplexed with the variant.
        /// - The low field of the clock sequence.
        /// - The spatially unique node identifier.
        d: [u8; 8],
    }

    impl <'a> PartialEq<GUID> for &'a GUID {
        fn eq(&self, other: &GUID) -> bool {
            self.a == other.a && self.b == other.b && self.c == other.c && self.d == other.d
        }
    }

    pub const LOADED_IMAGE_PROTOCOL: GUID = create_guid!(5B1B31A1-9562-11d2-8E3F-00A0C969723B);

    pub const RSDP: GUID = create_guid!(8868E871-E4F1-11D3-BC22-0080C73C8881);
}
