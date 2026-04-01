// dialogs.rs — Permission dialogs and confirmation dialogs.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

// ---------------------------------------------------------------------------
// Permission dialog types
// ---------------------------------------------------------------------------

/// A single option inside a permission request dialog.
#[derive(Debug, Clone)]
pub struct PermissionOption {
    pub label: String,
    pub key: char,
}

/// State for an in-flight permission request popup.
///
/// This struct is intentionally richer than the legacy version to match the
/// TS permission dialog: it carries the command/path preview, a danger
/// explanation, and a stable set of TS-compatible options.
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    pub tool_use_id: String,
    pub tool_name: String,
    /// Short verb phrase, e.g. "wants to run: `rm -rf /tmp/foo`"
    pub description: String,
    /// One-sentence danger explanation shown in yellow.
    pub danger_explanation: String,
    /// The raw command / path / URL (displayed in a code-block style line).
    pub input_preview: Option<String>,
    pub options: Vec<PermissionOption>,
    pub selected_option: usize,
}

impl PermissionRequest {
    /// Create a standard four-option dialog matching the TS dialog options:
    ///   `y` — Yes, allow once
    ///   `Y` — Yes, allow this session
    ///   `p` — Yes, always allow (persistent)
    ///   `n` — No, deny
    pub fn standard(tool_use_id: String, tool_name: String, description: String) -> Self {
        Self {
            tool_use_id,
            tool_name,
            description: description.clone(),
            danger_explanation: String::new(),
            input_preview: None,
            selected_option: 0,
            options: Self::default_options(),
        }
    }

    /// Build with a richer description derived from the full permission reason
    /// text produced by `cc_core::format_permission_reason`.
    ///
    /// The `reason` string may contain a newline splitting the one-liner from
    /// the danger explanation — this constructor splits on the first `\n` and
    /// places each part in the right field.
    pub fn from_reason(
        tool_use_id: String,
        tool_name: String,
        reason: String,
        input_preview: Option<String>,
    ) -> Self {
        let (description, danger_explanation) = if let Some(nl) = reason.find('\n') {
            (reason[..nl].to_string(), reason[nl + 1..].to_string())
        } else {
            (reason, String::new())
        };

        Self {
            tool_use_id,
            tool_name,
            description,
            danger_explanation,
            input_preview,
            selected_option: 0,
            options: Self::default_options(),
        }
    }

    /// The four canonical options (matches TS interactive permission dialog).
    pub fn default_options() -> Vec<PermissionOption> {
        vec![
            PermissionOption {
                label: "Yes, allow once".to_string(),
                key: 'y',
            },
            PermissionOption {
                label: "Yes, allow this session".to_string(),
                key: 'Y',
            },
            PermissionOption {
                label: "Yes, always allow (persistent)".to_string(),
                key: 'p',
            },
            PermissionOption {
                label: "No, deny".to_string(),
                key: 'n',
            },
        ]
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// Compute a centred `Rect` of the given `width` × `height` inside `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn word_wrap(text: &str, width: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;
    if width == 0 || UnicodeWidthStr::width(text) <= width {
        return vec![text.to_string()];
    }
    let mut result = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0usize;
    for word in text.split_whitespace() {
        let word_w = UnicodeWidthStr::width(word);
        if current_width == 0 {
            current_line.push_str(word);
            current_width = word_w;
        } else if current_width + 1 + word_w <= width {
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_w;
        } else {
            result.push(std::mem::take(&mut current_line));
            current_line.push_str(word);
            current_width = word_w;
        }
    }
    if !current_line.is_empty() {
        result.push(current_line);
    }
    if result.is_empty() {
        result.push(text.to_string());
    }
    result
}

// ---------------------------------------------------------------------------
// Main render function
// ---------------------------------------------------------------------------

/// Render a permission-request dialog as a centred overlay.
///
/// Layout (top → bottom):
///   ┌─ Permission Required ─────────────────────────┐
///   │                                                │
///   │  Tool: Bash                                    │
///   │                                                │
///   │  > rm -rf /tmp/foo                             │
///   │                                                │
///   │  Bash wants to run: `rm -rf /tmp/foo`          │
///   │  This will delete files permanently.           │
///   │                                                │
///   │  [y] Yes, allow once                           │
///   │  [Y] Yes, allow this session                   │
///   │▶ [p] Yes, always allow (persistent)            │
///   │  [n] No, deny                                  │
///   └────────────────────────────────────────────────┘
pub fn render_permission_dialog(frame: &mut Frame, pr: &PermissionRequest, area: Rect) {
    let inner_width = 62u16;
    let dialog_width = inner_width.min(area.width.saturating_sub(4));
    let text_width = (dialog_width as usize).saturating_sub(4); // 2 border + 2 padding

    // Count how many lines we need
    let desc_lines = word_wrap(&pr.description, text_width);
    let expl_lines = if pr.danger_explanation.is_empty() {
        vec![]
    } else {
        word_wrap(&pr.danger_explanation, text_width)
    };

    // preview line count
    let preview_line_count: u16 = if pr.input_preview.is_some() { 3 } else { 0 };

    let content_lines: u16 = 2 // "  Tool: <name>"  +  blank
        + desc_lines.len() as u16
        + if !expl_lines.is_empty() { expl_lines.len() as u16 + 1 } else { 0 }
        + preview_line_count
        + 1 // blank before options
        + pr.options.len() as u16
        + 1; // trailing blank

    let dialog_height = (content_lines + 2) // +2 for top/bottom border
        .min(area.height.saturating_sub(4));

    let dialog_area = centered_rect(dialog_width, dialog_height, area);

    frame.render_widget(Clear, dialog_area);

    let mut lines: Vec<Line> = Vec::new();

    // ---- Tool name header ---------------------------------------------------
    lines.push(Line::from(vec![
        Span::raw("  Tool: "),
        Span::styled(
            pr.tool_name.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    // ---- Input preview (code-block style) -----------------------------------
    if let Some(ref preview) = pr.input_preview {
        lines.push(Line::from(vec![
            Span::styled(
                "  \u{276F} ",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                preview.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
    }

    // ---- Description (word-wrapped) -----------------------------------------
    for desc_line in &desc_lines {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::raw(desc_line.clone()),
        ]));
    }

    // ---- Danger explanation (yellow) ----------------------------------------
    if !expl_lines.is_empty() {
        for expl_line in &expl_lines {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    expl_line.clone(),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }
        lines.push(Line::from(""));
    }

    // ---- Options ------------------------------------------------------------
    for (i, opt) in pr.options.iter().enumerate() {
        let is_selected = i == pr.selected_option;
        let prefix = if is_selected { "  \u{25BA} " } else { "    " };
        let key_style = if is_selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let label_style = if is_selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::raw(prefix),
            Span::styled(format!("[{}]", opt.key), key_style),
            Span::raw(" "),
            Span::styled(opt.label.clone(), label_style),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Permission Required ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Yellow));

    let para = Paragraph::new(lines).block(block);
    frame.render_widget(para, dialog_area);
}

// ---------------------------------------------------------------------------
// T2-6: Tool-specific permission request dialogs
// ---------------------------------------------------------------------------

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::Wrap;

/// Which tool-specific permission dialog is active.
#[derive(Debug, Clone)]
pub enum ToolPermissionKind {
    /// Bash command execution.
    Bash { command: String },
    /// File edit: show diff of proposed changes.
    FileEdit { path: String, diff: String },
    /// File write: show new file content.
    FileWrite { path: String, content_preview: String },
    /// File read: show path + line range.
    FileRead { path: String, line_range: Option<(u32, u32)> },
    /// Web fetch: show URL + domain risk.
    WebFetch { url: String, is_high_risk: bool },
    /// PowerShell script execution.
    PowerShell { script: String },
    /// Ask user a question (from AskUserQuestion tool).
    AskUser { question: String, choices: Vec<String> },
    /// MCP server elicitation (schema-driven form).
    Elicitation { server: String, title: String, fields: Vec<ElicitationField> },
}

/// A single field in an elicitation form.
#[derive(Debug, Clone)]
pub struct ElicitationField {
    pub name: String,
    pub description: String,
    pub field_type: ElicitationFieldType,
    pub required: bool,
    pub value: String, // current input value
}

/// Type of an elicitation field.
#[derive(Debug, Clone)]
pub enum ElicitationFieldType {
    Text,
    Number,
    Bool,
    Select(Vec<String>), // options
}

/// State for a tool-specific permission dialog.
#[derive(Debug, Clone)]
pub struct ToolPermissionDialog {
    /// What kind of dialog this is.
    pub kind: ToolPermissionKind,
    /// Currently focused button (0=Allow, 1=AlwaysAllow, 2=Deny).
    pub focused_button: usize,
    /// Scroll offset for content that overflows.
    pub scroll: u16,
    /// For Elicitation: which field is focused.
    pub focused_field: usize,
}

impl ToolPermissionDialog {
    pub fn new(kind: ToolPermissionKind) -> Self {
        Self { kind, focused_button: 0, scroll: 0, focused_field: 0 }
    }

    /// Move focus to next button.
    pub fn next_button(&mut self) {
        self.focused_button = (self.focused_button + 1) % 3;
    }

    /// Move focus to previous button.
    pub fn prev_button(&mut self) {
        self.focused_button = (self.focused_button + 2) % 3;
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll += 1;
    }
}

/// Render a tool-specific permission dialog as a centered overlay.
pub fn render_tool_permission_dialog(dialog: &ToolPermissionDialog, frame: &mut Frame) {
    let area = centered_dialog_area(frame.area(), 70, 20);
    frame.render_widget(Clear, area);

    let (title, content_lines) = build_dialog_content(dialog);

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner: content area + button row at bottom.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(inner);

    // Content area.
    let content_text = content_lines.join("\n");
    let para = Paragraph::new(content_text)
        .wrap(Wrap { trim: false })
        .scroll((dialog.scroll, 0))
        .style(Style::default().fg(Color::White));
    frame.render_widget(para, chunks[0]);

    // Button row.
    render_permission_buttons(dialog.focused_button, chunks[1], frame);
}

fn build_dialog_content(dialog: &ToolPermissionDialog) -> (&'static str, Vec<String>) {
    match &dialog.kind {
        ToolPermissionKind::Bash { command } => (
            "Allow Bash Command?",
            vec![
                "Command:".to_string(),
                format!("  $ {}", command),
            ],
        ),
        ToolPermissionKind::FileEdit { path, diff } => (
            "Allow File Edit?",
            {
                let mut lines = vec![format!("File: {}", path), String::new()];
                for line in diff.lines().take(30) {
                    lines.push(line.to_string());
                }
                lines
            },
        ),
        ToolPermissionKind::FileWrite { path, content_preview } => (
            "Allow File Write?",
            {
                let mut lines = vec![format!("File: {}", path), String::new()];
                for line in content_preview.lines().take(20) {
                    lines.push(format!("  {}", line));
                }
                lines
            },
        ),
        ToolPermissionKind::FileRead { path, line_range } => (
            "Allow File Read?",
            vec![
                format!("File: {}", path),
                match line_range {
                    Some((s, e)) => format!("Lines: {} \u{2013} {}", s, e),
                    None => "Full file".to_string(),
                },
            ],
        ),
        ToolPermissionKind::WebFetch { url, is_high_risk } => (
            "Allow Web Fetch?",
            vec![
                format!("URL: {}", url),
                if *is_high_risk {
                    "\u{26a0} Domain may be high-risk".to_string()
                } else {
                    "Domain appears safe".to_string()
                },
            ],
        ),
        ToolPermissionKind::PowerShell { script } => (
            "Allow PowerShell?",
            {
                let mut lines = vec!["Script:".to_string()];
                for line in script.lines().take(20) {
                    lines.push(format!("  {}", line));
                }
                lines
            },
        ),
        ToolPermissionKind::AskUser { question, choices } => (
            "Agent Question",
            {
                let mut lines = vec![question.clone()];
                if !choices.is_empty() {
                    lines.push(String::new());
                    lines.push("Options:".to_string());
                    for (i, c) in choices.iter().enumerate() {
                        lines.push(format!("  {}. {}", i + 1, c));
                    }
                }
                lines
            },
        ),
        ToolPermissionKind::Elicitation { server, title, fields } => (
            "Server Input Request",
            {
                let mut lines = vec![
                    format!("Server: {}", server),
                    format!("Request: {}", title),
                    String::new(),
                ];
                for f in fields {
                    lines.push(format!("  {} {}: {}", if f.required { "*" } else { " " }, f.name, f.value));
                }
                lines
            },
        ),
    }
}

fn render_permission_buttons(focused: usize, area: Rect, frame: &mut Frame) {
    let buttons = ["Allow", "Always Allow", "Deny"];
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    for (i, (label, chunk)) in buttons.iter().zip(chunks.iter()).enumerate() {
        let style = if i == focused {
            Style::default().fg(Color::Black).bg(if i == 2 { Color::Red } else { Color::Green })
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let btn = Paragraph::new(format!(" [ {} ] ", label))
            .style(style)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(btn, *chunk);
    }
}

/// Compute a centered rect of the given percentage size.
fn centered_dialog_area(r: Rect, percent_x: u16, min_height: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - min_height.min(80)) / 2),
            Constraint::Min(min_height),
            Constraint::Percentage((100 - min_height.min(80)) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_permission_request_has_four_options() {
        let pr = PermissionRequest::standard(
            "id1".to_string(),
            "Bash".to_string(),
            "Run a shell command".to_string(),
        );
        assert_eq!(pr.options.len(), 4);
        assert_eq!(pr.options[0].key, 'y');
        assert_eq!(pr.options[1].key, 'Y');
        assert_eq!(pr.options[2].key, 'p');
        assert_eq!(pr.options[3].key, 'n');
    }

    #[test]
    fn from_reason_splits_on_newline() {
        let pr = PermissionRequest::from_reason(
            "id2".to_string(),
            "Bash".to_string(),
            "Bash wants to run: `rm -rf /tmp`\nThis will delete files permanently.".to_string(),
            Some("rm -rf /tmp".to_string()),
        );
        assert_eq!(pr.description, "Bash wants to run: `rm -rf /tmp`");
        assert_eq!(pr.danger_explanation, "This will delete files permanently.");
        assert_eq!(pr.input_preview.as_deref(), Some("rm -rf /tmp"));
    }

    #[test]
    fn from_reason_no_newline() {
        let pr = PermissionRequest::from_reason(
            "id3".to_string(),
            "WebFetch".to_string(),
            "WebFetch wants to fetch: `https://example.com`".to_string(),
            None,
        );
        assert_eq!(
            pr.description,
            "WebFetch wants to fetch: `https://example.com`"
        );
        assert!(pr.danger_explanation.is_empty());
    }

    #[test]
    fn word_wrap_short_text_unchanged() {
        let wrapped = word_wrap("hello world", 80);
        assert_eq!(wrapped, vec!["hello world"]);
    }

    #[test]
    fn word_wrap_long_text_splits() {
        let text = "one two three four five six seven eight";
        let wrapped = word_wrap(text, 10);
        for line in &wrapped {
            assert!(line.len() <= 10, "Line too long: {:?}", line);
        }
    }
}
