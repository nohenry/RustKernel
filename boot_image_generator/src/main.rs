use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    mem::size_of,
    path::PathBuf,
    str::FromStr,
    vec,
};

use boot_fs::FileHeader;

const DRIVERS: &'static [&str] = &["libfile_system.a", "libpci.a"];
const DRIVER_PATH: &str =
    "D:\\Developement\\Projects\\RustKernel\\target\\x86_64-unknown-linux-musl\\debug";

const CDRIVERS: &'static [&str] = &["driver"];
const CDRIVER_PATH: &str = "D:\\Developement\\Projects\\RustKernel\\drivers-c\\c_driver";

const OTHER: &[&str] =
    &["D:\\Developement\\Projects\\RustKernel\\target\\kernel_target\\debug\\kernel"];

const MAX_FILES: usize = 64;
const HEADER_MAGIC: u16 = 0x6945;

fn main() {
    let files: Vec<PathBuf> = OTHER
        .iter()
        .map(|f| PathBuf::from_str(f).unwrap()) // Make sure kernel is first or everything breaks (I don't want a driver to be loaded as ther kernel :)
        .chain(
            CDRIVERS
                .iter()
                .map(|f| PathBuf::from_str(CDRIVER_PATH).unwrap().join(f)),
        )
        // .chain(
        //     DRIVERS
        //         .iter()
        //         .map(|f| PathBuf::from_str(DRIVER_PATH).unwrap().join(f)),
        // )
        .collect();
    let mut file_headers = vec![];
    let mut offset = size_of::<FileHeader>() * MAX_FILES + size_of::<u32>();

    println!("{:#?}", files);
    for file_path in files.iter() {
        let file = File::open(&file_path).expect("Unable to open file for reading!");
        let len = file.metadata().unwrap().len();

        let p = file_path.file_name().unwrap().to_str().unwrap().as_bytes();
        let mut name_arr = [0u8; 16];
        for b in p.iter().zip(name_arr.iter_mut()) {
            *b.1 = *b.0;
        }

        file_headers.push(FileHeader {
            magic: HEADER_MAGIC,
            name: name_arr,
            file_offset: offset as _,
            file_length: len as _,
        });

        offset += len as usize;
    }

    let output = PathBuf::from_str("boot_image.bin").expect("Unable to create path from strin!");
    let mut output_file = File::create(&output).expect("Unable to open file for writing!");

    let num_headers = file_headers.len() as u16;
    let num_bytes = num_headers.to_ne_bytes();
    output_file
        .write(&num_bytes)
        .expect("Error writing header count!");

    for header in &file_headers {
        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                (header as *const _) as *const u8,
                ::std::mem::size_of::<FileHeader>(),
            )
        };

        output_file
            .write(header_bytes)
            .expect("Unable to write file headers to file!");
    }

    for (file_path, header) in files.iter().zip(file_headers.iter()) {
        let mut file = File::open(&file_path).expect("Unable to open file for reading!");
        let mut buf = vec![];
        file.read_to_end(&mut buf).expect("Unable to read file!");

        output_file
            .seek(SeekFrom::Start(header.file_offset as u64))
            .expect("Unable to seek file for writing!");
        output_file
            .write(buf.as_ref())
            .expect("Unable to write to output file!");
    }

    println!("Hello, world!");
}
