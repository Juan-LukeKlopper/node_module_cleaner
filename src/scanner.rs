use homedir::my_home;
use jwalk::WalkDir;

pub fn get_array() -> Vec<String> {
    let homedir = my_home().unwrap().unwrap();
    println!("Loading...");

    WalkDir::new(&homedir)
        .process_read_dir(|_, _, _, children| {
            children.iter_mut().for_each(|r| {
                if let Ok(entry) = r
                    && entry.file_type().is_dir()
                {
                    let name = entry.file_name().to_string_lossy();
                    match name.as_ref() {
                        "node_modules" | ".cache" | ".vscode" | ".local" | ".npm" | ".nvm"
                        | ".steam" | ".var" | ".cargo" | "caches" | "Caches" => {
                            entry.read_children_path = None;
                        }
                        _ => {}
                    }
                }
            });
        })
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| entry.file_type().is_dir() && entry.path().ends_with("node_modules"))
        .map(|entry| {
            entry
                .path()
                .to_str()
                .unwrap_or("")
                .to_string()
                .trim_start_matches(homedir.to_str().unwrap())
                .to_string()
        })
        .collect()
}
