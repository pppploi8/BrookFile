use std::path::{Component, Path, PathBuf};

fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                result.push(comp);
            }
        }
    }
    result
}

pub fn is_path_under_root(requested_path: &Path, root_path: &Path) -> bool {
    let root_canonical = match root_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };

    if let Ok(requested_canonical) = requested_path.canonicalize() {
        return requested_canonical.starts_with(&root_canonical);
    }

    let mut path_to_check = requested_path.to_path_buf();
    let mut suffix = PathBuf::new();
    loop {
        if let Ok(canonical) = path_to_check.canonicalize() {
            let full = normalize_path(&canonical.join(&suffix));
            return full.starts_with(&root_canonical);
        }
        match path_to_check.file_name() {
            Some(name) => {
                suffix = PathBuf::from(name).join(&suffix);
            }
            None => return false,
        }
        if !path_to_check.pop() {
            return false;
        }
    }
}

pub fn is_recycle_bin_path_under_root(recycle_bin_path: &str, root_path: &str) -> bool {
    let rb = Path::new(recycle_bin_path);
    let root = Path::new(root_path);

    if let (Ok(rb_c), Ok(root_c)) = (rb.canonicalize(), root.canonicalize()) {
        return rb_c == root_c || rb_c.starts_with(&root_c);
    }

    let rb_n = normalize_path(rb);
    let root_n = normalize_path(root);
    if rb_n == root_n || rb_n.starts_with(&root_n) {
        for component in rb_n.strip_prefix(&root_n).unwrap_or(Path::new("")).components() {
            if matches!(component, Component::ParentDir) {
                return false;
            }
        }
        return true;
    }
    false
}

pub fn is_safe_path(path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    let path_obj = Path::new(path);
    for component in path_obj.components() {
        match component {
            std::path::Component::Prefix(_) => return false,
            std::path::Component::RootDir => return false,
            std::path::Component::CurDir => continue,
            std::path::Component::ParentDir => return false,
            std::path::Component::Normal(_) => continue,
        }
    }
    true
}

pub fn is_safe_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    if name == "." || name == ".." {
        return false;
    }
    if name.contains('\0') {
        return false;
    }
    if name.trim() != name {
        return false;
    }
    true
}

pub fn move_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            move_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
        std::fs::remove_dir(src)?;
    } else {
        std::fs::copy(src, dst)?;
        std::fs::remove_file(src)?;
    }
    Ok(())
}
