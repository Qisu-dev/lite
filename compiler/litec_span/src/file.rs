use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::Span;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub usize);

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub path: PathBuf,
    pub content: Arc<str>,
    pub line_offsets: Box<[usize]>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, path: PathBuf, content: Arc<str>) -> Self {
        let mut line_offsets = vec![0];
        let mut pos = 0;
        for ch in content.chars() {
            pos += ch.len_utf8();
            if ch == '\n' {
                line_offsets.push(pos);
            }
        }
        if *line_offsets.last().unwrap() != pos {
            line_offsets.push(pos);
        }
        Self {
            name: name.into(),
            path,
            content,
            line_offsets: line_offsets.into_boxed_slice(),
        }
    }

    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let line_idx = self
            .line_offsets
            .binary_search(&offset)
            .unwrap_or_else(|i| i - 1);
        let line_start = self.line_offsets[line_idx];
        let col = offset - line_start;
        (line_idx, col)
    }

    pub fn line(&self, line: usize) -> Option<&str> {
        if line >= self.line_offsets.len() - 1 {
            return None;
        }
        let start = self.line_offsets[line];
        let end = self.line_offsets[line + 1];
        Some(&self.content[start..end].trim_end_matches('\n'))
    }
}

#[derive(Debug)]
pub struct SourceMap {
    inner: RwLock<SourceMapInner>,
}

#[derive(Debug)]
struct SourceMapInner {
    files: Vec<Arc<SourceFile>>,
    by_path: HashMap<PathBuf, FileId>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(SourceMapInner {
                files: Vec::new(),
                by_path: HashMap::new(),
            }),
        }
    }

    /// 注册文件——同一路径幂等，返回已有 FileId。
    pub fn add_file(&self, path: impl Into<PathBuf>, content: Arc<str>) -> FileId {
        let path = path.into();
        let path = dunce::canonicalize(&path).unwrap_or(path);
        let mut inner = self.inner.write().unwrap();

        // 已注册——返回旧的
        if let Some(&fid) = inner.by_path.get(&path) {
            return fid;
        }

        let id = FileId(inner.files.len());
        let name = path
            .file_name()
            .and_then(|os| os.to_str())
            .map(String::from)
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        let file = Arc::new(SourceFile::new(name, path.clone(), content));
        inner.files.push(file);
        inner.by_path.insert(path, id);
        id
    }

    pub fn file_id_for(&self, path: &Path) -> Option<FileId> {
        self.inner.read().unwrap().by_path.get(path).copied()
    }

    pub fn file(&self, file_id: FileId) -> Option<Arc<SourceFile>> {
        self.inner.read().unwrap().files.get(file_id.0).cloned()
    }

    pub fn file_path(&self, file_id: FileId) -> Option<PathBuf> {
        self.inner
            .read()
            .unwrap()
            .files
            .get(file_id.0)
            .map(|f| f.path.clone())
    }

    pub fn line_col(&self, span: &Span) -> Option<(usize, usize)> {
        let file = self.file(span.file_id)?;
        Some(file.line_col(span.start))
    }

    pub fn snippet(&self, span: &Span) -> Option<String> {
        let file = self.file(span.file_id)?;
        file.content.get(span.start..span.end).map(String::from)
    }

    /// 显示用路径 —— 相对 CWD。
    /// 存的是绝对路径，这里是展示层。
    pub fn display_path(&self, file_id: FileId) -> String {
        let abs = match self.file_path(file_id) {
            Some(p) => p,
            None => return "<unknown>".into(),
        };
        display_path(&abs)
    }
}

pub fn display_path(abs: &Path) -> String {
    if let Ok(cwd) = std::env::current_dir() {
        let cwd = dunce::canonicalize(&cwd).unwrap_or(cwd);
        if let Ok(rel) = abs.strip_prefix(&cwd) {
            return rel.display().to_string();
        }
    }
    abs.display().to_string()
}
