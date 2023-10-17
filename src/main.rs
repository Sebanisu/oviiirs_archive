extern crate toml;

use std::fs;
use std::io;
use std::path::Path;
use toml::Value;
use std::process::exit;
use std::io::Write;

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

    let mut config = parse_toml_contents(&contents, config_path)?;

    let mut directories = extract_directories(&config)?;

    let user_choice = if directories.is_empty() {
        handle_empty_directories()
    } else {
        display_directory_info(&directories, &extract_chosen_directory(&config))
    };

	let chosen_directory = match user_choice {
		DirectorySelection::NewDirectory(new_dir) => {
			// Handle the case when a new directory is chosen
			println!("New directory selected: {}", new_dir);
			// Do something extra for the new directory
			directories.push(new_dir.clone());

			// Create the "directories" key if it doesn't exist
				if !config.as_table().unwrap().contains_key("directories") {
					config.as_table_mut().unwrap().insert("directories".to_string(), toml::Value::Array(Vec::new()));
				}

				// Update the "directories" array
				let directories_array = config.as_table_mut().unwrap().get_mut("directories").unwrap();
				if let toml::Value::Array(array) = directories_array {
					for dir in directories {
						array.push(toml::Value::String(dir));
					}
				} else {
					panic!("'directories' is not an array in the TOML file");
				}


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
			println!("Exiting the program");
			// Perform any necessary cleanup and exit the program
			// You can return a default value here or use a placeholder value
			exit(0);
		}
	};

	// Use the chosen_directory variable for any common logic
	if let Value::Table(table) = &mut config {
		table.insert("chosen_directory".to_string(), Value::String(chosen_directory.to_string()));
	} else {
		panic!("Root of config.toml is not a table");
	}
	save_config(&config,config_path)?;

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
        let dirs: io::Result<Vec<String>> = config_directories
            .as_array()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Invalid directories"))
            .and_then(|arr| {
                arr.iter()
                    .map(|dir| {
                        dir.as_str()
                            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Invalid directory path"))
                            .map(|dir_str| dir_str.to_string())
                    })
                    .collect::<Result<Vec<String>, io::Error>>()
            });
        dirs
    } else {
        Ok(Vec::new())
    }
}

fn extract_chosen_directory(config: &Value) -> Option<String> {
    if let Some(chosen_directory) = config.get("chosen_directory") {
        if let Some(chosen_directory_str) = chosen_directory.as_str() {
            Some(chosen_directory_str.to_string())
		} else {
			None // "chosen_directory" exists but is not a string
		}
    } else {
        None // "chosen_directory" does not exist in the config
    }
}


fn handle_empty_directories() -> DirectorySelection {
    println!("\nNo directories are configured.\nOptions:");	
    println!("  - Press Enter to Exit.");
	println!("  - Please enter a new directory path:");
    let mut new_dir_path = String::new();
    io::stdin().read_line(&mut new_dir_path).expect("Failed to read user input");
    let new_dir_path = new_dir_path.trim();
	loop {
		if !new_dir_path.is_empty() {
			let dir_exists = Path::new(new_dir_path).exists();

			if dir_exists {
				return DirectorySelection::NewDirectory(new_dir_path.to_string());
			} else {
				println!("The entered directory does not exist.");
			}
		} else {			
			return DirectorySelection::Exit;
		}	
	}
}

fn display_directory_info(directories: &Vec<String>, previously_chosen_directory: &Option<String>) -> DirectorySelection {
    loop {
		
		println!("\nSaved FF8 Directories:\n");
		
        for (index, dir_path) in directories.iter().enumerate() {
            let dir_exists = Path::new(dir_path).is_dir();
            if dir_exists {
				println!("{:>6}: {}", index + 1, dir_path);
            }
        }

        // Offer the option to enter a new directory
		
		
        println!("\nOptions:\n");
        println!("  - Enter 'N' to use a new directory");
		println!("  - Enter the number of the directory you want to choose (or '0' to exit):");
		match previously_chosen_directory{
			Some(s) => {
				println!("  - Press Enter to use the previously chosen directory: {}",s);
			},
			None => {
			}
		}
        

        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input).expect("Failed to read user input");

        user_input = user_input.trim().to_string();

		let is_condition_met = || {
			let user_empty = user_input.is_empty();
			let has_chosen_directory = previously_chosen_directory.is_some();

			if !user_empty {
				println!("User input is not empty.");
			}

			if !has_chosen_directory {
				println!("No previously chosen directory is available.");
			}

			let directory_condition = if let Some(directory_path) = &previously_chosen_directory {
				let path = Path::new(directory_path);
				let path_exists = path.exists();
				let is_directory = path.is_dir();

				if !path_exists {
					println!("Directory path does not exist: {:?}", directory_path);
				}

				if !is_directory {
					println!("Directory path is not a directory: {:?}", directory_path);
				}

				path_exists && is_directory
			} else {
				println!("No directory path available.");
				false
			};

			if !directory_condition {
				println!("Directory condition is not met.");
			}

			user_empty && has_chosen_directory && directory_condition
		};


		// Usage
		if is_condition_met() {
			// All conditions are true
		} else {
			// At least one condition is false
		}


		if is_condition_met()
		{
			return DirectorySelection::ExistingDirectory(previously_chosen_directory.as_ref().unwrap().clone());
		}
        else if user_input.eq_ignore_ascii_case("N") {
            // User wants to use a new directory
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

fn save_config(config: &toml::Value, filename: &str) -> Result<(), std::io::Error> {
    let config_str = toml::to_string(config)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to serialize updated config: {}", e)))?;

    let mut file = fs::File::create(filename)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to create the specified file: {}", e)))?;

    let mut buf_writer = io::BufWriter::new(&mut file);

    buf_writer.write_all(config_str.as_bytes())?;
    buf_writer.flush()?;
    Ok(())
}


