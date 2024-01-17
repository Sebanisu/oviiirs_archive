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

    let mut zzz_files: ZZZfiles = Default::default();
    let zzz_paths = process_files_in_directory(&config.locations.chosen_directory)?;
    let mut archive_strings = HashSet::new();

    zzz_paths.iter().try_for_each(|path| -> io::Result<()> {
        let mut data = read_data_from_file(&path)?;

        if data.fiflfs_files.is_none() || data.fiflfs_files.as_ref().unwrap().is_empty() {
            data.fiflfs_files
                .get_or_insert_with(|| find_archives(data.entries.clone(), &path));

            if let Some(archives) = data.fiflfs_files.as_ref() {
                for archive in archives {
                    archive_strings.insert(archive.fi.string_data.clone());
                    archive_strings.insert(archive.fs.string_data.clone());
                    archive_strings.insert(archive.fl.string_data.clone());
                }
            }

            if let Some(archives) = data.fiflfs_files.as_mut() {
                load_archives_fi_fl(archives.iter_mut())?;

                if let Some(field) = archives
                    .iter_mut()
                    .find(|item| item.archive_type == ArchiveType::Field)
                {
                    if field.field_archives.is_none()
                        || field.field_archives.as_ref().unwrap().is_empty()
                    {
                        field.field_archives = Some(
                            field
                                .field_archives
                                .take()
                                .map_or_else(|| find_archives_field(field), Ok)?,
                        );
                    }

                    if let Some(field_archives) = field.field_archives.as_mut() {
                        load_archives_fi_fl(field_archives.iter_mut())?;
                    }
                }
            }
        }

        zzz_files.push(data);
        Ok(())
    })?;

    //begin create toml of data
    let extract_path = generate_native_path("toml_dumps");
    let toml_path = extract_path.join("archives.toml").to_string();
    create_directories(&PathBuf::from(&toml_path))?;
    save_config(&zzz_files, &toml_path)?;

    zzz_files
        .into_iter()
        .try_for_each(|opt_zzz_file| -> io::Result<()> {
            if let Some(zzz_file) = opt_zzz_file {
                let filtered_entries = zzz_file
                    .entries
                    .iter()
                    .filter(|&entry| !archive_strings.contains(&entry.string_data));
                extract_zzz_files(filtered_entries, &zzz_file.file_path)?;

                if let Some(archives) = zzz_file.fiflfs_files.as_ref() {
                    extract_archives(
                        archives
                            .iter()
                            .filter(|&item| item.archive_type != ArchiveType::Field),
                    )?;

                    if let Some(field) = archives
                        .iter()
                        .find(|&item| item.archive_type == ArchiveType::Field)
                    {
                        if let Some(field_archives) = field.field_archives.as_ref() {
                            extract_archives(field_archives.iter())?;
                        }
                    }
                }
            }
            Ok(())
        })?;

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

fn load_archives_fi_fl<'a, I>(archives: I) -> io::Result<()>
where
    I: Iterator<Item = &'a mut FIFLFSZZZ>, // Change to mutable references
{
    for archive in archives {
        // Now you can use `archive` as a mutable reference
        if archive.fi_file.is_none() || archive.fi_file.as_ref().unwrap().entries.is_empty() {
            archive.fi_file = Some(archive.fi_file.take().map_or_else(
                || read_fi_entries_from_file(&archive.fi, &archive.file_path),
                Ok,
            )?);
        }

        if archive.fl_file.is_none() || archive.fi_file.as_ref().unwrap().entries.is_empty() {
            archive.fl_file = Some(archive.fl_file.take().map_or_else(
                || read_fl_entries_from_file(&archive.fl, &archive.file_path),
                Ok,
            )?);
        }
    }

    Ok(())
}

fn extract_archives<'a, I>(archives: I) -> io::Result<()>
where
    I: Iterator<Item = &'a FIFLFSZZZ>,
{
    for archive in archives {
        if archive.fi_file.is_none() || archive.fi_file.is_none()
        {
            continue;
        }
        //let fi_file = read_fi_entries_from_file(&archive.fi, &archive.file_path)?;

        //let fl_file = read_fl_entries_from_file(&archive.fl, &archive.file_path)?;

        let fi_file = archive.fi_file.as_ref().unwrap();
        let fl_file = archive.fl_file.as_ref().unwrap();

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

            let uncompressed_bytes = match fi.compression_type {
                CompressionTypeT::None => read_bytes_from_memory(
                    &fs_bytes,
                    fi.offset as usize,
                    fi.uncompressed_size as usize,
                ),
                CompressionTypeT::Lzss => lzss::decompress(
                    &read_compressed_bytes_from_memory_at_offset_lzss(
                        &fs_bytes,
                        fi.offset as usize,
                    ),
                    fi.uncompressed_size as usize,
                ),
                CompressionTypeT::Lz4 => lz4_decompress(
                    &read_compressed_bytes_from_memory_at_offset_lz4(
                        &fs_bytes,
                        fi.offset as usize,
                    ),
                    fi.uncompressed_size as usize,
                )?,
            };
            
            write_bytes_to_file(&new_extract_path, &uncompressed_bytes)?;
            
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
