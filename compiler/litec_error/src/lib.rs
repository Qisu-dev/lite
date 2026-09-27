use annotate_snippets::{
    AnnotationKind, Group, Level, Patch, Renderer, Snippet, renderer::DecorStyle,
};
use litec_span::{DUMMY_SPAN, SourceMap, Span};

pub use crate::diag_ctxt::DiagCtxt;
pub mod diag_ctxt;

#[derive(Debug, Clone, Copy)]
pub struct ErrorGuaranteed(pub(crate) ());

impl ErrorGuaranteed {
    pub(crate) fn new() -> Self {
        Self(())
    }
}

pub type PResult<T> = Result<T, ErrorGuaranteed>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextLines {
    /// 上下各 1 行
    Compact,
    /// 上下各 3 行
    Standard,
    /// 无限制
    Full,
    /// 自定义行数
    Limited(usize),
}

impl Default for ContextLines {
    fn default() -> Self {
        ContextLines::Standard
    }
}

impl ContextLines {
    fn context_size(&self) -> usize {
        match self {
            ContextLines::Compact => 1,
            ContextLines::Standard => 3,
            ContextLines::Full => usize::MAX,
            ContextLines::Limited(n) => *n,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagLevel {
    Error,
    Warning,
    Note,
    Help,
}

impl DiagLevel {
    pub fn to_annotate_level(self) -> Level<'static> {
        match self {
            DiagLevel::Error => Level::ERROR,
            DiagLevel::Warning => Level::WARNING,
            DiagLevel::Note => Level::NOTE,
            DiagLevel::Help => Level::HELP,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SlicedSource {
    pub content: String,
    /// 0-based 起始行号
    pub start_line: usize,
    /// `content` 起始位置在源文件中的字节偏移
    pub base_offset: usize,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldEmptyLines {
    /// 不折叠
    Keep,
    /// 折叠连续空行，错误行附近保留 `context_empty` 个空行
    Fold {
        context_empty: usize,
        marker: &'static str,
    },
    /// 删除所有空行
    Remove,
}

impl Default for FoldEmptyLines {
    fn default() -> Self {
        FoldEmptyLines::Fold {
            context_empty: 1,
            marker: "...",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    /// 要替换的范围
    pub span: Span,
    /// 替换成什么
    pub replacement: String,
    /// 描述——`replace with ASCII space`
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct Diag {
    pub level: DiagLevel,
    pub message: String,
    pub code: Option<String>,

    /// 主 span —— `-->` 的位置
    pub span: Span,
    /// 附加 label —— 同文件的与主 span 一起渲染；跨文件的独立成组
    pub labels: Vec<Label>,

    pub notes: Vec<String>,
    pub help: Vec<String>,
    pub suggestions: Vec<Suggestion>,

    pub context_lines: ContextLines,
    pub fold_empty: FoldEmptyLines,
}

impl Diag {
    pub fn new(level: DiagLevel, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
            code: None,
            span: DUMMY_SPAN,
            labels: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
            context_lines: ContextLines::default(),
            fold_empty: FoldEmptyLines::default(),
            suggestions: Vec::new(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(DiagLevel::Error, message)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(DiagLevel::Warning, message)
    }

    pub fn note(message: impl Into<String>) -> Self {
        Self::new(DiagLevel::Note, message)
    }

    pub fn help(message: impl Into<String>) -> Self {
        Self::new(DiagLevel::Help, message)
    }

    pub fn with_suggestion(
        mut self,
        span: Span,
        replacement: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.suggestions.push(Suggestion {
            span,
            replacement: replacement.into(),
            description: description.into(),
        });
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// 设置主 span —— `-->` 指向的位置
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// 添加附加 label —— **不要**在这里写 `help:` / `note:` 前缀，渲染层会加
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
        });
        self
    }

    /// **不要**在这里写 `note:` 前缀
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// **不要**在这里写 `help:` 前缀
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help.push(help.into());
        self
    }

    pub fn with_context_lines(mut self, context: ContextLines) -> Self {
        self.context_lines = context;
        self
    }

    pub fn with_fold_empty(mut self, fold: FoldEmptyLines) -> Self {
        self.fold_empty = fold;
        self
    }

    pub fn render(&self, source_map: &SourceMap) -> String {
        let Some(sources) = self.prepare(source_map) else {
            return self.fallback_render();
        };
        let Some(groups) = self.build_report(&sources, source_map) else {
            return self.fallback_render();
        };

        let renderer = Renderer::styled()
            .anonymized_line_numbers(false)
            .decor_style(DecorStyle::Unicode);
        renderer.render(&groups)
    }

    pub fn render_to_string(&self, source_map: &SourceMap) -> String {
        self.render(source_map)
    }

    fn prepare(&self, source_map: &SourceMap) -> Option<Vec<SlicedSource>> {
        let source_file = source_map.file(self.span.file_id)?;
        let path_str = source_file.path.to_str()?;

        let start_lc = source_file.line_col(self.span.start);
        let end_lc = source_file.line_col(self.span.end);
        let error_start_line = start_lc.0;
        let error_end_line = end_lc.0;

        let total_lines = if source_file.line_offsets.len() > 1 {
            source_file.line_offsets.len() - 1
        } else {
            1
        };

        let context = self.context_lines.context_size();
        let display_start = error_start_line.saturating_sub(context);
        let display_end = (error_end_line + context).min(total_lines.saturating_sub(1));

        let main_sliced = self.extract_lines(
            &source_file.content,
            display_start,
            display_end,
            error_start_line,
            error_end_line,
        );
        let main_base_offset = source_file.line_offsets[display_start];

        let mut sources = vec![SlicedSource {
            content: main_sliced,
            start_line: display_start,
            base_offset: main_base_offset,
            path: path_str.to_string(),
        }];

        // 跨文件 label
        for label in &self.labels {
            if label.span.file_id == self.span.file_id {
                continue;
            }
            let Some(label_file) = source_map.file(label.span.file_id) else {
                continue;
            };
            let Some(label_path) = label_file.path.to_str() else {
                continue;
            };

            let label_start = label_file.line_col(label.span.start);
            let label_end = label_file.line_col(label.span.end);

            let label_display_start = label_start.0.saturating_sub(context);
            let label_display_end =
                (label_end.0 + context).min(label_file.line_offsets.len().saturating_sub(2));

            let label_slice = self.extract_lines(
                &label_file.content,
                label_display_start,
                label_display_end,
                label_start.0,
                label_end.0,
            );
            let label_base = label_file.line_offsets[label_display_start];

            sources.push(SlicedSource {
                content: label_slice,
                start_line: label_display_start,
                base_offset: label_base,
                path: label_path.to_string(),
            });
        }

        Some(sources)
    }

    fn extract_lines(
        &self,
        source: &str,
        start_line: usize,
        end_line: usize,
        error_start_line: usize,
        error_end_line: usize,
    ) -> String {
        match self.fold_empty {
            FoldEmptyLines::Keep => extract_lines_preserve_empty(source, start_line, end_line),
            FoldEmptyLines::Fold {
                context_empty,
                marker,
            } => extract_lines_fold_empty(
                source,
                start_line,
                end_line,
                error_start_line,
                error_end_line,
                context_empty,
                marker,
            ),
            FoldEmptyLines::Remove => extract_lines_remove_empty(source, start_line, end_line),
        }
    }

    fn build_report<'a>(
        &'a self,
        sources: &'a [SlicedSource],
        source_map: &'a SourceMap,
    ) -> Option<Vec<Group<'a>>> {
        let main_source = sources.first()?;
        let content_len = main_source.content.len();

        let (main_start, main_end) = self.adjust_span(self.span, main_source, content_len);

        let mut main_title = self.level.to_annotate_level().primary_title(&self.message);
        if let Some(code) = &self.code {
            main_title = main_title.id(code);
        }

        let mut main_snippet = Snippet::source(&main_source.content)
            .path(&main_source.path)
            .line_start(main_source.start_line + 1)
            .fold(false)
            .annotation(
                AnnotationKind::Primary
                    .span(main_start..main_end)
                    .label(&self.message),
            );

        for label in &self.labels {
            if label.span.file_id != self.span.file_id {
                continue;
            }
            let (ls, le) = self.adjust_span(label.span, main_source, content_len);
            main_snippet =
                main_snippet.annotation(AnnotationKind::Context.span(ls..le).label(&label.message));
        }

        let mut main_group = Group::with_title(main_title).element(main_snippet);

        for help in &self.help {
            main_group = main_group.element(Level::HELP.message(help.as_str()));
        }
        for note in &self.notes {
            main_group = main_group.element(Level::NOTE.message(note.as_str()));
        }

        let mut groups = vec![main_group];

        // 跨文件 label
        let mut source_idx = 1;
        for label in &self.labels {
            if label.span.file_id == self.span.file_id {
                continue;
            }
            let Some(label_source) = sources.get(source_idx) else {
                break;
            };
            source_idx += 1;

            let label_content_len = label_source.content.len();
            let (ls, le) = self.adjust_span(label.span, label_source, label_content_len);

            let label_title = Level::NOTE.secondary_title(&label.message);
            let label_snippet = Snippet::source(&label_source.content)
                .path(&label_source.path)
                .line_start(label_source.start_line + 1)
                .annotation(AnnotationKind::Primary.span(ls..le).label(&label.message));

            groups.push(Group::with_title(label_title).element(label_snippet));
        }

        // suggestion
        for sug in &self.suggestions {
            if sug.span.file_id != self.span.file_id {
                continue;
            }

            let Some(source_file) = source_map.file(sug.span.file_id) else {
                continue;
            };

            let (line, _) = source_file.line_col(sug.span.start);

            let line_start = source_file.line_offsets[line - 1];
            let line_end = source_file
                .line_offsets
                .get(line)
                .copied()
                .unwrap_or(source_file.content.len());

            let line_text = source_file.content[line_start as usize..line_end as usize]
                .trim_end_matches('\n')
                .trim_end_matches('\r');

            let whole_line_start = line_start.saturating_sub(main_source.base_offset) as usize;
            let whole_line_end = (line_start + line_text.len())
                .saturating_sub(main_source.base_offset)
                .min(content_len) as usize;

            let rel_start = (sug.span.start - line_start) as usize;
            let rel_end = (sug.span.end - line_start) as usize;
            let new_line = format!(
                "{}{}{}",
                &line_text[..rel_start],
                sug.replacement,
                &line_text[rel_end..],
            );

            let sug_snippet = Snippet::source(&main_source.content)
                .path(&main_source.path)
                .line_start(main_source.start_line + 1)
                .patch(Patch::new(whole_line_start..whole_line_end, new_line));

            let sug_group = Group::with_title(Level::HELP.primary_title(sug.description.as_str()))
                .element(sug_snippet);

            groups.push(sug_group);
        }

        Some(groups)
    }

    fn adjust_span(&self, span: Span, source: &SlicedSource, content_len: usize) -> (usize, usize) {
        let start = span
            .start
            .saturating_sub(source.base_offset)
            .min(content_len) as usize;
        let end = span.end.saturating_sub(source.base_offset).min(content_len) as usize;
        if start > end {
            (end, start)
        } else {
            (start, end)
        }
    }

    fn fallback_render(&self) -> String {
        let level_str = match self.level {
            DiagLevel::Error => "error",
            DiagLevel::Warning => "warning",
            DiagLevel::Note => "note",
            DiagLevel::Help => "help",
        };
        let code_str = self
            .code
            .as_ref()
            .map(|c| format!("[{}] ", c))
            .unwrap_or_default();
        format!("{}: {}{}", level_str, code_str, self.message)
    }
}

fn extract_lines_preserve_empty(source: &str, start_line: usize, end_line: usize) -> String {
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let end_line = end_line.min(lines.len().saturating_sub(1));
    if start_line >= lines.len() {
        return String::new();
    }
    lines[start_line..=end_line].concat()
}

fn extract_lines_fold_empty(
    source: &str,
    start_line: usize,
    end_line: usize,
    error_start_line: usize,
    error_end_line: usize,
    context_empty: usize,
    marker: &str,
) -> String {
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let end_line = end_line.min(lines.len().saturating_sub(1));
    if start_line >= lines.len() {
        return String::new();
    }

    let mut result = String::new();
    let mut consecutive_empty = 0;
    let mut last_was_folded = false;

    for (idx, line) in lines[start_line..=end_line].iter().enumerate() {
        let absolute_line = start_line + idx;
        let is_empty = line.trim().is_empty();

        let in_error_context = absolute_line + context_empty >= error_start_line
            && absolute_line <= error_end_line + context_empty;

        if is_empty {
            if in_error_context {
                result.push_str(line);
                consecutive_empty = 0;
                last_was_folded = false;
            } else {
                consecutive_empty += 1;
                if consecutive_empty == 1 && !last_was_folded {
                    result.push_str(marker);
                    result.push('\n');
                    last_was_folded = true;
                }
            }
        } else {
            consecutive_empty = 0;
            last_was_folded = false;
            result.push_str(line);
        }
    }

    result
}

fn extract_lines_remove_empty(source: &str, start_line: usize, end_line: usize) -> String {
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let end_line = end_line.min(lines.len().saturating_sub(1));
    if start_line >= lines.len() {
        return String::new();
    }
    lines[start_line..=end_line]
        .iter()
        .filter(|line| !line.trim().is_empty())
        .copied()
        .collect::<Vec<_>>()
        .concat()
}

pub fn error(message: impl Into<String>) -> Diag {
    Diag::error(message)
}

pub fn warning(message: impl Into<String>) -> Diag {
    Diag::warning(message)
}
