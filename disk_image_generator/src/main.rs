use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, PathBuf},
    str::FromStr,
};

use fatfs::{format_volume, FormatVolumeOptions};
use fscommon::BufStream;
use iso::option::{ElToritoOpt, Opt};

mod iso;

const IN_FILES: &[&str] = &[
    "D:\\Developement\\Projects\\RustKernel\\target\\x86_64-unknown-uefi\\debug\\kernel_loader.efi",
];
// "D:\\Developement\\Projects\\RustKernel\\boot_image_generator\\boot_image.bin"
// "efi/boot/btimg.bin"
const OUT_FILES: &[&str] = &["efi/boot/bootx64.efi"];

fn main() -> io::Result<()> {
    env_logger::init();

    let img_file_path =
        PathBuf::from_str("misc/boot/kernel.img").expect("Unable to create image file path!");
    let out_file =
        PathBuf::from_str("misc/kernel.iso").expect("Unable to create output file path!");

    {
        let img_file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&img_file_path)
        {
            Ok(file) => file,
            Err(err) => {
                println!("Failed to open image!");
                return Err(err);
            }
        };

        let bytes_per_sector = 512u16;
        let mut total_sectors = 5000u32;

        for file in IN_FILES.iter() {
            let from_file = File::open(file).expect("Unable to open file for reading!");
            let len = from_file.metadata().unwrap().len();
            total_sectors += (len / bytes_per_sector as u64 + 1) as u32;
        }

        let buffer = BufStream::new(&img_file);
        format_volume(
            buffer,
            FormatVolumeOptions::new()
                .fat_type(fatfs::FatType::Fat32)
                .total_sectors(total_sectors)
                .bytes_per_sector(bytes_per_sector),
        )?;

        let buffer = BufStream::new(&img_file);
        let options = fatfs::FsOptions::new();
        let fs = fatfs::FileSystem::new(buffer, options).unwrap();
        let root_dir = fs.root_dir();

        for file in IN_FILES.iter().zip(OUT_FILES.iter()) {
            let from = PathBuf::from_str(file.0).unwrap();
            let to = PathBuf::from_str(file.1).unwrap();

            let mut file_buffer = Vec::new();
            let mut from_file = File::open(from).expect("Unable to open file for reading!");
            from_file
                .read_to_end(&mut file_buffer)
                .expect("Error reading file!");

            let mut current = root_dir.clone();
            let comps = to.components();
            let last = comps.clone().last().unwrap();

            for part in comps {
                if part == last {
                    break;
                }
                match &part {
                    Component::Normal(s) => {
                        current = current
                            .create_dir(s.to_str().unwrap())
                            .expect(format!("Unable to create file {}!", to.display()).as_str());
                    }
                    _ => (),
                }
            }
            let mut file = current
                .create_file(to.file_name().unwrap().to_str().unwrap())
                .unwrap();

            file.write_all(file_buffer.as_slice())?;
        }
    }

    let mut opts = Opt {
        output: out_file.to_path_buf(),
        eltorito_opt: ElToritoOpt {
            eltorito_boot: Some(String::from(
                img_file_path.file_name().unwrap().to_str().unwrap(),
            )),
            no_emu_boot: true,
            grub2_boot_info: false,
            no_boot: false,
            boot_info_table: false,
        },
        embedded_boot: None,
        grub2_mbr: None,
        boot_load_size: 4548,
        protective_msdos_label: false,
        input_files: vec![img_file_path],
    };

    iso::create_iso(&mut opts)?;

    Ok(())
}
