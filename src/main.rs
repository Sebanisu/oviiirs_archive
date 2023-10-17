extern crate toml;

use std::fs;
use std::io;
use std::path::Path;
use toml::Value;
use std::process::exit;

enum DirectorySelection {
    NewDirectory(String),
    ExistingDirectory(String),
    Exit,
}

fn main() -> io::Result<()> {
    let config_path = "config.toml";

    if !Path::new(config_path).exists() {
        create_default_config(config_path)?;
    }

    let contents = read_file_contents(config_path)?;

    let config = parse_toml_contents(&contents, config_path)?;

    let directories = extract_directories(&config)?;

    if directories.is_empty() {
        handle_empty_directories();
    } else {
        display_directory_info(&directories);
    }

    Ok(())
}

fn create_default_config(config_path: &str) -> io::Result<()> {
    let default_config = r#"
    directories = []
    "#;

    fs::write(config_path, default_config)?;
    println!("config.toml created with default configuration.");
    Ok(())
}

fn read_file_contents(config_path: &str) -> io::Result<String> {
    fs::read_to_string(config_path).or_else(|_| {
        eprintln!("Could not read file `{}`", config_path);
        exit(1);
    })
}

fn parse_toml_contents(contents: &str, config_path: &str) -> io::Result<Value> {
    toml::from_str(contents).or_else(|_| {
        eprintln!("Unable to load data from `{}`", config_path);
        exit(1);
    })
}

fn extract_directories(config: &Value) -> io::Result<Vec<String>> {
    if let Some(config_directories) = config.get("directories") {
        config_directories
            .as_array()
            .ok_or(io::Error::new(io::ErrorKind::Other, "Invalid directories"))?
            .iter()
            .map(|dir| {
                dir.as_str()
                    .ok_or(io::Error::new(io::ErrorKind::Other, "Invalid directory path"))
                    .map(|dir_str| dir_str.to_string())
            })
            .collect::<Result<Vec<String>, _>>()
    } else {
        Ok(Vec::new())
    }
}

fn handle_empty_directories() -> DirectorySelection {
    println!("No directories are configured. Please enter a new directory path:");
    let mut new_dir_path = String::new();
    io::stdin().read_line(&mut new_dir_path).expect("Failed to read user input");
    let new_dir_path = new_dir_path.trim();

    if !new_dir_path.is_empty() {
        let dir_exists = Path::new(new_dir_path).exists();

        if dir_exists {
			return DirectorySelection::NewDirectory(new_dir_path.to_string());
        } else {
            println!("The entered directory does not exist.");
        }
    } else {
        println!("Invalid directory path.");
    }	
	return DirectorySelection::Exit;
}

fn append_directory_to_config(config_path: &str, new_dir_path: &str) {
	let mut new_config = Value::Table(toml::value::Table::new());
	new_config
		.as_table_mut()
		.unwrap()
		.insert("directories".to_string(), Value::Array(vec![Value::String(new_dir_path.to_string())]));

	let new_config_str = toml::to_string(&new_config).expect("Failed to generate default config");
	fs::write(config_path, new_config_str).expect("Failed to write to config.toml");

	println!("Directory added to config.toml.");
}

fn display_directory_info(directories: &Vec<String>) -> DirectorySelection {
    loop {
        for (index, dir_path) in directories.iter().enumerate() {
            let dir_exists = Path::new(dir_path).is_dir();
            if dir_exists {
                println!("{}: {}", index + 1, dir_path);
            }
        }

        // Offer the option to enter a new directory
        println!("Enter 'N' to add a new directory, or");
        println!("Enter the number of the directory you want to choose (or '0' to exit):");

        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input).expect("Failed to read user input");

        user_input = user_input.trim().to_string();

        if user_input.eq_ignore_ascii_case("N") {
            // User wants to add a new directory
            println!("Enter the path of the new directory:");
            let mut new_dir_path = String::new();
            io::stdin().read_line(&mut new_dir_path).expect("Failed to read user input");
            let new_dir_path = new_dir_path.trim().to_string();

            if !new_dir_path.is_empty() {
                if Path::new(&new_dir_path).is_dir() {
					return DirectorySelection::NewDirectory(new_dir_path);
                } else {
                    println!("Invalid directory path. Directory does not exist. No directory added.");
                }
            } else {
                println!("Invalid directory path. No directory added.");
            }
        } else {
            match user_input.parse::<usize>() {
                Ok(choice) if choice >= 1 && choice <= directories.len() => {
                    // User selected a valid directory
                    let selected_directory = &directories[choice - 1];
                    println!("You chose directory {}:\nPath: {}\n", choice, selected_directory);
                    // Now you can use 'selected_directory' for further processing.
                    return DirectorySelection::ExistingDirectory(selected_directory.clone());
                }
                Ok(0) => {
                    // User chose to exit
                    println!("Exiting...");
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


