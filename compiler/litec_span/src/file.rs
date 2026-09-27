use std::{
    path::PathBuf,
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
    pub content: String,
    pub line_offsets: Box<[usize]>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, path: PathBuf, content: impl Into<String>) -> Self {
        let content = content.into();
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

/// 全局源码管理器#[derive(Debug, Default)]
#[derive(Debug)]
pub struct SourceMap {
    /// 内部加锁——外层 `Arc<SourceMap>` 共享不变。
    /// `SourceFile` 一旦创建就不可变，所以用 `Arc` 共享。
    files: RwLock<Vec<Arc<SourceFile>>>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self {
            files: RwLock::new(Vec::new()),
        }
    }

    pub fn add_file(&self, path: PathBuf, content: impl Into<String>) -> FileId {
        let mut files = self.files.write().unwrap();

        let id = FileId(files.len());
        let name = path
            .file_name()
            .and_then(|os| os.to_str())
            .map(String::from)
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        let file = Arc::new(SourceFile::new(name, path, content.into()));
        files.push(file);
        id
    }

    pub fn file(&self, file_id: FileId) -> Option<Arc<SourceFile>> {
        self.files.read().unwrap().get(file_id.0).cloned()
    }

    pub fn line_col(&self, span: &Span) -> Option<(usize, usize)> {
        let file = self.file(span.file_id)?;
        Some(file.line_col(span.start))
    }

    /// 返回 `String`——因为不能从锁守卫后借用。
    pub fn snippet(&self, span: &Span) -> Option<String> {
        let file = self.file(span.file_id)?;
        file.content.get(span.start..span.end).map(String::from)
    }
}
