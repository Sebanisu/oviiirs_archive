use std::{fs, io, path::PathBuf, process::exit};

use itertools::Itertools;
use oviiirs_archive::oviiirs_archive::*;
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

        for archive in archives {
            save_config(
                &archive,
                &generate_new_filename_custom_extension(
                    &Utf8WindowsPath::new(&archive.fi.string_data),
                    "fiflfs_zzz",
                ),
            )?;
            let fi_file = read_fi_entries_from_file(&archive.fi, &zzz_file)?;
            save_config(&fi_file, &generate_new_filename(&archive.fi.string_data))?;

            let fl_file = read_fl_entries_from_file(&archive.fl, &zzz_file)?;
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
                let native_file_path =
                    relative_windows_file_path.with_encoding::<Utf8NativeEncoding>();
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
                        let raw_file_bytes = read_bytes_from_file(&zzz_file, zzz_offset, zzz_size)?;
                        write_bytes_to_file(&new_extract_path, &raw_file_bytes)?;
                    }
                    CompressionTypeT::Lzss => {
                        let compressed_bytes =
                            read_bytes_from_file(&zzz_file, zzz_offset, compressed_size)?;
                        write_bytes_to_file(&new_extract_path, &compressed_bytes)?;
                        //     let result = MyLzss::decompress_stack(
                        //         lzss::SliceReader::new(&compressed_bytes),
                        //         lzss::VecWriter::with_capacity(zzz_size as usize),
                        //     );
                        //     match result {
                        //         Ok(buffer) => write_bytes_to_file(&new_extract_path, &buffer)?,
                        //         Err(e) => {
                        //             eprintln!("lzss error: {}", e);
                        //         }
                        //     }
                    }
                    CompressionTypeT::Lz4 => {}
                };
            }
            //let _fs_entry = &archive.fs;
            // Do something with fs_entry
        }
        //end dump toml of data
    }

    Ok(())
}
