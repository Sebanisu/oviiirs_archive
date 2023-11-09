extern crate bincode;
extern crate toml;

use core::fmt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::process::exit;
use typed_path::Utf8WindowsPathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
struct FI {
    uncompressed_size: u32,
    offset: u32,
    compression_type: CompressionTypeT,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct FIfile {
    file_path: String,
    entries: Vec<FI>,
}

#[derive(Debug, Serialize, Deserialize)]
enum CompressionTypeT {
    None,
    Lzss,
    Lz4,
}

impl Default for CompressionTypeT {
    fn default() -> Self {
        CompressionTypeT::None
    }
}

impl fmt::Display for CompressionTypeT {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompressionTypeT::None => write!(f, "None"),
            CompressionTypeT::Lzss => write!(f, "Lzss"),
            CompressionTypeT::Lz4 => write!(f, "Lz4"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
enum LanguageCode {
    None,
    En,
    De,
    Es,
    Fr,
    It,
    Jp, // Add more language codes as needed
}

impl Default for LanguageCode {
    fn default() -> Self {
        LanguageCode::None
    }
}

impl fmt::Display for LanguageCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LanguageCode::None => write!(f, "None"),
            LanguageCode::En => write!(f, "en"),
            LanguageCode::De => write!(f, "de"),
            LanguageCode::Es => write!(f, "es"),
            LanguageCode::Fr => write!(f, "fr"),
            LanguageCode::It => write!(f, "it"),
            LanguageCode::Jp => write!(f, "jp"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ZZZEntry {
    string_length: u32,
    string_data: String,
    file_offset: u64,
    file_size: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ZZZHeader {
    file_path: String,
    count: u32,
    entries: Vec<ZZZEntry>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct FL {
    file_path: String,
    entries: Vec<String>,
}

// Top level struct to hold the TOML data.
#[derive(Serialize, Deserialize, Default)]
struct Config {
    #[serde(default)]
    locations: Locations,
}

// Config struct holds to data from the `[config]` section.
#[derive(Serialize, Deserialize, Default)]
struct Locations {
    #[serde(default)]
    chosen_directory: String,
    #[serde(default)]
    directories: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
struct FIFLFSZZZ {    
    file_path: String,
    language: LanguageCode,
    fi: Option<ZZZEntry>,
    fl: Option<ZZZEntry>,
    fs: Option<ZZZEntry>,
}

impl FIFLFSZZZ {
    fn all_some(&self) -> bool {
        self.fi.is_some() && self.fl.is_some() && self.fs.is_some()
    }
}

impl FIFLFSZZZ {
    fn push(&mut self, entry: ZZZEntry) -> bool {
        match entry.string_data.as_str() {
            s if s.ends_with("fi") => {
                self.fi = Some(entry);
                true
            }
            s if s.ends_with("fl") => {
                self.fl = Some(entry);
                true
            }
            s if s.ends_with("fs") => {
                self.fs = Some(entry);
                true
            }
            _ => false, // Return false for unrecognized extensions
        }
    }
}

impl Locations {
    // Function to ensure chosen_directory is in directories
    fn ensure_chosen_directory_in_directories(&mut self) {
        let path = Path::new(&self.chosen_directory);
        if path.exists() && path.is_dir() {
            if !self.directories.contains(&self.chosen_directory) {
                self.directories.push(self.chosen_directory.clone());
            }
        }
    }
}

enum DirectorySelection {
    NewDirectory(String),
    ExistingDirectory(String),
    Exit,
}

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

    match process_files_in_directory(&config.locations.chosen_directory) {
        Ok(zzz_files) => {
            for zzz_file in zzz_files {
                match read_data_from_file(&zzz_file) {
                    Ok(data) => {
                        let tmp_zzz_filename = generate_zzz_filename(&zzz_file);
                        save_config(&data, &tmp_zzz_filename)?;
                        // Iterate through ZZZEntry::string_data and filter for paths ending with ".fl"
                        let groups = find_groups(data);
                        for fiflfs in groups {
                            if let Some(fi_entry) = &fiflfs.fi {
                                // This is a relative path ending with ".fl"
                                println!("Found .fi file: {:?}", fi_entry.string_data);
                                match read_fi_entries_from_file(&fi_entry, &zzz_file) {
                                    Ok(fi) => {
                                        // Successfully read entries
                                        let fi_file_name =
                                            generate_new_filename(&fi_entry.string_data);
                                        save_config(&fi, &fi_file_name)?;
                                    }
                                    Err(err) => {
                                        // Handle the error
                                        eprintln!("Error reading entries: {}", err);
                                    }
                                }
                            }

                            if let Some(fl_entry) = &fiflfs.fl {
                                // This is a relative path ending with ".fl"
                                println!("Found .fl file: {:?}", fl_entry.string_data);
                                match read_fl_entries_from_file(&fl_entry, &zzz_file) {
                                    Ok(flfile) => {
                                        // Successfully read entries
                                        let fl_file_name =
                                            generate_new_filename(&fl_entry.string_data);
                                        save_config(&flfile, &fl_file_name)?;
                                    }
                                    Err(err) => {
                                        // Handle the error
                                        eprintln!("Error reading entries: {}", err);
                                    }
                                }
                            }

                            if let Some(fs_entry) = &fiflfs.fs {
                                // Do something with fs_entry
                                println!("Found fs file: {:?}", fs_entry.string_data);
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("Error: {:?}", err);
                    }
                }
            }
        }

        Err(err) => {
            eprintln!("Error: {:?}", err);
        }
    }
    Ok(())
}

fn read_file_contents(config_path: &String) -> io::Result<String> {
    fs::read_to_string(config_path).or_else(|_| {
        eprintln!("Could not read file `{}`", config_path);
        Ok(String::new()) // Return an empty string in case of an error
    })
}

fn load_config_from_file(config_path: &String) -> io::Result<Config> {
    // Read the contents of the configuration file
    let contents = read_file_contents(&config_path)?;

    // Attempt to parse the TOML content into a Config structure
    match toml::from_str::<Config>(&contents) {
        Ok(data) => Ok(data),
        Err(_) => {
            // Handle the error case, you can choose to return a default Config or propagate the error.
            // For example, return a default Config:
            Ok(Default::default())
        }
    }
}

fn filter_valid_directories(dirs: &Vec<String>) -> Vec<String> {
    let mut valid_dirs = Vec::<String>::new();

    for dir in dirs {
        if let Ok(metadata) = fs::metadata(&dir) {
            if metadata.is_dir() {
                valid_dirs.push(dir.clone());
            }
        }
    }
    // Sort the valid directories
    valid_dirs.sort();

    // Deduplicate the sorted list
    valid_dirs.dedup();

    valid_dirs
}

fn display_directory_info(
    directories: &Vec<String>,
    previously_chosen_directory: &String,
) -> DirectorySelection {
    loop {
        println!("\nSaved FF8 Directories:\n");
        if directories.is_empty() {
            println!("    None...");
        } else {
            for (index, dir_path) in directories.iter().enumerate() {
                println!(" {:>3}: {}", index + 1, dir_path);
            }
        }

        // Offer the option to enter a new directory

        println!("\nOptions:\n");
        println!("  - Enter 'N' to use a new directory");
        println!("  - Enter the number of the directory you want to choose (or '0' to exit):");
        let has_chosen_directory = {
            let path = Path::new(&previously_chosen_directory);
            path.exists() && path.is_dir()
        };

        if has_chosen_directory {
            println!(
                "  - Press Enter to use the previously chosen directory: {}",
                previously_chosen_directory
            );
        }

        let mut user_input = String::new();
        io::stdin()
            .read_line(&mut user_input)
            .expect("Failed to read user input");

        user_input = user_input.trim().to_string();

        let is_condition_met = || {
            if user_input.is_empty() && !has_chosen_directory {
                println!("No previously chosen directory is available.");
            }
            user_input.is_empty() && has_chosen_directory
        };

        if is_condition_met() {
            return DirectorySelection::ExistingDirectory(previously_chosen_directory.clone());
        } else if user_input.eq_ignore_ascii_case("N") {
            // User wants to use a new directory
            println!("Enter the path of the new directory:");
            let mut new_dir_path = String::new();
            io::stdin()
                .read_line(&mut new_dir_path)
                .expect("Failed to read user input");
            let new_dir_path = new_dir_path.trim().to_string();

            if !new_dir_path.is_empty() {
                if Path::new(&new_dir_path).is_dir() {
                    return DirectorySelection::NewDirectory(new_dir_path);
                } else {
                    println!(
                        "Invalid directory path. Directory does not exist. No directory added."
                    );
                }
            } else {
                println!("Invalid directory path. No directory added.");
            }
        } else {
            match user_input.parse::<usize>() {
                Ok(choice) if choice >= 1 && choice <= directories.len() => {
                    // User selected a valid directory
                    let selected_directory = &directories[choice - 1];
                    println!(
                        "You chose directory {}:\nPath: {}\n",
                        choice, selected_directory
                    );
                    // Now you can use 'selected_directory' for further processing.
                    return DirectorySelection::ExistingDirectory(selected_directory.clone());
                }
                Ok(0) => {
                    // User chose to exit
                    return DirectorySelection::Exit;
                }
                _ => {
                    // Invalid choice
                    println!("Invalid choice. Please enter a valid number or 'N'.");
                }
            }
        }
    }
}

fn save_config<T>(config: &T, filename: &String) -> Result<(), std::io::Error>
where
    T: Serialize,
{
    let config_str = toml::to_string(config).map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to serialize updated config: {}", e),
        )
    })?;

    let mut file = fs::File::create(filename).map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to create the specified file: {}", e),
        )
    })?;

    let mut buf_writer = io::BufWriter::new(&mut file);

    buf_writer.write_all(config_str.as_bytes())?;
    buf_writer.flush()?;
    Ok(())
}

fn process_files_in_directory(directory: &String) -> io::Result<Vec<String>> {
    // List files in the specified directory.
    let entries = fs::read_dir(directory)?;
    let mut files = Vec::<String>::new();
    // Iterate through the directory entries and filter files with ".zzz" extension.
    for entry in entries {
        let entry = entry?;

        if let Some(file_name) = entry.file_name().to_str() {
            if file_name.ends_with(".zzz") {
                // You can perform actions on the ".zzz" files here.
                println!("Found a .zzz file: {}", file_name);
                if let Some(file_path) = entry.path().to_str() {
                    files.push(file_path.to_string());
                }
            }
        }
    }

    Ok(files)
}

fn read_data_from_file(file_path: &String) -> io::Result<ZZZHeader> {
    let mut file = File::open(file_path)?;

    // Read the 32-bit count from the file
    let mut count_bytes = [0u8; 4];
    file.read_exact(&mut count_bytes)?;
    let count = u32::from_le_bytes(count_bytes);

    // Deserialize the entries
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let string_length_bytes: [u8; 4] =
            bincode::deserialize(&read_bytes(&mut file, 4)?).unwrap();
        let string_length = u32::from_le_bytes(string_length_bytes);

        let string_data_bytes = read_bytes(&mut file, string_length as usize)?;
        let string_data = String::from_utf8(string_data_bytes).unwrap();

        let file_offset = bincode::deserialize(&read_bytes(&mut file, 8)?).unwrap();
        let file_size_bytes: [u8; 4] = bincode::deserialize(&read_bytes(&mut file, 4)?).unwrap();
        let file_size = u32::from_le_bytes(file_size_bytes);

        entries.push(ZZZEntry {
            string_length,
            string_data,
            file_offset,
            file_size,
        });
    }

    Ok(ZZZHeader {
        file_path: file_path.to_string(),
        count,
        entries,
    })
}

fn read_bytes<R: Read>(reader: &mut R, length: usize) -> io::Result<Vec<u8>> {
    let mut buffer = vec![0; length];
    reader.read_exact(&mut buffer)?;
    Ok(buffer)
}

fn generate_zzz_filename(path: &String) -> String {
    let base_name = Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("default");

    let zzz_filename = format!("{}_zzz.toml", base_name);
    zzz_filename
}

fn read_fl_entries_from_file(
    entry: &ZZZEntry,
    file_path: &str,
) -> Result<FL, Box<dyn std::error::Error>> {
    // Open the file specified by file_path for reading
    let file = File::open(file_path)?;

    // Initialize a BufReader for efficient reading
    let mut reader = BufReader::new(file);

    // Create a FL struct to hold the entries
    let mut fl = FL::default();

    fl.file_path = entry.string_data.clone();

    // Seek to the file_offset
    reader.seek(SeekFrom::Start(entry.file_offset))?;

    // Read strings separated by newlines up to (file_offset + file_size)
    let mut buffer = String::new();
    let mut bytes_read = u64::default();

    while bytes_read < entry.file_size as u64 {
        match reader.read_line(&mut buffer) {
            Ok(bytes_of_line) => {
                // Update the number of bytes read
                bytes_read += bytes_of_line as u64;
            }
            Err(error) => eprintln!("error: {error}"),
        }

        // Add the read line to FL.entries
        fl.entries.push(buffer.trim().to_string());

        // Clear the buffer for the next line
        buffer.clear();
    }

    Ok(fl)
}

fn read_fi_entries_from_file(
    entry: &ZZZEntry,
    file_path: &str,
) -> Result<FIfile, Box<dyn std::error::Error>> {
    // Open the file specified by file_path for reading
    let mut file = File::open(file_path)?;

    // Seek to the file_offset
    file.seek(SeekFrom::Start(entry.file_offset))?;

    // Create a FIfile struct to hold the entries
    let mut fifile = FIfile::default();
    fifile.file_path = entry.string_data.clone();

    // Read strings separated by newlines up to (file_offset + file_size)
    let mut buffer = vec![0; entry.file_size as usize];
    file.read_exact(&mut buffer)?;

    let mut cursor = io::Cursor::new(buffer);

    while cursor.position() < entry.file_size as u64 {
        match bincode::deserialize::<FI>(&read_bytes(&mut cursor, std::mem::size_of::<FI>())?) {
            Ok(fi) => {
                fifile.entries.push(fi);
            }
            Err(error) => eprintln!("error: {error}"),
        }
        // Move the cursor back to the correct position after deserialization
        //cursor.set_position(cursor.position() + (std::mem::size_of::<FI>() as u64));
    }

    Ok(fifile)
}

fn generate_new_filename(path: &str) -> String {
    let path_buf = Utf8WindowsPathBuf::from(path);
    let filename = path_buf.file_name().unwrap().to_string();
    let parent = path_buf.parent();
    let lang: Option<String> = parent.and_then(|p| {
        if let Some(dir_name) = p.file_name() {
            let parent_str = dir_name.to_string();
            if parent_str.starts_with("lang-") {
                Some(parent_str[5..].to_string())
            } else {
                None
            }
        } else {
            None
        }
    });

    let extension = path_buf
        .extension()
        .and_then(|ext| Some(ext.to_string()))
        .unwrap_or("".to_string());

    let new_filename = match lang {
        Some(lang_code) => format!(
            "{}_{}_{}.toml",
            filename.replace(&format!(".{}", extension), ""),
            extension,
            lang_code
        ),
        None => format!(
            "{}_{}.toml",
            filename.replace(&format!(".{}", extension), ""),
            extension
        ),
    };

    new_filename
}

fn find_groups(entries: Vec<ZZZEntry>, &file_path:String) -> Vec<FIFLFSZZZ> {
    let mut groups: HashMap<String, FIFLFSZZZ> = HashMap::new();

    for entry in entries {
        let prefix = get_prefix(&entry.string_data);
        groups
            .entry(prefix)
            .or_insert_with(FIFLFSZZZ::default)
            .push(entry);
    }

    groups
        .values()
        .cloned()
        .filter(|group| group.all_some())
        .collect()
}

fn get_prefix(s: &str) -> String {
    let parts: Vec<&str> = s.rsplitn(2, '.').collect(); // Use rsplitn to split only once from the right
    if parts.len() >= 2 {
        parts[1].to_string()
    } else {
        s.to_string()
    }
}
