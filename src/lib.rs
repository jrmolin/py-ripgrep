use pyo3::prelude::*;
use pyo3::exceptions::{PyIndexError,};

use std::{
    collections::HashMap,
    io,
    path::Path,
};
use ignore::{WalkBuilder, DirEntry, Walk};
use thiserror::Error;

// start copied-in file

use {
    grep::{
        regex::{self, RegexMatcher},
        searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkError, SinkMatch},
    },
};


// end copied-in file
//
#[derive(Error, Debug)]
pub enum FinderError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Grep(#[from] regex::Error),
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),

    #[error("unknown what this is: {0}")]
    Unknown(String),
}

impl SinkError for FinderError {
    fn error_message<T: std::fmt::Display>(message: T) -> FinderError {
        FinderError::Unknown(message.to_string())
    }
}


type FinderResult<T> = Result<T, FinderError>;

impl From<FinderError> for PyErr {
    fn from(err: FinderError) -> PyErr {
        PyIndexError::new_err(format!("Finder Error: {err}"))
    }
}

#[derive(Default)]
struct Results {
    files_matched: Vec<String>,

    lines_matched: HashMap<String, Vec<String>>,
}

struct ResultsSink {
    path: String,
    matches: Vec<String>,
}

impl Results {
    pub fn add_file(&mut self, path: String) {
        self.files_matched.push(path);
    }

    pub fn update(&mut self, sink: ResultsSink) {
        if sink.matches.len() > 0 {
            self.files_matched.push(sink.path.clone());

            self.lines_matched.insert(sink.path, sink.matches);

        }
    }
}

impl ResultsSink {
    fn new(path: &Path) -> Self
    {
        Self {
            path: path.to_string_lossy().to_string(),
            matches: Vec::new(),
        }
    }

}

impl Sink for ResultsSink {
    type Error = FinderError;
    // define error
    // define fn matched
    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch) -> Result<bool, Self::Error> {
        let buffer = match std::str::from_utf8(mat.bytes()) {
            Ok(matched) => matched,
            Err(err) => return Err(err.into()),
        };
        let lineno = match mat.line_number() {
            Some(line_number) => line_number,
            // shouldn't hit this, because we forcibly turn on line numbers
            None => return Err(FinderError::Unknown(format!("line numbers not configured"))),
        };

        self.matches.push(format!("{lineno} :: {buffer}"));
        Ok(true)
    }

}
    

///
/// finder = Finder([".", "/usr/lib", "/usr/include", "~/git"])
/// finder.add_path("~/sandbox")
/// finder.files_only = True
/// find.add_search("some string part", ignore_case=True)
/// # ?? async behavior?
/// files = await find.run()
#[derive(Clone)]
struct Needle {
    data: String,
    matcher: RegexMatcher,
}

struct FinderInner {
    dirs: Vec<String>,
    searches: Vec<Needle>,
}

#[pyclass]
struct Finder {
    dirs: Vec<String>,
    searches: Vec<Needle>,
}

impl Needle {
    fn new(data: String) -> FinderResult<Self> {
        let matcher = RegexMatcher::new_line_matcher(&data)?;
        Ok(Self {
            data: data,
            matcher: matcher,
        })
    }
}

fn is_dir(d: &DirEntry) -> bool {
    match d.file_type() {
        Some(v) => v.is_dir(),
        None => true,
    }
}

fn find_files_without_match(walker: Walk, results: &mut Results)
{
    for result in walker {
        let entry = result.unwrap();
        if is_dir(&entry) {
            continue;
        }

        let owned = entry.into_path();
        match owned.to_str() {
            Some(s) => {
                results.add_file(String::from(s));
            },
            None => continue,
        }
    }
}

impl FinderInner {
    fn new(paths: &Vec<String>, searches: &Vec<Needle>) -> Self {
        Self {
            dirs: paths.clone(),
            searches: searches.clone(),
        }
    }

    fn build_walker(&self) -> Walk {
        // get an iterator over our directories
        let mut walker_builder = WalkBuilder::new(&self.dirs[0]);
        walker_builder
            .standard_filters(true)
            .hidden(false)
            .filter_entry(|entry| skip_git(entry));

        for path_str in self.dirs.iter().skip(1) {
            walker_builder.add(path_str);
        }
        walker_builder.build()
    }

    pub fn find_files(&self) -> FinderResult<Results> {
        let walker = self.build_walker();

        let mut results = Results::default();
        find_files_without_match(walker, &mut results);

        Ok(results)
    }

    // async arun
    // sync run
    fn search(&self) -> FinderResult<Results> {
        let walker = self.build_walker();
        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .line_number(true)
            .build();
        let mut results = Results::default();

        for entry in walker {
            if let Ok(file) = entry {
                if is_dir(&file) {
                    continue;
                }
                let mut sink = ResultsSink::new(file.path());
                for matcher in &self.searches {
                    let result = searcher.search_path(
                        &matcher.matcher,
                        file.path(),
                        &mut sink,
                    );
                    if let Err(err) = result {
                        eprintln!("{}: {}", file.path().display(), err);
                    }
                }

                results.update(sink);
            }
        }
        Ok(results)
    }
}

#[pymethods]
impl Finder {
    #[new]
    #[pyo3(text_signature = "(paths : [])")]
    fn new(paths: Vec<String>) -> Self {
        Self {
            dirs: paths,
            searches: Vec::new(),
        }
    }

    #[pyo3(text_signature = "(search : String)")]
    pub fn add_regex(&mut self, search: String) -> PyResult<usize> {
        self.searches.push(Needle::new(search)?);
        Ok(self.searches.len())
    }

    pub fn find_files(&self) -> PyResult<Vec<String>> {
        let finder = FinderInner::new(&self.dirs, &self.searches);

        match finder.find_files() {
            Ok(r) => {
                Ok(r.files_matched)
            }
            Err(e) => Err(e.into())
        }

    }

    // async arun
    // sync run
    fn search(&self) -> PyResult<HashMap<String, Vec<String>>> {
        let finder = FinderInner::new(&self.dirs, &self.searches);

        let results = finder.search()?;

        Ok(results.lines_matched)
    }
}

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

/// then make a pymodule, pyfunction that returns a list of strings

/// A Python module implemented in Rust.
#[pymodule]
fn py_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Finder>()?;
    Ok(())
}
