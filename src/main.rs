extern crate toml;

use std::fs;
use std::io;
use std::path::Path;
use toml::Value;
use std::process::exit;

fn main() -> io::Result<()> {
    let config_path = "config.toml";

    if !Path::new(config_path).exists() {
        create_default_config(config_path)?;
    }

    let contents = read_file_contents(config_path)?;

    let config = parse_toml_contents(&contents, config_path)?;

    let directories = extract_directories(&config)?;

    if directories.is_empty() {
        handle_empty_directories(config_path)?;
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

fn handle_empty_directories(config_path: &str) {
    println!("No directories are configured. Please enter a new directory path:");
    let mut new_dir_path = String::new();
    io::stdin().read_line(&mut new_dir_path).expect("Failed to read user input");
    let new_dir_path = new_dir_path.trim();

    if !new_dir_path.is_empty() {
        let dir_exists = Path::new(new_dir_path).is_dir();

        if dir_exists {
            let mut new_config = Value::Table(toml::value::Table::new());
            new_config
                .as_table_mut()
                .unwrap()
                .insert("directories".to_string(), Value::Array(vec![Value::String(new_dir_path.to_string())]));

            let new_config_str = toml::to_string(&new_config).expect("Failed to generate default config");
            fs::write(config_path, new_config_str).expect("Failed to write to config.toml");

            println!("Directory added to config.toml.");
        } else {
            println!("The entered directory does not exist.");
        }
    } else {
        println!("Invalid directory path.");
    }
}

fn display_directory_info(directories: &[String]) {
    for (index, dir_path) in directories.iter().enumerate() {
        let dir_exists = Path::new(dir_path).is_dir();
        println!("Directory {}:\nPath: {}\nExists: {}\n", index + 1, dir_path, dir_exists);
    }
}
