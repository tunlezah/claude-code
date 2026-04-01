//! MCP server management UI.
//! Mirrors src/components/mcp/ (12 files).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

// ---------------------------------------------------------------------------
// Data types (view-level; mirrors cc_mcp types)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpViewStatus {
    Connected,
    Connecting,
    Disconnected,
    Error,
}

impl McpViewStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Connecting => "connecting",
            Self::Disconnected => "disconnected",
            Self::Error => "error",
        }
    }
    pub fn badge(&self) -> &'static str {
        match self {
            Self::Connected => "●",
            Self::Connecting => "◌",
            Self::Disconnected => "○",
            Self::Error => "⚠",
        }
    }
    pub fn color(&self) -> Color {
        match self {
            Self::Connected => Color::Green,
            Self::Connecting => Color::Yellow,
            Self::Disconnected => Color::DarkGray,
            Self::Error => Color::Red,
        }
    }
}

#[derive(Debug, Clone)]
pub struct McpServerView {
    pub name: String,
    pub transport: String, // "stdio" | "sse" | "http"
    pub status: McpViewStatus,
    pub tool_count: usize,
    pub resource_count: usize,
    pub prompt_count: usize,
    pub error_message: Option<String>,
    /// All tools provided by this server.
    pub tools: Vec<McpToolView>,
}

#[derive(Debug, Clone)]
pub struct McpToolView {
    pub name: String,
    pub server: String,
    pub description: String,
    pub input_schema: Option<String>,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpViewPane {
    ServerList,
    ToolList,
    ToolDetail,
}

#[derive(Debug, Clone)]
pub struct McpViewState {
    pub open: bool,
    pub servers: Vec<McpServerView>,
    pub active_pane: McpViewPane,
    pub selected_server: usize,
    pub selected_tool: usize,
    pub tool_search: String,
    pub server_scroll: usize,
    pub tool_scroll: usize,
}

impl McpViewState {
    pub fn new() -> Self {
        Self {
            open: false,
            servers: Vec::new(),
            active_pane: McpViewPane::ServerList,
            selected_server: 0,
            selected_tool: 0,
            tool_search: String::new(),
            server_scroll: 0,
            tool_scroll: 0,
        }
    }

    pub fn open(&mut self, servers: Vec<McpServerView>) {
        self.servers = servers;
        self.selected_server = 0;
        self.selected_tool = 0;
        self.tool_search.clear();
        self.active_pane = McpViewPane::ServerList;
        self.open = true;
    }

    pub fn close(&mut self) { self.open = false; }

    pub fn switch_pane(&mut self) {
        self.active_pane = match self.active_pane {
            McpViewPane::ServerList => McpViewPane::ToolList,
            McpViewPane::ToolList => McpViewPane::ToolDetail,
            McpViewPane::ToolDetail => McpViewPane::ServerList,
        };
    }

    pub fn select_prev(&mut self) {
        match self.active_pane {
            McpViewPane::ServerList => {
                if self.selected_server > 0 { self.selected_server -= 1; }
            }
            McpViewPane::ToolList | McpViewPane::ToolDetail => {
                if self.selected_tool > 0 { self.selected_tool -= 1; }
            }
        }
    }

    pub fn select_next(&mut self) {
        match self.active_pane {
            McpViewPane::ServerList => {
                if self.selected_server + 1 < self.servers.len() {
                    self.selected_server += 1;
                }
            }
            McpViewPane::ToolList | McpViewPane::ToolDetail => {
                let count = self.filtered_tools().len();
                if self.selected_tool + 1 < count {
                    self.selected_tool += 1;
                }
            }
        }
    }

    pub fn push_search_char(&mut self, c: char) {
        self.tool_search.push(c);
        self.selected_tool = 0;
    }

    pub fn pop_search_char(&mut self) {
        self.tool_search.pop();
        self.selected_tool = 0;
    }

    /// All tools across all servers, filtered by search query.
    pub fn filtered_tools(&self) -> Vec<&McpToolView> {
        let q = self.tool_search.to_lowercase();
        self.servers
            .iter()
            .flat_map(|s| s.tools.iter())
            .filter(|t| {
                q.is_empty()
                    || t.name.to_lowercase().contains(&q)
                    || t.description.to_lowercase().contains(&q)
                    || t.server.to_lowercase().contains(&q)
            })
            .collect()
    }
}

impl Default for McpViewState {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

pub fn render_mcp_view(state: &McpViewState, area: Rect, buf: &mut Buffer) {
    if !state.open { return; }

    let w = (area.width * 9 / 10).max(50).min(area.width);
    let h = (area.height * 4 / 5).max(15).min(area.height);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    let dialog = Rect { x, y, width: w, height: h };

    Clear.render(dialog, buf);
    Block::default()
        .title(" MCP Servers [Tab: pane, ↑↓: navigate, r: reconnect, Esc: close] ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan))
        .render(dialog, buf);

    let inner = Rect {
        x: dialog.x + 1,
        y: dialog.y + 1,
        width: dialog.width.saturating_sub(2),
        height: dialog.height.saturating_sub(2),
    };

    // Split: servers (left 35%) | tools (right 65%)
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(inner);

    render_server_list(state, panes[0], buf);

    // Right pane: tool list + tool detail
    let right_panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(panes[1]);

    render_tool_list(state, right_panes[0], buf);
    render_tool_detail(state, right_panes[1], buf);
}

fn render_server_list(state: &McpViewState, area: Rect, buf: &mut Buffer) {
    let focused = state.active_pane == McpViewPane::ServerList;
    let border_style = if focused { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) };
    Block::default().title(" Servers ").borders(Borders::ALL).border_style(border_style).render(area, buf);

    let inner = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(2) };

    // Group by transport
    let stdio: Vec<_> = state.servers.iter().enumerate().filter(|(_, s)| s.transport == "stdio").collect();
    let sse: Vec<_> = state.servers.iter().enumerate().filter(|(_, s)| s.transport == "sse").collect();
    let http: Vec<_> = state.servers.iter().enumerate().filter(|(_, s)| s.transport == "http").collect();

    let mut row = 0u16;

    let render_group = |group: &Vec<(usize, &McpServerView)>, label: &str, row: &mut u16, area: Rect, buf: &mut Buffer, selected: usize, focused: bool| {
        if group.is_empty() { return; }
        if *row >= area.height { return; }
        Paragraph::new(Line::from(vec![Span::styled(label.to_string(), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))]))
            .render(Rect { x: area.x, y: area.y + *row, width: area.width, height: 1 }, buf);
        *row += 1;
        for (idx, server) in group {
            if *row >= area.height { break; }
            let sel = *idx == selected && focused;
            let prefix = if sel { "> " } else { "  " };
            let style = if sel { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() };
            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(server.status.badge().to_string(), Style::default().fg(server.status.color())),
                Span::styled(format!(" {}", server.name), style),
                Span::styled(format!("  {} tools", server.tool_count), Style::default().fg(Color::DarkGray)),
            ]);
            Paragraph::new(line).render(Rect { x: area.x, y: area.y + *row, width: area.width, height: 1 }, buf);
            if let Some(err) = &server.error_message {
                *row += 1;
                if *row < area.height {
                    let short: String = err.chars().take(area.width as usize - 4).collect();
                    Paragraph::new(Line::from(vec![Span::styled(format!("    {}", short), Style::default().fg(Color::Red))]))
                        .render(Rect { x: area.x, y: area.y + *row, width: area.width, height: 1 }, buf);
                }
            }
            *row += 1;
        }
    };

    render_group(&stdio, "stdio", &mut row, inner, buf, state.selected_server, focused);
    render_group(&sse, "SSE", &mut row, inner, buf, state.selected_server, focused);
    render_group(&http, "HTTP", &mut row, inner, buf, state.selected_server, focused);
}

fn render_tool_list(state: &McpViewState, area: Rect, buf: &mut Buffer) {
    let focused = state.active_pane == McpViewPane::ToolList || state.active_pane == McpViewPane::ToolDetail;
    let border_style = if focused { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) };
    Block::default().title(" Tools ").borders(Borders::ALL).border_style(border_style).render(area, buf);

    let inner = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(2) };

    // Search bar
    let search_line = Line::from(vec![
        Span::styled("/ ", Style::default().fg(Color::DarkGray)),
        Span::raw(state.tool_search.clone()),
    ]);
    Paragraph::new(search_line).render(Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 }, buf);

    let list_area = Rect { x: inner.x, y: inner.y + 1, width: inner.width, height: inner.height.saturating_sub(1) };
    let tools = state.filtered_tools();
    let max_visible = list_area.height as usize;
    let start = state.selected_tool.saturating_sub(max_visible / 2);

    for (i, tool) in tools[start..].iter().enumerate() {
        if i >= max_visible { break; }
        let sel = start + i == state.selected_tool;
        let prefix = if sel { "> " } else { "  " };
        let style = if sel { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() };

        let avail = list_area.width.saturating_sub(20) as usize;
        let name = format!("{}:{}", tool.server, tool.name);
        let name_short: String = name.chars().take(avail).collect();
        let desc: String = tool.description.chars().take(30).collect();

        let line = Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(name_short, style.fg(Color::Cyan)),
            Span::styled(format!("  {}", desc), Style::default().fg(Color::DarkGray)),
        ]);
        Paragraph::new(line).render(
            Rect { x: list_area.x, y: list_area.y + i as u16, width: list_area.width, height: 1 },
            buf,
        );
    }
}

fn render_tool_detail(state: &McpViewState, area: Rect, buf: &mut Buffer) {
    let focused = state.active_pane == McpViewPane::ToolDetail;
    let border_style = if focused { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) };
    Block::default().title(" Tool Detail ").borders(Borders::ALL).border_style(border_style).render(area, buf);

    let inner = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(2) };

    let tools = state.filtered_tools();
    let Some(tool) = tools.get(state.selected_tool) else {
        Paragraph::new("Select a tool to view details.")
            .style(Style::default().fg(Color::DarkGray))
            .render(inner, buf);
        return;
    };

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(format!("{}:{}", tool.server, tool.name), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::default());
    for line in tool.description.lines() {
        lines.push(Line::from(vec![Span::raw(line.to_string())]));
    }
    if let Some(schema) = &tool.input_schema {
        lines.push(Line::default());
        lines.push(Line::from(vec![Span::styled("Input:", Style::default().fg(Color::DarkGray))]));
        for line in schema.lines().take(10) {
            lines.push(Line::from(vec![Span::styled(format!("  {}", line), Style::default().fg(Color::DarkGray))]));
        }
    }

    Paragraph::new(lines)
        .wrap(ratatui::widgets::Wrap { trim: false })
        .render(inner, buf);
}
