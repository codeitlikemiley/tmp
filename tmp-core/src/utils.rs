pub fn is_binary_available(binary: &str) -> bool {
    let paths = match std::env::var_os("PATH") {
        Some(val) => std::env::split_paths(&val).collect::<Vec<_>>(),
        None => return false,
    };
    for mut path in paths {
        path.push(binary);
        if path.is_file() {
            return true;
        }
        if cfg!(target_os = "windows") {
            let mut exe_path = path.clone();
            exe_path.set_extension("exe");
            if exe_path.is_file() {
                return true;
            }
        }
    }
    false
}
