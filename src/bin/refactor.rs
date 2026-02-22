use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Modules(HashMap<String, String>); // key = file path, value = content

fn main() -> Result<()> {
    // 1️⃣ Read JSON
    let json_path = "module_structure.json"; // path to JSON
    let json_data = fs::read_to_string(json_path)
        .with_context(|| format!("Failed to read JSON file: {}", json_path))?;

    let modules: HashMap<String, String> = serde_json::from_str(&json_data)
        .with_context(|| "Failed to parse JSON into HashMap<String,String>")?;

    // 2️⃣ Write files
    for (path, content) in modules {
        let path_obj = Path::new(&path);
        if let Some(parent) = path_obj.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }
        fs::write(&path, content).with_context(|| format!("Failed to write file {:?}", path))?;
        println!("✅ Wrote {}", path);
    }

    println!("All modules written successfully!");
    Ok(())
}
