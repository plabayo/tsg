use core::ops::Range;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use regex::Regex;

use super::Meta;

#[derive(Debug, Copy, Clone)]
pub enum FileKind {
    Include,
    Layout,
    Page,
}

impl FileKind {
    pub fn from_str(s: &str) -> Result<FileKind> {
        Ok(match s.to_lowercase().as_str() {
            "includes" => FileKind::Include,
            "layouts" => FileKind::Layout,
            "pages" => FileKind::Page,
            kind => return Err(anyhow!("unexpected raw kind {}", kind)),
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub enum FileFormat {
    Html,
    Markdown,
    Yaml,
    Json,
    Rhai,
    Bash,
}

impl FileFormat {
    pub fn from_str(s: &str) -> std::result::Result<FileFormat, FileInfoError> {
        Ok(match s.to_lowercase().as_str() {
            "html" | "htm" | "xhtml" | "xml" => FileFormat::Html,
            "yaml" | "yml" => FileFormat::Yaml,
            "json" => FileFormat::Json,
            "rhai" => FileFormat::Rhai,
            "md" | "markdown" | "mdown" | "mkdn" | "mdwn" | "mdtxt" | "mdtext" | "text" | "rmd" => {
                FileFormat::Markdown
            }
            "sh" => FileFormat::Bash,
            _ => return Err(FileInfoError::UnexpectedFileFormat(String::from(s))),
        })
    }
}

#[derive(Debug, Clone)]
pub struct FileLocale {
    raw_str: String,
}

impl FileLocale {
    pub fn from_str(s: &str) -> FileLocale {
        FileLocale {
            raw_str: String::from(s),
        }
    }
}

#[derive(Clone)]
pub struct FileInfo {
    kind: FileKind,
    path: String,
    directory: Option<Range<usize>>,
    name: Range<usize>,
    locale: Option<FileLocale>,
    format: FileFormat,
}

impl FileInfo {
    pub fn new(raw_path: &str) -> std::result::Result<FileInfo, FileInfoError> {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"(?i)(?P<kind>includes|layouts|pages)(?P<dir>((/|\\)[^/\\]+)+)?(/|\\)(?P<name>\s+)(?P<locale>(\.[a-z\-_\d]+)+)?(\.(?P<ext>[a-z]+)$").unwrap();
        }
        // extract raw name, locale (opt) and extension (indicates file format)
        let (raw_kind, raw_dir, raw_name, raw_locale_opt, raw_ext, path) =
            match RE.captures(raw_path) {
                Some(m) => (
                    m.name("kind").unwrap(),
                    m.name("dir"), // dir is optional, and not defined if direct in root of kind
                    m.name("name").unwrap(),
                    m.name("locale"), // also the locale can be optional within this pattern
                    m.name("ext").unwrap(),
                    String::from(raw_path),
                ),
                None => return Err(FileInfoError::UnexpectedFilePath(String::from(raw_path))),
            };
        // "parse" the file format from the file extension
        let file_format = FileFormat::from_str(raw_ext.as_str())?;
        // optionally "parse" the locale from the locale part
        let locale = raw_locale_opt.and_then(|m| Some(FileLocale::from_str(m.as_str())));
        // "parse" the kind dir from file path, no need to do fancy here as the
        // regex above should have ensured it is one of our expected kinds
        let kind = FileKind::from_str(raw_kind.as_str()).unwrap();
        // optionally turn the dir into a String
        let directory = raw_dir.and_then(|dir| Some(dir.range()));

        // return the parsed File Info
        Ok(FileInfo {
            kind: kind,
            path: path,
            directory: directory,
            name: raw_name.range(),
            locale: locale,
            format: file_format,
        })
    }

    pub fn kind(&self) -> FileKind {
        self.kind
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn directory(&self) -> Option<&str> {
        self.directory
            .as_ref()
            .and_then(|range| Some(&self.path[range.start..range.end]))
    }

    pub fn name(&self) -> &str {
        &self.path[self.name.start..self.name.end]
    }

    pub fn locale(&self) -> Option<&FileLocale> {
        self.locale.as_ref()
    }

    pub fn format(&self) -> FileFormat {
        self.format
    }
}

impl TryFrom<&Path> for FileInfo {
    type Error = FileInfoError;

    fn try_from(path: &Path) -> std::result::Result<FileInfo, FileInfoError> {
        match path.to_str() {
            Some(path_str) => FileInfo::new(path_str),
            None => Err(FileInfoError::InvalidPath),
        }
    }
}

impl TryFrom<&PathBuf> for FileInfo {
    type Error = FileInfoError;

    fn try_from(path: &PathBuf) -> std::result::Result<FileInfo, FileInfoError> {
        match path.to_str() {
            Some(path_str) => FileInfo::new(path_str),
            None => Err(FileInfoError::InvalidPath),
        }
    }
}

#[derive(Debug)]
pub enum FileInfoError {
    UnexpectedFileFormat(String),
    InvalidPath,
    UnexpectedFilePath(String),
}

impl Error for FileInfoError {}

impl fmt::Display for FileInfoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileInfoError::UnexpectedFileFormat(ext) => {
                write!(f, "unexpected file format: {}", ext)
            }
            FileInfoError::InvalidPath => write!(f, "invalid file path"),
            FileInfoError::UnexpectedFilePath(path) => write!(f, "unexpected file path: {}", path),
        }
    }
}

pub struct File {
    file_info: FileInfo,
    meta: Option<Meta>,
    content: Vec<u8>,
}

impl File {
    pub fn read<P: AsRef<Path>>(path: P) -> Result<File> {
        let path = path.as_ref();
        let file_info: FileInfo = path.try_into()?;
        file_info.try_into()
    }

    pub fn info(&self) -> &FileInfo {
        &self.file_info
    }

    pub fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }

    pub fn content(&self) -> &[u8] {
        &self.content[..]
    }
}

impl TryFrom<FileInfo> for File {
    type Error = anyhow::Error;

    fn try_from(file_info: FileInfo) -> Result<File> {
        let mut content = fs::read(file_info.path())?;
        let meta = Meta::extract(file_info.format(), &mut content)?;
        Ok(File { file_info, meta, content })
    }
}
