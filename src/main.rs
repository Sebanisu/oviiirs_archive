use std::{collections::HashSet, fs, io, path::PathBuf, process::exit};

use oviiirs_archive::oviiirs_archive::*;
mod lzss;
use typed_path::{Utf8NativeEncoding, Utf8NativePathBuf, Utf8TypedPath, Utf8WindowsPath};

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

        let mut archive_strings = HashSet::new();
        for archive in &archives {
            archive_strings.insert(&archive.fi.string_data);
            archive_strings.insert(&archive.fs.string_data);
            archive_strings.insert(&archive.fl.string_data);
        }

        let filtered_entries = data
            .entries
            .iter()
            .filter(|&entry| !archive_strings.contains(&entry.string_data));

        extract_zzz_files(filtered_entries, &zzz_file)?;

        //begin create toml of data
        let native_file_path = generate_relative_path(&generate_zzz_filename(&zzz_file));
        let extract_path = generate_native_path("toml_dumps");
        let zzz_toml_path = extract_path.join(native_file_path).to_string();
        create_directories(&PathBuf::from(&zzz_toml_path))?;
        save_config(&data, zzz_toml_path.as_str())?;
        dump_archives_toml(archives.iter())?;

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
                dump_archives_toml(field_archives.iter())?;
            }
            None => {
                // Handle the case when no element with ArchiveType::Field is found
                //println!("Element with ArchiveType::Field not found");
            }
        }
    }

    Ok(())
}

fn extract_zzz_files<'a, I>(entries: I, file_path: &String) -> io::Result<()>
where
    I: Iterator<Item = &'a ZZZEntry>,
{
    for entry in entries {
        let native_file_path = generate_relative_path(&entry.string_data);
        let extract_path = generate_native_path("test");
        let new_extract_path = PathBuf::from(extract_path.join(native_file_path).as_str());
        create_directories(&new_extract_path)?;

        let decompressed_bytes =
            read_bytes_from_file(file_path, entry.file_offset, entry.file_size as u64)?;

        println!(
            "file offset: {}, file size {}, relative path {}",
            entry.file_offset,
            entry.file_size,
            new_extract_path.display()
        );
        println!("--------------------------");
        write_bytes_to_file(&new_extract_path, &decompressed_bytes)?;
    }
    Ok(())
}

fn dump_archives_toml<'a, I>(archives: I) -> io::Result<()>
where
    I: Iterator<Item = &'a FIFLFSZZZ>,
{
    let extract_path = generate_native_path("toml_dumps");
    for archive in archives {
        {
            let native_file_path = generate_relative_path(&generate_new_filename_custom_extension(
                &Utf8WindowsPath::new(&archive.fi.string_data),
                "fiflfs_zzz",
            ));
            let toml_path = extract_path.join(native_file_path).to_string();
            create_directories(&PathBuf::from(&toml_path))?;
            save_config(&archive, &toml_path)?;
        }
        {
            let native_file_path =
                generate_relative_path(&generate_new_filename(&archive.fi.string_data));
            let toml_path = extract_path.join(native_file_path).to_string();
            create_directories(&PathBuf::from(&toml_path))?;
            let fi_file = read_fi_entries_from_file(&archive.fi, &archive.file_path)?;
            save_config(&fi_file, &toml_path)?;
        }
        {
            let native_file_path =
                generate_relative_path(&generate_new_filename(&&archive.fl.string_data));
            let toml_path = extract_path.join(native_file_path).to_string();
            create_directories(&PathBuf::from(&toml_path))?;
            let fl_file = read_fl_entries_from_file(&archive.fl, &archive.file_path)?;
            save_config(&fl_file, &toml_path)?;
        }
    }
    Ok(())
}

fn extract_archives<'a, I>(archives: I) -> io::Result<()>
where
    I: Iterator<Item = &'a FIFLFSZZZ>,
{
    for archive in archives {
        let fi_file = read_fi_entries_from_file(&archive.fi, &archive.file_path)?;

        let fl_file = read_fl_entries_from_file(&archive.fl, &archive.file_path)?;

        // Technically you don't need to always read the whole fs into memory except when it or it's parents are compressed. Just a simplication to load it into memory.
        let fs_bytes = match archive.fs.compression_type {
            CompressionTypeT::None => read_bytes_from_file(
                &archive.file_path,
                archive.fs.file_offset,
                archive.fs.file_size as u64,
            )?,
            CompressionTypeT::Lzss => lzss::decompress(
                &read_compressed_bytes_from_file_at_offset_lzss(
                    &archive.file_path,
                    archive.fs.file_offset,
                )?,
                archive.fs.file_size as usize,
            ),
            CompressionTypeT::Lz4 => lz4_decompress(
                &read_compressed_bytes_from_file_at_offset_lz4(
                    &archive.file_path,
                    archive.fs.file_offset,
                )?,
                archive.fs.file_size as usize,
            )?,
        };

        for fi_fl in fi_file
            .entries
            .iter()
            .zip(fl_file.entries.iter())
            .filter(|(fi, _)| fi.uncompressed_size != 0)
        {
            let (fi, fl) = fi_fl;

            let native_file_path = generate_relative_path(&fl);
            let extract_path = generate_native_path("test");
            let new_extract_path = PathBuf::from(extract_path.join(native_file_path).as_str());
            create_directories(&new_extract_path)?;

            println!("FI: {:?}", fi);
            println!("FL: {:?}", fl);
            println!(
                "file offset: {}, file size {}, relative path {}",
                fi.offset,
                fi.uncompressed_size,
                new_extract_path.display()
            );
            println!("--------------------------");

            match fi.compression_type {
                CompressionTypeT::None => {
                    let raw_file_bytes = read_bytes_from_memory(
                        &fs_bytes,
                        fi.offset as usize,
                        fi.uncompressed_size as usize,
                    );
                    write_bytes_to_file(&new_extract_path, &raw_file_bytes)?;
                }
                CompressionTypeT::Lzss => {
                    let decompressed_bytes = lzss::decompress(
                        &read_compressed_bytes_from_memory_at_offset_lzss(
                            &fs_bytes,
                            fi.offset as usize,
                        ),
                        fi.uncompressed_size as usize,
                    );
                    write_bytes_to_file(&new_extract_path, &decompressed_bytes)?;
                }
                CompressionTypeT::Lz4 => {
                    let decompressed_bytes = lz4_decompress(
                        &read_compressed_bytes_from_memory_at_offset_lz4(
                            &fs_bytes,
                            fi.offset as usize,
                        ),
                        fi.uncompressed_size as usize,
                    )?;
                    write_bytes_to_file(&new_extract_path, &decompressed_bytes)?;
                }
            };
        }
    }
    //end dump toml of data
    Ok(())
}

fn generate_relative_path(fl: &str) -> Utf8NativePathBuf {
    let windows_file_path = Utf8WindowsPath::new(fl);
    let relative_windows_file_path = match windows_file_path.strip_prefix("c:\\") {
        Ok(p) => p,
        Err(_) => windows_file_path,
    };
    relative_windows_file_path.with_encoding::<Utf8NativeEncoding>()
}

fn generate_native_path(outpath: &str) -> Utf8NativePathBuf {
    match Utf8TypedPath::derive(outpath) {
        Utf8TypedPath::Unix(p) => p.with_encoding::<Utf8NativeEncoding>(),
        Utf8TypedPath::Windows(p) => p.with_encoding::<Utf8NativeEncoding>(),
    }
}

fn create_directories(new_extract_path: &PathBuf) -> io::Result<()> {
    match new_extract_path.parent() {
        Some(parent) => fs::create_dir_all(parent),
        None => {
            // Handle the case where `new_extract_path.parent()` is `None`.
            // You can choose to do nothing or add some error handling logic.
            Ok(())
        }
    }
}
