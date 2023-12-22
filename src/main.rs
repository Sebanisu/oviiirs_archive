use std::{fs, io, path::PathBuf, process::exit};

use itertools::Itertools;
use oviiirs_archive::oviiirs_archive::*;
mod lzss;
use typed_path::{Utf8NativeEncoding, Utf8NativePath, Utf8WindowsPath};

fn main() -> io::Result<()> {
    let config_path: String = "config.toml".to_string();

    let mut config = load_config_from_file(&config_path)?;

    config.locations.ensure_chosen_directory_in_directories();

    let directories = filter_valid_directories(&config.locations.directories);

    let user_choice = display_directory_info(&directories, &config.locations.chosen_directory);

    config.locations.chosen_directory = match user_choice {
        DirectorySelection::NewDirectory(new_dir) => {
            // Handle the case when a new directory is chosen
            println!("New directory selected: {}", new_dir);
            config.locations.directories.push(new_dir.clone());

            // Sort the directories
            config.locations.directories.sort();

            // Deduplicate the sorted list
            config.locations.directories.dedup();
            new_dir
        }
        DirectorySelection::ExistingDirectory(existing_dir) => {
            // Handle the case when an existing directory is chosen
            println!("Existing directory selected: {}", existing_dir);
            // Remember the chosen directory and do something for existing directory
            existing_dir
        }
        DirectorySelection::Exit => {
            // Handle the case when the user chooses to exit
            println!("Exiting...");
            // Perform any necessary cleanup and exit the program
            // You can return a default value here or use a placeholder value
            exit(0);
        }
    };

    save_config(&config, &config_path)?;

    let zzz_files = process_files_in_directory(&config.locations.chosen_directory)?;

    for zzz_file in zzz_files {
        let data = read_data_from_file(&zzz_file)?;
        let archives = find_archives(data.entries.clone(), &zzz_file);

        //begin create toml of data
        save_config(&data, &generate_zzz_filename(&zzz_file))?;

        extract_archives(
            archives
                .iter()
                .filter(|&item| item.archive_type != ArchiveType::Field),
        )?;

        let result = archives
            .iter()
            .find(|&item| item.archive_type == ArchiveType::Field);
        match result {
            Some(found_element) => {
                // Do something with the found element
                println!("Found: {:?}", found_element);
                let field_archives = find_archives_field(found_element)?;
                extract_archives(field_archives.iter())?;
            }
            None => {
                // Handle the case when no element with ArchiveType::Field is found
                println!("Element with ArchiveType::Field not found");
            }
        }
    }

    Ok(())
}

// fn extract_archives<I>(archives: I) -> io::Result<()>
// where
//     I: Iterator<Item = FIFLFSZZZ>,

fn extract_archives<'a, I>(archives: I) -> io::Result<()>
where
    I: Iterator<Item = &'a FIFLFSZZZ>,
{
    for archive in archives {
        save_config(
            &archive,
            &generate_new_filename_custom_extension(
                &Utf8WindowsPath::new(&archive.fi.string_data),
                "fiflfs_zzz",
            ),
        )?;
        let fi_file = read_fi_entries_from_file(&archive.fi, &archive.file_path)?;
        save_config(&fi_file, &generate_new_filename(&archive.fi.string_data))?;

        let fl_file = read_fl_entries_from_file(&archive.fl, &archive.file_path)?;
        save_config(&fl_file, &generate_new_filename(&&archive.fl.string_data))?;

        for fi_eob in fi_file
            .entries
            .iter()
            .zip(fl_file.entries.iter())
            .filter(|(fi, _)| fi.uncompressed_size != 0)
            .zip_longest(
                fi_file
                    .entries
                    .iter()
                    .skip(1)
                    .filter(|fi| fi.uncompressed_size != 0),
            )
        {
            let (fi, fl) = fi_eob.as_ref().left().unwrap();

            let compressed_size: u64 = match fi_eob.as_ref().right() {
                Some(next_fi) => next_fi.offset as u64 - fi.offset as u64,
                None => archive.fs.file_size as u64 - fi.offset as u64,
            };

            let zzz_offset = archive.fs.file_offset + fi.offset as u64;
            let zzz_size = fi.uncompressed_size as u64;
            let windows_file_path = Utf8WindowsPath::new(fl);
            let relative_windows_file_path = match windows_file_path.strip_prefix("c:\\") {
                Ok(p) => p,
                Err(_) => windows_file_path,
            };
            let extract_path = Utf8NativePath::new("test");
            let native_file_path = relative_windows_file_path.with_encoding::<Utf8NativeEncoding>();
            //let file_path = Path::new(native_file_path.as_str());
            let new_extract_path = PathBuf::from(extract_path.join(native_file_path).as_str());
            println!("FI: {:?}", fi);
            println!("FL: {:?}", fl);
            println!(
                "zzz Offset: {}, zzz size {}, compressed size {}, relative path {}",
                zzz_offset,
                zzz_size,
                compressed_size,
                new_extract_path.display()
            );
            println!("--------------------------");

            // Create the directories for the path
            if let Some(parent) = new_extract_path.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    eprintln!("Error creating directories: {}", err);
                } else {
                    //println!("Directories created successfully");
                }
            }

            match fi.compression_type {
                CompressionTypeT::None => {
                    let raw_file_bytes =
                        read_bytes_from_file(&archive.file_path, zzz_offset, zzz_size)?;
                    write_bytes_to_file(&new_extract_path, &raw_file_bytes)?;
                }
                CompressionTypeT::Lzss => {
                    let decompressed_bytes = lzss::decompress(
                        &read_compressed_bytes_from_file_at_offset_lzss(
                            &archive.file_path,
                            zzz_offset,
                        )?,
                        fi.uncompressed_size as usize,
                    );
                    write_bytes_to_file(&new_extract_path, &decompressed_bytes)?;
                }
                CompressionTypeT::Lz4 => {
                    let decompressed_bytes = lz4_decompress(
                        &read_compressed_bytes_from_file_at_offset_lz4(
                            &archive.file_path,
                            zzz_offset,
                        )?,
                        fi.uncompressed_size as usize,
                    )?;
                    write_bytes_to_file(&new_extract_path, &decompressed_bytes)?;
                }
            };
        }
        //let _fs_entry = &archive.fs;
        // Do something with fs_entry
    }
    //end dump toml of data
    Ok(())
}
