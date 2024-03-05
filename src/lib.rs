pub use oviiirs_archive::{
    capitalize, display_directory_info, filter_valid_directories, find_archives,
    find_archives_field, generate_new_filename, generate_new_filename_custom_extension,
    generate_zzz_filename, load_bincode_from_file, load_toml_from_file, lz4_decompress,
    process_files_in_directory, read_bytes_from_file, read_bytes_from_memory,
    read_compressed_bytes_from_file_at_offset_lz4, read_compressed_bytes_from_file_at_offset_lzss,
    read_compressed_bytes_from_memory_at_offset_lzss, read_data_from_file, save_bincode, save_toml,
    write_bytes_to_file, CompressionTypeT, DirectorySelection,
};
mod lzss;
pub mod oviiirs_archive {
    use bincode;
    use core::fmt;
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::fs;
    use std::fs::File;
    use std::io;
    use std::io::BufRead;
    use std::io::BufReader;
    use std::io::Cursor;
    use std::io::Read;
    use std::io::Seek;
    use std::io::SeekFrom;
    use std::io::Write;
    use std::path::Path;
    use std::path::PathBuf;
    use toml;
    use typed_path::Utf8Path;
    use typed_path::Utf8TypedPath;
    use typed_path::Utf8WindowsPath;

    #[derive(Debug, Serialize, Deserialize, Default, Clone)]
    pub struct ZZZfiles {
        pub main: Option<ZZZHeader>,
        pub other: Option<ZZZHeader>,
    }

    impl ZZZfiles {
        pub fn push(&mut self, entry: ZZZHeader) -> bool {
            let file_path = Path::new(&entry.file_path);
            if let Some(extension) = file_path.extension() {
                if extension == "zzz" {
                    if let Some(stem) = file_path.file_stem() {
                        match stem.to_str() {
                            Some("main") => {
                                self.main = Some(entry);
                                return true;
                            }
                            Some("other") => {
                                self.other = Some(entry);
                                return true;
                            }
                            _ => {}
                        }
                    }
                }
            }
            false
        }
    }
    impl<'a> IntoIterator for &'a ZZZfiles {
        type Item = Option<&'a ZZZHeader>;
        type IntoIter = std::vec::IntoIter<Self::Item>;

        fn into_iter(self) -> Self::IntoIter {
            vec![self.main.as_ref(), self.other.as_ref()].into_iter()
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default, Clone)]
    #[repr(C)]
    pub struct FI {
        pub uncompressed_size: u32,
        pub offset: u32,
        pub compression_type: CompressionTypeT,
    }

    #[derive(Debug, Serialize, Deserialize, Default, Clone)]
    pub struct FIfile {
        pub file_path: String,
        pub entries: Vec<FI>,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
    #[repr(u32)]
    pub enum CompressionTypeT {
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
                CompressionTypeT::None => write!(f, "none"),
                CompressionTypeT::Lzss => write!(f, "lzss"),
                CompressionTypeT::Lz4 => write!(f, "lz4"),
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub enum ZZZArchiveType {
        None,
        Main,
        Other,
    }

    impl std::fmt::Display for ZZZArchiveType {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                ZZZArchiveType::None => write!(f, "none"),
                ZZZArchiveType::Main => write!(f, "Main"),
                ZZZArchiveType::Other => write!(f, "Other"),
            }
        }
    }

    impl FromStr for ZZZArchiveType {
        fn from_str(s: &str) -> Self {
            match s {
                "main" => ZZZArchiveType::Main,
                "other" => ZZZArchiveType::Other,
                _ => ZZZArchiveType::None,
            }
        }
    }

    impl Default for ZZZArchiveType {
        fn default() -> Self {
            ZZZArchiveType::None
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub enum ArchiveType {
        None,
        Battle,
        Field,
        Magic,
        Main,
        Menu,
        World,
        Other(String), // Custom or additional archive types
    }

    impl std::fmt::Display for ArchiveType {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                ArchiveType::None => write!(f, "none"),
                ArchiveType::Battle => write!(f, "Battle"),
                ArchiveType::Field => write!(f, "Field"),
                ArchiveType::Magic => write!(f, "Magic"),
                ArchiveType::Main => write!(f, "Main"),
                ArchiveType::Menu => write!(f, "Menu"),
                ArchiveType::World => write!(f, "World"),
                ArchiveType::Other(s) => write!(f, "{}", s),
            }
        }
    }

    impl FromStr for ArchiveType {
        fn from_str(s: &str) -> Self {
            let trimmed = s.trim();

            if trimmed.is_empty() {
                return ArchiveType::None;
            }
            match trimmed {
                "battle" => ArchiveType::Battle,
                "field" => ArchiveType::Field,
                "magic" => ArchiveType::Magic,
                "main" => ArchiveType::Main,
                "menu" => ArchiveType::Menu,
                "world" => ArchiveType::World,
                other => ArchiveType::Other(other.to_lowercase()),
            }
        }
    }

    impl Default for ArchiveType {
        fn default() -> Self {
            ArchiveType::None
        }
    }

    pub fn capitalize(s: &str) -> String {
        let (first, rest) = s.split_at(1);
        format!("{}{}", first.to_uppercase(), rest.to_lowercase())
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub enum LanguageCode {
        None,
        En,
        De,
        Es,
        Fr,
        It,
        Jp, // Add more language codes as needed
    }

    trait FromStr {
        fn from_str(s: &str) -> Self;
    }

    impl FromStr for LanguageCode {
        fn from_str(s: &str) -> Self {
            match s {
                "en" => LanguageCode::En,
                "de" => LanguageCode::De,
                "es" => LanguageCode::Es,
                "fr" => LanguageCode::Fr,
                "it" => LanguageCode::It,
                "jp" => LanguageCode::Jp,
                _ => LanguageCode::None,
            }
        }
    }

    impl Default for LanguageCode {
        fn default() -> Self {
            LanguageCode::None
        }
    }

    impl std::fmt::Display for LanguageCode {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                LanguageCode::None => write!(f, "none"),
                LanguageCode::En => write!(f, "en"),
                LanguageCode::De => write!(f, "de"),
                LanguageCode::Es => write!(f, "es"),
                LanguageCode::Fr => write!(f, "fr"),
                LanguageCode::It => write!(f, "it"),
                LanguageCode::Jp => write!(f, "jp"),
            }
        }
    }

    #[derive(Debug, Deserialize, Serialize, Clone, Default)]
    pub struct ZZZEntry {
        pub string_length: u32,
        pub string_data: String,
        pub file_offset: u64,
        pub file_size: u32,
        pub compression_type: CompressionTypeT, //this doesn't exist in the ZZZ file but it does in FIFLFS.
    }

    pub trait ReadEntry: DeserializeOwned {
        fn read_entry(cursor: &mut Cursor<Vec<u8>>) -> io::Result<Self> {
            let cursor_position = cursor.position() as usize;
            let cursor_data = cursor.get_ref();

            // Ensure that there's enough data remaining in the cursor for deserialization
            let remaining_bytes = cursor_data.len() - cursor_position;
            if remaining_bytes < std::mem::size_of::<Self>() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Insufficient data for deserialization",
                ));
            }

            // Get a slice of the data from the current cursor position, exactly the size of Self
            let data_slice =
                &cursor_data[cursor_position..cursor_position + std::mem::size_of::<Self>()];

            // Deserialize the slice into the desired type
            let result = bincode::deserialize(data_slice).map_err(|err| match *err {
                bincode::ErrorKind::Io(io_err) => io_err,
                _ => io::Error::new(io::ErrorKind::InvalidData, err),
            })?;

            // Move the cursor forward by the size of Self
            let new_position = cursor_position + std::mem::size_of::<Self>();
            cursor.set_position(new_position as u64);

            Ok(result)
        }
    }

    // Implementing ReadEntries trait
    trait ReadEntries: ReadEntry {
        fn read_entries(cursor: &mut Cursor<Vec<u8>>) -> io::Result<Vec<Self>> {
            let mut vec: Vec<Self> = vec![];
            loop {
                match Self::read_entry(cursor) {
                    Ok(item) => {
                        vec.push(item);
                    }
                    Err(error) => {
                        return match error.kind() {
                            io::ErrorKind::UnexpectedEof => {
                                //println!("Cursor is already at the end of the data.");
                                Ok(vec)
                            }
                            _ => {
                                eprintln!("Error occurred: {}", error);
                                Err(error)
                            }
                        };
                    }
                }
            }
        }
    }

    // Implement ReadEntry for FI
    impl ReadEntry for FI {}
    impl ReadEntries for FI {}

    pub trait ConvertFromZZZEntryAndFile: Sized {
        fn from_zzz_entry_and_file(entry: &ZZZEntry, file_path: &str) -> io::Result<Self>;
    }

    impl ConvertFromZZZEntryAndFile for FIfile {
        fn from_zzz_entry_and_file(entry: &ZZZEntry, file_path: &str) -> io::Result<Self> {
            // Create a FIfile struct to hold the entries
            let mut fifile = FIfile::default();
            fifile.file_path = entry.string_data.clone();

            // Technically you don't need to always read the whole fi into memory except when it or it's parents are compressed. Just a simplication to load it into memory. You could always calculate the position from the fl file. Index*12 = the offset of an entry.
            let buffer = match entry.compression_type {
                CompressionTypeT::None => {
                    read_bytes_from_file(file_path, entry.file_offset, entry.file_size as u64)?
                }
                CompressionTypeT::Lzss => crate::lzss::decompress(
                    &read_compressed_bytes_from_file_at_offset_lzss(&file_path, entry.file_offset)?,
                    entry.file_size as usize,
                ),
                CompressionTypeT::Lz4 => lz4_decompress(
                    &read_compressed_bytes_from_file_at_offset_lz4(&file_path, entry.file_offset)?,
                    entry.file_size as usize,
                )?,
            };
            if buffer.len() != entry.file_size as usize {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Buffer size doesn't match entry.file_size",
                ));
            }

            let mut cursor = io::Cursor::new(buffer);

            fifile.entries = FI::read_entries(&mut cursor)?;

            Ok(fifile)
        }
    }

    impl ConvertFromZZZEntryAndFile for FL {
        fn from_zzz_entry_and_file(entry: &ZZZEntry, file_path: &str) -> io::Result<FL> {
            // Open the file specified by file_path for reading
            let buffer_bytes = match entry.compression_type {
                CompressionTypeT::None => {
                    read_bytes_from_file(file_path, entry.file_offset, entry.file_size as u64)?
                }
                CompressionTypeT::Lzss => crate::lzss::decompress(
                    &read_compressed_bytes_from_file_at_offset_lzss(&file_path, entry.file_offset)?,
                    entry.file_size as usize,
                ),
                CompressionTypeT::Lz4 => lz4_decompress(
                    &read_compressed_bytes_from_file_at_offset_lz4(&file_path, entry.file_offset)?,
                    entry.file_size as usize,
                )?,
            };

            let cursor = Cursor::new(buffer_bytes);

            // Initialize a BufReader for efficient reading
            let mut reader = BufReader::new(cursor);

            // Create a FL struct to hold the entries
            let mut fl = FL::default();

            fl.file_path = entry.string_data.clone();

            // Read strings separated by newlines up to (file_offset + file_size)
            let mut buffer = String::new();

            loop {
                match reader.read_line(&mut buffer) {
                    Ok(bytes_of_line) => {
                        if bytes_of_line == 0 {
                            break;
                        }
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
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct ZZZHeader {
        pub file_path: String,
        pub archive_type: ZZZArchiveType,
        pub count: u32,
        pub entries: Vec<ZZZEntry>,
        pub fiflfs_files: Option<Vec<FIFLFSZZZ>>,
    }

    #[derive(Debug, Deserialize, Serialize, Default, Clone)]
    pub struct FL {
        pub file_path: String,
        pub entries: Vec<String>,
    }

    // Top level struct to hold the TOML data.
    // Config struct holds to data from the `[config]` section.
    #[derive(Serialize, Deserialize, Default, Clone)]
    pub struct Config {
        #[serde(default)]
        pub locations: Locations,
        pub extract_regex_filter: String,
    }

    #[derive(Serialize, Deserialize, Default, Clone)]
    pub struct Locations {
        #[serde(default)]
        pub chosen_directory: String,
        #[serde(default = "default_extract_directory")]
        pub extract_directory: String,
        #[serde(default)]
        pub directories: Vec<String>,
    }

    fn default_extract_directory() -> String {
        String::from("test")
    }

    #[derive(Debug, Serialize, Deserialize, Default, Clone)]
    struct FIFLFSZZZTemp {
        fi: Option<ZZZEntry>,
        fl: Option<ZZZEntry>,
        fs: Option<ZZZEntry>,
    }

    impl FIFLFSZZZTemp {
        fn all_some(&self) -> bool {
            self.fi.is_some() && self.fl.is_some() && self.fs.is_some()
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default, Clone)]
    pub struct FIFLFSZZZ {
        pub file_path: String,
        pub language: LanguageCode,
        pub archive_type: ArchiveType,
        pub fi: ZZZEntry,
        pub fl: ZZZEntry,
        pub fs: ZZZEntry,
        pub fi_file: Option<FIfile>,
        pub fl_file: Option<FL>,
        pub field_archives: Option<Vec<FIFLFSZZZ>>,
    }

    impl FIFLFSZZZTemp {
        fn move_into_final(self, file_path: String) -> FIFLFSZZZ {
            let string_data = &self.fi.as_ref().unwrap().string_data;
            let path_buf = Utf8WindowsPath::new(string_data);
            let language = get_language_code(&path_buf);
            let archive_type = get_archive_type(&path_buf);
            FIFLFSZZZ {
                file_path,
                language,
                archive_type,
                fi: self.fi.unwrap(),
                fl: self.fl.unwrap(),
                fs: self.fs.unwrap(),
                fi_file: None,
                fl_file: None,
                field_archives: None,
            }
        }
    }

    impl FIFLFSZZZTemp {
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
        pub fn ensure_chosen_directory_in_directories(&mut self) {
            let path = Path::new(&self.chosen_directory);
            if path.exists() && path.is_dir() {
                if !self.directories.contains(&self.chosen_directory) {
                    self.directories.push(self.chosen_directory.clone());
                }
            }
        }
    }

    pub enum DirectorySelection {
        NewDirectory(String),
        ExistingDirectory(String),
        Exit,
    }

    #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    pub enum MainMenuSelection {
        ChangeFF8Directory,
        ChangeExtractDirectory,
        ExtractAllFiles,
        ChangeRegExFilter,
        RebuildCache,
        Exit,
    }

    #[derive(Debug)]
    pub enum ParseMainMenuError {
        InvalidInput(String),
    }

    impl fmt::Display for MainMenuSelection {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "{}",
                match self {
                    MainMenuSelection::ChangeFF8Directory => "Change FF8 Directory",
                    MainMenuSelection::ChangeExtractDirectory => "Change Extract Directory",
                    MainMenuSelection::ExtractAllFiles => "Extract All Files",
                    MainMenuSelection::ChangeRegExFilter => "Change RegEx Filter",
                    MainMenuSelection::RebuildCache => "Rebuild Cache",
                    MainMenuSelection::Exit => "Exit",
                }
            )
        }
    }

    impl std::str::FromStr for MainMenuSelection {
        type Err = ParseMainMenuError;

        fn from_str(s: &str) -> Result<Self, ParseMainMenuError> {
            match s.trim() {
                s if s == format!("{}", MainMenuSelection::ChangeFF8Directory as u32) => {
                    Ok(MainMenuSelection::ChangeFF8Directory)
                }
                s if s == format!("{}", MainMenuSelection::ChangeExtractDirectory as u32) => {
                    Ok(MainMenuSelection::ChangeExtractDirectory)
                }
                s if s == format!("{}", MainMenuSelection::ExtractAllFiles as u32) => {
                    Ok(MainMenuSelection::ExtractAllFiles)
                }
                s if s == format!("{}", MainMenuSelection::ChangeRegExFilter as u32) => {
                    Ok(MainMenuSelection::ChangeRegExFilter)
                }
                s if s == format!("{}", MainMenuSelection::RebuildCache as u32) => {
                    Ok(MainMenuSelection::RebuildCache)
                }
                s if s == format!("{}", MainMenuSelection::Exit as u32) => {
                    Ok(MainMenuSelection::Exit)
                }
                _ => Err(ParseMainMenuError::InvalidInput(s.to_string())),
            }
        }
    }

    pub fn write_bytes_to_file(file_path: &PathBuf, data: &[u8]) -> io::Result<()> {
        let mut file = File::create(file_path)?;
        file.write_all(data)?;

        Ok(())
    }

    fn read_file_contents_as_string(config_path: &String) -> io::Result<String> {
        fs::read_to_string(config_path).or_else(|_| {
            eprintln!("Could not read file `{}`", config_path);
            Ok(String::new()) // Return an empty string in case of an error
        })
    }

    pub fn load_toml_from_file<T>(config_path: &String) -> io::Result<T>
    where
        T: DeserializeOwned + Default,
    {
        // Read the contents of the configuration file
        let contents = read_file_contents_as_string(config_path)?;

        // Attempt to parse the content into the specified type (T)
        match toml::from_str::<T>(&contents) {
            Ok(data) => Ok(data),
            Err(_) => {
                // Handle the error case, you can choose to return a default instance of T or propagate the error.
                // For example, return a default instance of T:
                Ok(Default::default())
            }
        }
    }

    pub fn load_bincode_from_file<T>(config_path: &str) -> io::Result<T>
    where
        T: DeserializeOwned + Default,
    {
        // Read the contents of the configuration file as binary data
        let contents = read_file_contents_as_vec_u8(config_path)?;

        // Attempt to deserialize the binary content into the specified type (T)
        match bincode::deserialize::<T>(&contents) {
            Ok(data) => Ok(data),
            Err(_) => {
                // Handle the error case, you can choose to return a default instance of T or propagate the error.
                // For example, return a default instance of T:
                Ok(Default::default())
            }
        }
    }

    fn read_file_contents_as_vec_u8(file_path: &str) -> io::Result<Vec<u8>> {
        let path = Path::new(file_path);
        let mut contents = Vec::new();

        fs::File::open(path)?.read_to_end(&mut contents)?;

        Ok(contents)
    }

    pub fn filter_valid_directories(dirs: &Vec<String>) -> Vec<String> {
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

    pub fn display_directory_info(
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

    pub fn save_toml<T>(config: &T, filename: &str) -> Result<(), std::io::Error>
    where
        T: Serialize,
    {
        let config_str = toml::to_string(config).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to serialize updated toml: {}", e),
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

    pub fn save_bincode<T>(config: &T, filename: &str) -> Result<(), io::Error>
    where
        T: Serialize,
    {
        // Serialize the configuration using bincode
        let config_bytes = bincode::serialize(config).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to serialize updated bincode: {}", e),
            )
        })?;

        // Create or open the specified file
        let mut file = fs::File::create(filename).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to create the specified file: {}", e),
            )
        })?;

        // Write the serialized data to the file
        file.write_all(&config_bytes)?;

        Ok(())
    }

    pub fn process_files_in_directory(directory: &String) -> io::Result<Vec<String>> {
        // List files in the specified directory.
        let entries = fs::read_dir(directory)?;
        let mut files = Vec::<String>::new();
        // Iterate through the directory entries and filter files with ".zzz" extension.
        for entry in entries {
            let entry = entry?;

            if let Some(file_name) = entry.file_name().to_str() {
                if file_name.ends_with(".zzz") {
                    // You can perform actions on the ".zzz" files here.
                    if let Some(file_path) = entry.path().to_str() {
                        files.push(file_path.to_string());
                    }
                }
            }
        }

        Ok(files)
    }

    pub fn read_data_from_file(file_path: &String) -> io::Result<ZZZHeader> {
        let archive_type = match Utf8TypedPath::derive(file_path) {
            Utf8TypedPath::Unix(unix_path_buf) => {
                // Handle Unix path
                get_zzz_archive_type(&unix_path_buf)
            }
            Utf8TypedPath::Windows(windows_path_buf) => {
                // Handle Windows path
                get_zzz_archive_type(&windows_path_buf)
            }
        };

        let mut file = File::open(file_path)?;

        // Read the 32-bit count from the file
        let mut count_bytes = [0u8; 4];
        file.read_exact(&mut count_bytes)?;
        let count = u32::from_le_bytes(count_bytes);

        // Deserialize the entries
        let mut entries = Vec::with_capacity(count as usize);
        let compression_type = CompressionTypeT::None;
        for _ in 0..count {
            let string_length_bytes: [u8; 4] =
                bincode::deserialize(&read_bytes(&mut file, 4)?).unwrap();
            let string_length = u32::from_le_bytes(string_length_bytes);

            let string_data_bytes = read_bytes(&mut file, string_length as usize)?;
            let string_data = String::from_utf8(string_data_bytes).unwrap();

            let file_offset = bincode::deserialize(&read_bytes(&mut file, 8)?).unwrap();
            let file_size_bytes: [u8; 4] =
                bincode::deserialize(&read_bytes(&mut file, 4)?).unwrap();
            let file_size = u32::from_le_bytes(file_size_bytes);

            entries.push(ZZZEntry {
                string_length,
                string_data,
                file_offset,
                file_size,
                compression_type,
            });
        }

        Ok(ZZZHeader {
            file_path: file_path.to_string(),
            archive_type,
            count,
            entries,
            fiflfs_files: None,
        })
    }

    fn read_bytes<R: Read>(reader: &mut R, length: usize) -> io::Result<Vec<u8>> {
        let mut buffer = vec![0; length];
        reader.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    pub fn generate_zzz_filename(path: &String) -> String {
        let base_name = Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("default");

        let zzz_filename = format!("{}_zzz.toml", base_name);
        zzz_filename
    }

    pub fn generate_new_filename(path: &str) -> String {
        let path_buf = Utf8WindowsPath::new(path);
        let filename = path_buf.file_name().unwrap().to_string();
        let lang_code = get_language_code(&path_buf);

        let extension = path_buf
            .extension()
            .and_then(|ext| Some(ext.to_string()))
            .unwrap_or("".to_string());

        let new_filename = match lang_code {
            LanguageCode::None => format!(
                "{}_{}.toml",
                filename.replace(&format!(".{}", extension), ""),
                extension
            ),
            _ => format!(
                "{}_{}_{}.toml",
                filename.replace(&format!(".{}", extension), ""),
                extension,
                lang_code
            ),
        };

        new_filename
    }

    pub fn find_archives_field(archive: &FIFLFSZZZ) -> io::Result<Vec<FIFLFSZZZ>> {
        let mut archives: HashMap<String, FIFLFSZZZTemp> = HashMap::new();

        let file_path = &archive.file_path;

        let fi_file = FIfile::from_zzz_entry_and_file(&archive.fi, &file_path)?;

        let fl_file = FL::from_zzz_entry_and_file(&archive.fl, &file_path)?;

        let entries = fi_file.entries.iter().zip(&fl_file.entries);

        for entry in entries {
            let prefix = get_prefix(&entry.1);
            archives
                .entry(prefix)
                .or_insert_with(FIFLFSZZZTemp::default)
                .push(ZZZEntry {
                    string_length: entry.1.len() as u32,
                    string_data: entry.1.clone(),
                    file_offset: entry.0.offset as u64 + archive.fs.file_offset,
                    file_size: entry.0.uncompressed_size,
                    compression_type: entry.0.compression_type,
                });
        }

        Ok(archives
            .values()
            .cloned()
            .filter(|group| group.all_some())
            .map(|group| group.move_into_final(file_path.clone()))
            .collect())
    }

    pub fn find_archives(entries: Vec<ZZZEntry>, file_path: &String) -> Vec<FIFLFSZZZ> {
        let mut archives: HashMap<String, FIFLFSZZZTemp> = HashMap::new();

        for entry in entries {
            let prefix = get_prefix(&entry.string_data);
            archives
                .entry(prefix)
                .or_insert_with(FIFLFSZZZTemp::default)
                .push(entry);
        }

        archives
            .values()
            .cloned()
            .filter(|group| group.all_some())
            .map(|group| group.move_into_final(file_path.clone()))
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

    fn get_language_code<E: for<'enc> typed_path::Utf8Encoding<'enc>>(
        path_buf: &Utf8Path<E>,
    ) -> LanguageCode {
        let parent = path_buf.parent();

        if let Some(dir_name) = parent.and_then(|p| p.file_name()) {
            let parent_str = dir_name.to_string();
            if parent_str.starts_with("lang-") {
                LanguageCode::from_str(&parent_str[5..])
            } else {
                LanguageCode::None
            }
        } else {
            LanguageCode::None
        }
    }

    fn get_archive_type<E: for<'enc> typed_path::Utf8Encoding<'enc>>(
        path_buf: &Utf8Path<E>,
    ) -> ArchiveType {
        let filename = path_buf.file_stem().unwrap().to_string();
        ArchiveType::from_str(&filename)
    }

    fn get_zzz_archive_type<E: for<'enc> typed_path::Utf8Encoding<'enc>>(
        path_buf: &Utf8Path<E>,
    ) -> ZZZArchiveType {
        let filename = path_buf.file_stem().unwrap().to_string();
        ZZZArchiveType::from_str(&filename)
    }

    pub fn generate_new_filename_custom_extension<E: for<'enc> typed_path::Utf8Encoding<'enc>>(
        path_buf: &Utf8Path<E>,
        extension: &str,
    ) -> String {
        let filename = path_buf.file_stem().unwrap().to_string();
        let lang_code = get_language_code(&path_buf);

        let new_filename = match lang_code {
            LanguageCode::None => format!(
                "{}_{}.toml",
                filename.replace(&format!(".{}", extension), ""),
                extension
            ),
            _ => format!(
                "{}_{}_{}.toml",
                filename.replace(&format!(".{}", extension), ""),
                extension,
                lang_code
            ),
        };

        new_filename
    }

    pub fn read_compressed_bytes_from_memory_at_offset_lzss(
        input_data: &[u8],
        offset: usize,
    ) -> Vec<u8> {
        // Ensure that the offset is within bounds
        if offset >= input_data.len() {
            return Vec::new(); // Return an empty vector if offset is out of bounds
        }
        // Deserialize a u32 from the file
        let compressed_size_as_bytes: [u8; 4] =
            bincode::deserialize(&input_data[offset..(offset + 4)]).unwrap();
        let compressed_size = u32::from_le_bytes(compressed_size_as_bytes) as usize;

        // Calculate the end index based on offset and size
        let start_index = offset + 4;
        let end_index = start_index + compressed_size.min(input_data.len() - start_index);
        input_data[start_index..end_index].to_vec()
    }

    pub fn read_compressed_bytes_from_file_at_offset_lzss(
        file_path: &str,
        offset: u64,
    ) -> io::Result<Vec<u8>> {
        // Open the file
        let mut file = File::open(file_path)?;

        // Move to the specified offset
        file.seek(SeekFrom::Start(offset))?;

        // Deserialize a u32 from the file
        let size_as_bytes: [u8; 4] = bincode::deserialize(&read_bytes(&mut file, 4)?).unwrap();
        let size = u32::from_le_bytes(size_as_bytes);

        // Read the specified number of bytes following the offset
        let mut buffer = vec![0; size as usize];
        file.read_exact(&mut buffer)?;

        Ok(buffer)
    }

    pub fn read_compressed_bytes_from_memory_at_offset_lz4(
        input_data: &[u8],
        offset: usize,
    ) -> Vec<u8> {
        // Ensure that the offset is within bounds
        if offset >= input_data.len() {
            return Vec::new(); // Return an empty vector if offset is out of bounds
        }
        // Deserialize a u32 from the file
        let compressed_size_as_bytes: [u8; 4] =
            bincode::deserialize(&input_data[offset..(offset + 4)]).unwrap();
        let compressed_size = u32::from_le_bytes(compressed_size_as_bytes) as usize - 8;

        // Calculate the end index based on offset and size
        let start_index = offset + 12;
        let end_index = start_index + compressed_size.min(input_data.len() - start_index);

        input_data[12..end_index].to_vec()
    }

    pub fn read_compressed_bytes_from_file_at_offset_lz4(
        file_path: &str,
        offset: u64,
    ) -> io::Result<Vec<u8>> {
        // Open the file
        let mut file = File::open(file_path)?;

        // Move to the specified offset
        file.seek(SeekFrom::Start(offset))?;

        // Deserialize a u32 from the file
        let compressed_size_as_bytes: [u8; 4] =
            bincode::deserialize(&read_bytes(&mut file, 4)?).unwrap();
        let compressed_size = u32::from_le_bytes(compressed_size_as_bytes) - 8;

        file.seek(SeekFrom::Current(4))?;

        let uncompressed_size_as_bytes: [u8; 4] =
            bincode::deserialize(&read_bytes(&mut file, 4)?).unwrap();
        let _uncompressed_size = u32::from_le_bytes(uncompressed_size_as_bytes);

        //file.seek(SeekFrom::Current(8))?;

        // Read the specified number of bytes following the offset
        let mut buffer = vec![0; compressed_size as usize];
        file.read_exact(&mut buffer)?;

        Ok(buffer)
    }

    // Function to read bytes from a file at a specified offset
    pub fn read_bytes_from_file(
        file_path: &str,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, io::Error> {
        // Open the file
        let mut file = File::open(file_path)?;

        // Seek to the specified offset
        file.seek(SeekFrom::Start(offset))?;

        // Read the specified number of bytes
        let mut buffer = vec![0u8; size as usize];
        file.read_exact(&mut buffer)?;

        Ok(buffer)
    }

    pub fn read_bytes_from_memory(input_data: &[u8], offset: usize, size: usize) -> Vec<u8> {
        // Ensure that the offset is within bounds
        if offset >= input_data.len() {
            return Vec::new(); // Return an empty vector if offset is out of bounds
        }

        // Calculate the end index based on offset and size
        let end_index = offset + size.min(input_data.len() - offset);

        // Create a Vec<u8> from the sliced portion
        input_data[offset..end_index].to_vec()
    }

    pub fn lz4_decompress(input_data: &[u8], size: usize) -> Result<Vec<u8>, io::Error> {
        lz4::block::decompress(&input_data, Some(size as i32))
    }
}
