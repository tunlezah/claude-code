//! Message type renderers for the TUI.
//! Mirrors src/components/messages/ and src/components/Messages.tsx.
//!
//! Each message type has a dedicated render function. The top-level
//! `render_message()` dispatcher routes to the correct renderer based
//! on message content.

use cc_core::types::{ContentBlock, Message, Role, ToolResultContent};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

mod markdown;
pub use markdown::render_markdown;

/// Context passed to all renderers.
pub struct RenderContext {
    /// Current terminal width (for word-wrap decisions).
    pub width: u16,
    /// Whether syntax highlighting is enabled.
    pub highlight: bool,
    /// Whether to show thinking blocks.
    pub show_thinking: bool,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            width: 80,
            highlight: true,
            show_thinking: false,
        }
    }
}

/// A styled line for rendering.
pub type StyledLine<'a> = Line<'a>;

/// Render a code block with optional language label. Uses basic styling
/// since full syntect integration is behind a feature flag.
pub fn render_code_block(lang: Option<&str>, code: &str, _width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let label = lang.unwrap_or("code");
    lines.push(Line::from(vec![Span::styled(
        format!("--- {} ", label),
        Style::default().fg(Color::DarkGray),
    )]));
    for line in code.lines() {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(line.to_string(), Style::default().fg(Color::White)),
        ]));
    }
    lines.push(Line::from(vec![Span::styled(
        "----------------".to_string(),
        Style::default().fg(Color::DarkGray),
    )]));
    lines
}

/// Render an assistant text message body.
pub fn render_assistant_text(text: &str, ctx: &RenderContext) -> Vec<Line<'static>> {
    render_markdown(text, ctx.width)
}

/// Render a user text message body.
fn render_user_text_with_ctx(text: &str, ctx: &RenderContext) -> Vec<Line<'static>> {
    render_markdown(text, ctx.width)
}

/// Legacy public helper retained for snapshot tests.
pub fn render_user_text(text: &str) -> Vec<Line<'static>> {
    render_user_text_with_ctx(text, &RenderContext::default())
}

/// Render a tool-use block (tool name + args summary).
pub fn render_tool_use(tool_name: &str, input_json: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("* ", Style::default().fg(Color::Yellow)),
        Span::styled(
            tool_name.to_string(),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
    ]));
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(input_json) {
        if let Some(obj) = val.as_object() {
            for (k, v) in obj.iter().take(5) {
                let v_str = match v {
                    serde_json::Value::String(s) => {
                        let short: String = s.chars().take(80).collect();
                        if s.len() > 80 { format!("{}...", short) } else { short }
                    }
                    _ => {
                        let s = v.to_string();
                        let short: String = s.chars().take(80).collect();
                        if s.len() > 80 { format!("{}...", short) } else { short }
                    }
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}: ", k), Style::default().fg(Color::DarkGray)),
                    Span::raw(v_str),
                ]));
            }
        }
    }
    lines
}

/// Render a tool result (success variant).
pub fn render_tool_result_success(output: &str, truncated: bool) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("+ ", Style::default().fg(Color::Green)),
        Span::styled(
            "Result",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
    ]));
    for (i, line) in output.lines().enumerate() {
        if i >= 20 {
            lines.push(Line::from(vec![Span::styled(
                "  ... (truncated)",
                Style::default().fg(Color::DarkGray),
            )]));
            break;
        }
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::raw(line.to_string()),
        ]));
    }
    if truncated {
        lines.push(Line::from(vec![Span::styled(
            "  ... output truncated",
            Style::default().fg(Color::DarkGray),
        )]));
    }
    lines
}

/// Render a tool result (error variant).
pub fn render_tool_result_error(error: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "x Error",
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )]));
    for line in error.lines().take(10) {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(line.to_string(), Style::default().fg(Color::Red)),
        ]));
    }
    lines
}

/// Render a "compact boundary" separator.
pub fn render_compact_boundary() -> Vec<Line<'static>> {
    vec![Line::from(vec![Span::styled(
        "----------- context compacted -----------",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
    )])]
}

/// Render a summary message (post-compact).
pub fn render_summary_message(text: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "Summary",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )]));
    for line in text.lines() {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(line.to_string(), Style::default().fg(Color::Gray)),
        ]));
    }
    lines
}

/// Render an unseen divider.
pub fn render_unseen_divider(count: usize) -> Vec<Line<'static>> {
    vec![Line::from(vec![Span::styled(
        format!("---- {} new message{} ----", count, if count == 1 { "" } else { "s" }),
        Style::default().fg(Color::Yellow),
    )])]
}

/// Render a system message (dimmed, italic).
pub fn render_system_message(text: &str) -> Vec<Line<'static>> {
    text.lines()
        .map(|line| {
            Line::from(vec![Span::styled(
                line.to_string(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )])
        })
        .collect()
}

/// Render a thinking block (collapsible - show header only when collapsed).
pub fn render_thinking_block(text: &str, expanded: bool) -> Vec<Line<'static>> {
    if !expanded {
        return vec![Line::from(vec![Span::styled(
            "> Thinking",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        )])];
    }
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "v Thinking",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
    )]));
    for line in text.lines() {
        lines.push(Line::from(vec![
            Span::styled("  | ", Style::default().fg(Color::DarkGray)),
            Span::styled(line.to_string(), Style::default().fg(Color::DarkGray)),
        ]));
    }
    lines
}

/// Render a rate-limit warning banner.
pub fn render_rate_limit_banner(retry_after_secs: u64) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![Span::styled(
            "Rate limit exceeded",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            format!("  Retrying in {}s...", retry_after_secs),
            Style::default().fg(Color::Yellow),
        )]),
    ]
}

/// Render a hook progress line (grey spinner + command).
pub fn render_hook_progress(command: &str, last_line: Option<&str>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("... ", Style::default().fg(Color::DarkGray)),
        Span::styled(command.to_string(), Style::default().fg(Color::DarkGray)),
    ]));
    if let Some(line) = last_line {
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", line),
            Style::default().fg(Color::DarkGray),
        )]));
    }
    lines
}

fn render_message_header(role: &Role, width: u16) -> Line<'static> {
    let (prefix, style) = match role {
        Role::User => (
            "You",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Role::Assistant => (
            "Claude",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let hr_len = (width as usize).saturating_sub(prefix.len() + 4);
    Line::from(vec![
        Span::styled(format!("  {} ", prefix), style),
        Span::styled("-".repeat(hr_len), Style::default().fg(Color::DarkGray)),
    ])
}

fn flush_text(lines: &mut Vec<Line<'static>>, role: &Role, text: &mut String, ctx: &RenderContext) {
    if text.is_empty() {
        return;
    }

    let rendered = match role {
        Role::User => render_markdown(text, ctx.width),
        Role::Assistant => render_assistant_text(text, ctx),
    };
    lines.extend(rendered);
    text.clear();
}

fn tool_result_text(content: &ToolResultContent) -> String {
    match content {
        ToolResultContent::Text(text) => text.clone(),
        ToolResultContent::Blocks(blocks) => {
            let joined = blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    ContentBlock::Thinking { thinking, .. } => Some(thinking.as_str()),
                    ContentBlock::RedactedThinking { .. } => Some("[redacted thinking]"),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            if joined.is_empty() {
                "[structured tool result]".to_string()
            } else {
                joined
            }
        }
    }
}

fn render_attachment_line(kind: &str, label: String) -> Vec<Line<'static>> {
    vec![Line::from(vec![
        Span::styled(
            format!("  {} ", kind),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        ),
        Span::styled(label, Style::default().fg(Color::DarkGray)),
    ])]
}

pub fn render_message(msg: &Message, ctx: &RenderContext) -> Vec<Line<'static>> {
    let mut lines = vec![render_message_header(&msg.role, ctx.width)];
    let mut pending_text = String::new();

    for block in msg.content_blocks() {
        match block {
            ContentBlock::Text { text } => {
                if !pending_text.is_empty() {
                    pending_text.push('\n');
                }
                pending_text.push_str(&text);
            }
            ContentBlock::Thinking { thinking, .. } => {
                flush_text(&mut lines, &msg.role, &mut pending_text, ctx);
                lines.extend(render_thinking_block(&thinking, ctx.show_thinking));
            }
            ContentBlock::RedactedThinking { .. } => {
                flush_text(&mut lines, &msg.role, &mut pending_text, ctx);
                lines.push(Line::from(vec![Span::styled(
                    "> Thinking redacted",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )]));
            }
            ContentBlock::ToolUse { name, input, .. } => {
                flush_text(&mut lines, &msg.role, &mut pending_text, ctx);
                lines.extend(render_tool_use(&name, &input.to_string()));
            }
            ContentBlock::ToolResult { content, is_error, .. } => {
                flush_text(&mut lines, &msg.role, &mut pending_text, ctx);
                let text = tool_result_text(&content);
                lines.extend(if is_error.unwrap_or(false) {
                    render_tool_result_error(&text)
                } else {
                    render_tool_result_success(&text, false)
                });
            }
            ContentBlock::Image { source } => {
                flush_text(&mut lines, &msg.role, &mut pending_text, ctx);
                let label = source
                    .url
                    .clone()
                    .or(source.media_type.clone())
                    .unwrap_or_else(|| "embedded image".to_string());
                lines.extend(render_attachment_line("Image", label));
            }
            ContentBlock::Document { title, context, source, .. } => {
                flush_text(&mut lines, &msg.role, &mut pending_text, ctx);
                let label = title
                    .or(context)
                    .or(source.url)
                    .or(source.media_type)
                    .unwrap_or_else(|| "attached document".to_string());
                lines.extend(render_attachment_line("Document", label));
            }
        }
    }

    flush_text(&mut lines, &msg.role, &mut pending_text, ctx);
    lines.push(Line::from(""));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.to_string()).collect::<String>()
    }

    #[test]
    fn render_message_uses_message_families_for_assistant_blocks() {
        let msg = Message::assistant_blocks(vec![
            ContentBlock::Thinking {
                thinking: "reasoning".to_string(),
                signature: "sig".to_string(),
            },
            ContentBlock::Text {
                text: "hello".to_string(),
            },
            ContentBlock::ToolUse {
                id: "tool-1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({ "path": "README.md" }),
            },
            ContentBlock::ToolResult {
                tool_use_id: "tool-1".to_string(),
                content: ToolResultContent::Text("file contents".to_string()),
                is_error: Some(false),
            },
        ]);
        let ctx = RenderContext {
            width: 80,
            highlight: true,
            show_thinking: false,
        };

        let rendered = render_message(&msg, &ctx)
            .into_iter()
            .map(|line| line_text(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Claude"));
        assert!(rendered.contains("Thinking"));
        assert!(rendered.contains("read_file"));
        assert!(rendered.contains("Result"));
        assert!(rendered.contains("hello"));
    }

    #[test]
    fn render_message_renders_user_text_under_user_header() {
        let msg = Message::user("hello from user");
        let ctx = RenderContext::default();

        let rendered = render_message(&msg, &ctx)
            .into_iter()
            .map(|line| line_text(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("You"));
        assert!(rendered.contains("hello from user"));
    }
}

