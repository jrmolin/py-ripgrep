use pyo3::prelude::*;
use pyo3::types::PyList;

use std::ffi::OsStr;

use std::{path::Path};
use {ignore::{WalkBuilder, DirEntry,}};

fn skip_git(d: &DirEntry) -> bool
{
    // is this a directory?
    if let Some(file_type) = d.file_type() {
        if file_type.is_dir() {
            // is it '.git'?
            if d.file_name() == ".git" {
                return false;
            }

        }
    } else {
        // if we're here, we couldn't get a file type, so skip
        return false;
    }

    true

}

/// add the crate ignore
/// use ignore::Walk;
/// 
/// basic case:
/// for result in Walk::new("./") {
///     // Each item yielded by the iterator is either a directory entry or an
///     // error, so either print the path or the error.
///     match result {
///         Ok(entry) => println!("{}", entry.path().display()),
///         Err(err) => println!("ERROR: {}", err),
///     }
/// }

/// advanced case (if we don't want to ignore hidden files):
/// use ignore::WalkBuilder;
/// 
/// for result in WalkBuilder::new("./").hidden(false).build() {
///     println!("{:?}", result);
/// }

#[pyfunction]
fn walk(path: &str) -> Vec<String> {
    let walker = WalkBuilder::new(path)
        .standard_filters(true)
        .hidden(false).filter_entry(|entry| {
            skip_git(entry)
        }).build();
    let mut count = 0;
    let mut files = vec![];
    for result in walker {
        let entry = result.unwrap();
        let skip_this = match entry.file_type() {
            Some(v) => v.is_dir(),
            None => true,
        };
        if skip_this {
            continue;
        }
        // PathBuf
        let owned = entry.into_path();
        let stripped = match owned.strip_prefix(path) {
            Ok(s) => s,
            _ => &owned,
        };
        match stripped.to_str() {
            Some(s) => {
                files.push(String::from(s));
                count += 1;
            }
        ,
            None => continue,
        }
    }

    println!("found {count} files");
    files
}

/// then make a pymodule, pyfunction that returns a list of strings

/// A Python module implemented in Rust.
#[pymodule]
fn py_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(walk, m)?)?;
    Ok(())
}
