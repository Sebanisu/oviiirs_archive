extern crate toml;

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::exit;

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
    let config_path = "config.toml";

    let contents = read_file_contents(config_path)?;

    //let mut config = parse_toml_contents(&contents, config_path)?;

    let mut config: Config = match toml::from_str::<Config>(&contents) {
        // If successful, return data as `Data` struct.
        // `data` is a local variable.
        Ok(data) => data,
        // Handle the `error` case.
        Err(_) => {
            // // Write `msg` to `stderr`.
            // eprintln!("Unable to load data from `{}`", config_path);
            // // Exit the program with exit code `1`.
            // exit(1);
            Default::default()
        }
    };

    config.locations.ensure_chosen_directory_in_directories();

    let directories = filter_valid_directories(&config.locations.directories);

    let user_choice = display_directory_info(&directories, &config.locations.chosen_directory);

    let chosen_directory = match user_choice {
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

    config.locations.chosen_directory = chosen_directory;

    save_config(&config, config_path)?;

    Ok(())
}

fn read_file_contents(config_path: &str) -> io::Result<String> {
    fs::read_to_string(config_path).or_else(|_| {
        eprintln!("Could not read file `{}`", config_path);
        Ok(String::new()) // Return an empty string in case of an error
    })
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

fn save_config(config: &Config, filename: &str) -> Result<(), std::io::Error> {
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
