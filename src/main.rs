use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    process::exit,
};

use oviiirs_archive::oviiirs_archive::*;
mod lzss;
use regex::Regex;
use typed_path::{Utf8NativeEncoding, Utf8NativePathBuf, Utf8TypedPath, Utf8WindowsPath};

fn main() -> io::Result<()> {
    let config_path: String = "config.toml".to_string();

    let mut config: Config = load_toml_from_file(&config_path)?;

    config.locations.ensure_chosen_directory_in_directories();

    let has_chosen_directory = {
        let path = Path::new(&config.locations.chosen_directory);
        path.exists() && path.is_dir()
    };
    if !has_chosen_directory {
        change_ff8_directory(&mut config);

        save_toml(&config, &config_path)?;
    }
    loop {
        let main_menu_options: Vec<(MainMenuSelection, Option<&str>, Option<&str>)> = vec![
            (
                MainMenuSelection::ChangeFF8Directory,
                Some("current: "),
                Some(&config.locations.chosen_directory),
            ),
            (
                MainMenuSelection::ChangeExtractDirectory,
                Some("current: "),
                Some(&config.locations.extract_directory),
            ),
            (
                MainMenuSelection::ChangeRegExFilter,
                Some("r"),
                Some(if config.extract_regex_filter.is_empty() {
                    ".*"
                } else {
                    &config.extract_regex_filter
                }),
            ),
            (MainMenuSelection::RebuildCache, None, None),
            (MainMenuSelection::Exit, None, None),
        ];
        main_menu_options
            .iter()
            .for_each(|(label, attr, value)| match (label, attr, value) {
                (label, Some(attr), Some(value)) => {
                    println!("\t{}: {} ({}\"{}\")", *label as u32, label, attr, value);
                }
                (label, None, None) => {
                    println!("\t{}: {}", *label as u32, label);
                }
                (_, &None, &Some(value)) => {
                    println!("\t{}: {} (\"{}\")", *label as u32, label, value);
                }
                (_, &Some(value), &None) => {
                    println!("\t{}: {} (\"{}\")", *label as u32, label, value);
                }
            });

        let mut user_input = String::new();
        io::stdin()
            .read_line(&mut user_input)
            .expect("Failed to read user input");

        user_input = user_input.trim().to_string();

        let cache_path = generate_native_path("cache");
        let toml_path = cache_path.join("archives.toml").to_string();
        let bincode_path = cache_path.join("archives.bin").to_string();

        create_directories(&PathBuf::from(&toml_path))?;
        match user_input.parse::<MainMenuSelection>() {
            Ok(MainMenuSelection::ChangeFF8Directory) => {
                change_ff8_directory(&mut config);

                save_toml(&config, &config_path)?;
            }
            Ok(MainMenuSelection::ChangeExtractDirectory) => {
                println!("\nEnter a new extract path: ");
                let mut user_input_extract_path = String::new();
                io::stdin()
                    .read_line(&mut user_input_extract_path)
                    .expect("Failed to read user input");

                user_input_extract_path = user_input_extract_path.trim().to_string();
                match is_valid_path(&user_input_extract_path) {
                    true => {
                        config.locations.extract_directory = user_input_extract_path;
                        save_toml(&config, &config_path)?;
                    }
                    false => {
                        eprintln!("Error not a valid path: \"{}\"\n", user_input_extract_path);
                    }
                }
            }
            Ok(MainMenuSelection::RebuildCache) => {
                let zzz_files = load_archives(&config)?;
                save_toml(&zzz_files, &toml_path)?;
                save_bincode(&zzz_files, &bincode_path)?;
            }
            Ok(MainMenuSelection::ExtractAllFiles) => {
                let zzz_files = load_or_rebuild_cache(&config, &toml_path, &bincode_path)?;

                extract_all_files(&zzz_files, &config)?;
            }
            Ok(MainMenuSelection::ChangeRegExFilter) => {
                println!("\nEnter a new extract RegEx filter: ");
                let mut user_input_regex_filter = String::new();
                io::stdin()
                    .read_line(&mut user_input_regex_filter)
                    .expect("Failed to read user input");

                user_input_regex_filter = user_input_regex_filter.trim().to_string();

                if user_input_regex_filter.is_empty() {
                    config.extract_regex_filter.clear();
                    save_toml(&config, &config_path)?;
                } else if let Ok(_) = Regex::new(&user_input_regex_filter) {
                    // The regex is valid
                    config.extract_regex_filter = user_input_regex_filter;
                    save_toml(&config, &config_path)?;
                } else {
                    eprintln!("Invalid RegEx r\"{}\"", user_input_regex_filter);
                }
            }
            Ok(MainMenuSelection::Exit) => {
                // Handle the case when the user chooses to exit
                println!("Exiting...");
                // Perform any necessary cleanup and exit the program
                // You can return a default value here or use a placeholder value
                //exit(0);
                return Ok(());
            }
            Err(err) => {
                match err {
                    ParseMainMenuError::InvalidInput(msg) => {
                        println!("Error Invalid Input: \"{}\"\n", msg);
                        // Handle invalid input
                    }
                }
            }
        }
    }
}

// Load data from bincode file if it exists, otherwise from TOML file or rebuild cache
fn load_or_rebuild_cache(
    config: &Config,
    toml_path: &String,
    bincode_path: &String,
) -> Result<ZZZfiles, io::Error> {
    match (
        Path::new(bincode_path).exists(),
        Path::new(toml_path).exists(),
    ) {
        (true, _) => load_bincode_from_file(bincode_path),
        (false, true) => load_toml_from_file(toml_path),
        (false, false) => {
            let zzz_files = load_archives(config)?;

            save_toml(&zzz_files, toml_path)?;
            save_bincode(&zzz_files, bincode_path)?;

            Ok(zzz_files)
        }
    }
}

fn is_valid_path(path_str: &str) -> bool {
    let path_buf = PathBuf::from(path_str);

    // Check if the path has invalid characters
    if let Some(_) = path_buf.to_str() {
        // Path is valid
        true
    } else {
        // Path contains invalid characters
        false
    }
}

fn change_ff8_directory(config: &mut Config) {
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
}

fn extract_all_files(zzz_files: &ZZZfiles, config: &Config) -> io::Result<()> {
    let re = match &config.extract_regex_filter {
        s if s.is_empty() => Regex::new(r".*"),
        _ => Regex::new(&config.extract_regex_filter),
    };
    let archive_strings = get_archive_strings(&zzz_files.main);
    zzz_files
        .into_iter()
        .try_for_each(|opt_zzz_file| -> io::Result<()> {
            if let Some(zzz_file) = opt_zzz_file {
                let filtered_entries = zzz_file.entries.iter().filter(|&entry| {
                    !archive_strings.contains(&entry.string_data)
                        && (re.is_err()
                            || re.as_ref().is_ok_and(|r| r.is_match(&entry.string_data)))
                });
                extract_zzz_files(filtered_entries, &zzz_file.file_path, &config)?;

                if let Some(archives) = zzz_file.fiflfs_files.as_ref() {
                    extract_archives(
                        archives
                            .iter()
                            .filter(|&item| item.archive_type != ArchiveType::Field),
                        &config,
                    )?;

                    if let Some(field) = archives
                        .iter()
                        .find(|&item| item.archive_type == ArchiveType::Field)
                    {
                        if let Some(field_archives) = field.field_archives.as_ref() {
                            extract_archives(field_archives.iter(), &config)?;
                        }
                    }
                }
            }
            Ok(())
        })?;
    Ok(())
}

fn get_archive_strings(archive: &Option<ZZZHeader>) -> HashSet<String> {
    let mut archive_strings = HashSet::new();
    if let Some(main) = archive.as_ref() {
        if let Some(archives) = main.fiflfs_files.as_ref() {
            for archive in archives {
                archive_strings.insert(archive.fi.string_data.clone());
                archive_strings.insert(archive.fs.string_data.clone());
                archive_strings.insert(archive.fl.string_data.clone());
            }
        }
    }
    archive_strings
}

fn load_archives(config: &Config) -> io::Result<ZZZfiles> {
    let mut zzz_files: ZZZfiles = Default::default();
    let zzz_paths = process_files_in_directory(&config.locations.chosen_directory)?;

    zzz_paths.iter().try_for_each(|path| -> io::Result<()> {
        let mut data = read_data_from_file(&path)?;

        if data.fiflfs_files.is_none() || data.fiflfs_files.as_ref().unwrap().is_empty() {
            data.fiflfs_files
                .get_or_insert_with(|| find_archives(data.entries.clone(), &path));

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
    Ok(zzz_files)
}

fn extract_zzz_files<'a, I>(entries: I, file_path: &String, config: &Config) -> io::Result<()>
where
    I: Iterator<Item = &'a ZZZEntry>,
{
    for entry in entries {
        let native_file_path = generate_relative_path(&entry.string_data);
        let extract_path = generate_native_path(&config.locations.extract_directory);
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

fn extract_archives<'a, I>(archives: I, config: &Config) -> io::Result<()>
where
    I: Iterator<Item = &'a FIFLFSZZZ>,
{
    let re = match &config.extract_regex_filter {
        s if s.is_empty() => Regex::new(r".*"),
        _ => Regex::new(&config.extract_regex_filter),
    };
    for archive in archives {
        if archive.fi_file.is_none() || archive.fi_file.is_none() {
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
            .filter(|(fi, fl)| {
                fi.uncompressed_size != 0
                    && (re.is_err() || re.as_ref().is_ok_and(|r| r.is_match(&fl)))
            })
        {
            let (fi, fl) = fi_fl;

            let native_file_path = generate_relative_path(&fl);
            let extract_path = generate_native_path(&config.locations.extract_directory);
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
                    &read_compressed_bytes_from_memory_at_offset_lz4(&fs_bytes, fi.offset as usize),
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
    if let Some(parent) = new_extract_path.parent() {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}
