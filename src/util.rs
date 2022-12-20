use std::path::{Component, Path};

/// Function taken from the [zip](https://docs.rs/zip/0.6.3/src/zip/read.rs.html#896-911) crate source.
/// This function is used to sanitize the file name of a zip entry.F
pub fn sanitize_zip_filename(filename: &str) -> Option<&Path> {
    if filename.contains('\0') {
        return None;
    }
    let path = Path::new(filename);
    let mut dir_depth = 0usize;
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => return None,
            Component::ParentDir => dir_depth = dir_depth.checked_sub(1)?,
            Component::Normal(_) => dir_depth += 1,
            Component::CurDir => {}
        }
    }
    Some(path)
}
