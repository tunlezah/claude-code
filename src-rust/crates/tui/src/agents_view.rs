//! Agent / coordinator progress views for the TUI.
//! Mirrors src/components/agents/ (13 files).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// The current status of a sub-agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Running,
    WaitingForTool,
    Complete,
    Failed,
}

impl AgentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::WaitingForTool => "waiting",
            Self::Complete => "done",
            Self::Failed => "failed",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Idle => Color::DarkGray,
            Self::Running => Color::Green,
            Self::WaitingForTool => Color::Yellow,
            Self::Complete => Color::Cyan,
            Self::Failed => Color::Red,
        }
    }
}

/// A sub-agent or coordinator instance.
#[derive(Debug, Clone)]
pub struct AgentInfo {
    /// Unique agent ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Current status.
    pub status: AgentStatus,
    /// Current tool being executed (if any).
    pub current_tool: Option<String>,
    /// Number of turns completed.
    pub turns_completed: u32,
    /// Is this the coordinator?
    pub is_coordinator: bool,
    /// Brief description or last output snippet.
    pub last_output: Option<String>,
}

/// A defined agent (from .claude/agents/*.md or plugin).
#[derive(Debug, Clone)]
pub struct AgentDefinition {
    /// Agent name.
    pub name: String,
    /// Source: "user" | "plugin:{name}" | "builtin".
    pub source: String,
    /// Model name.
    pub model: Option<String>,
    /// Memory scope.
    pub memory_scope: Option<String>,
    /// Description.
    pub description: String,
    /// Tool list (empty = all tools).
    pub tools: Vec<String>,
    /// If another agent overrides this one.
    pub shadowed_by: Option<String>,
}

// ---------------------------------------------------------------------------
// Screen routes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentsRoute {
    List,
    Detail(usize),        // index into definitions
    Editor(Option<usize>), // None = create new
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Full state for the agents menu overlay.
#[derive(Debug, Clone)]
pub struct AgentsMenuState {
    pub open: bool,
    pub route: AgentsRoute,
    pub definitions: Vec<AgentDefinition>,
    pub active_agents: Vec<AgentInfo>,
    pub list_scroll: usize,
    pub selected_row: usize,
}

impl AgentsMenuState {
    pub fn new() -> Self {
        Self {
            open: false,
            route: AgentsRoute::List,
            definitions: Vec::new(),
            active_agents: Vec::new(),
            list_scroll: 0,
            selected_row: 0,
        }
    }

    pub fn open(&mut self, project_root: &std::path::Path) {
        self.definitions = load_agent_definitions(project_root);
        self.selected_row = 0;
        self.list_scroll = 0;
        self.route = AgentsRoute::List;
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn select_prev(&mut self) {
        if self.selected_row > 0 {
            self.selected_row -= 1;
        }
    }

    pub fn select_next(&mut self) {
        let max = self.definitions.len(); // +1 for "create new"
        if self.selected_row < max {
            self.selected_row += 1;
        }
    }

    pub fn confirm_selection(&mut self) {
        if self.selected_row == 0 {
            // [+ Create new agent]
            self.route = AgentsRoute::Editor(None);
        } else {
            let idx = self.selected_row - 1;
            if idx < self.definitions.len() {
                self.route = AgentsRoute::Detail(idx);
            }
        }
    }

    pub fn go_back(&mut self) {
        match &self.route {
            AgentsRoute::Detail(_) | AgentsRoute::Editor(_) => {
                self.route = AgentsRoute::List;
            }
            AgentsRoute::List => {
                self.close();
            }
        }
    }
}

impl Default for AgentsMenuState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

/// Load agent definitions from `.claude/agents/` in project root and home dir.
pub fn load_agent_definitions(project_root: &std::path::Path) -> Vec<AgentDefinition> {
    let mut defs = Vec::new();
    let dirs = [
        dirs::home_dir().map(|h| h.join(".claude").join("agents")),
        Some(project_root.join(".claude").join("agents")),
    ];

    for dir_opt in &dirs {
        let Some(dir) = dir_opt else { continue };
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "md") {
                if let Some(def) = parse_agent_def(&path) {
                    defs.push(def);
                }
            }
        }
    }

    defs
}

fn parse_agent_def(path: &std::path::Path) -> Option<AgentDefinition> {
    let content = std::fs::read_to_string(path).ok()?;
    let stem = path.file_stem()?.to_string_lossy().to_string();

    let (name, model, memory, description, tools) = if content.starts_with("---") {
        let end = content[3..].find("\n---")? + 3;
        let front = &content[3..end];
        let name = extract_yaml_str(front, "name").unwrap_or_else(|| stem.clone());
        let model = extract_yaml_str(front, "model");
        let memory = extract_yaml_str(front, "memory_scope")
            .or_else(|| extract_yaml_str(front, "memory"));
        let desc = extract_yaml_str(front, "description").unwrap_or_default();
        let tools = extract_yaml_list(front, "tools");
        (name, model, memory, desc, tools)
    } else {
        (
            stem,
            None,
            None,
            content.lines().next().unwrap_or("").to_string(),
            vec![],
        )
    };

    Some(AgentDefinition {
        name,
        source: "user".to_string(),
        model,
        memory_scope: memory,
        description,
        tools,
        shadowed_by: None,
    })
}

fn extract_yaml_str(front: &str, key: &str) -> Option<String> {
    for line in front.lines() {
        if let Some(rest) = line.strip_prefix(&format!("{key}:")) {
            return Some(
                rest.trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            );
        }
    }
    None
}

fn extract_yaml_list(front: &str, key: &str) -> Vec<String> {
    for line in front.lines() {
        if let Some(rest) = line.strip_prefix(&format!("{key}:")) {
            let rest = rest.trim().trim_matches('[').trim_matches(']');
            return rest
                .split(',')
                .map(|s| {
                    s.trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string()
                })
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    Vec::new()
}

// ---------------------------------------------------------------------------
// Rendering: Agents Menu overlay
// ---------------------------------------------------------------------------

/// Render the agents menu overlay.
pub fn render_agents_menu(state: &AgentsMenuState, area: Rect, buf: &mut Buffer) {
    if !state.open {
        return;
    }

    // Center dialog: 70% width, 80% height
    let w = (area.width * 7 / 10).max(40).min(area.width);
    let h = (area.height * 4 / 5).max(10).min(area.height);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    let dialog_area = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    Clear.render(dialog_area, buf);

    match &state.route {
        AgentsRoute::List => render_agents_list(state, dialog_area, buf),
        AgentsRoute::Detail(idx) => {
            if let Some(def) = state.definitions.get(*idx) {
                render_agent_detail(def, dialog_area, buf);
            }
        }
        AgentsRoute::Editor(Some(idx)) => {
            render_agent_editor(state.definitions.get(*idx), dialog_area, buf);
        }
        AgentsRoute::Editor(None) => {
            render_agent_editor(None, dialog_area, buf);
        }
    }
}

fn render_agents_list(state: &AgentsMenuState, area: Rect, buf: &mut Buffer) {
    Block::default()
        .title(" Agents [↑↓ navigate, Enter: select, Esc: close] ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan))
        .render(area, buf);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // First row: [+ Create new agent]
    let create_selected = state.selected_row == 0;
    let create_style = if create_selected {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };
    let prefix = if create_selected { "> " } else { "  " };
    let create_line = Line::from(vec![
        Span::styled(prefix, create_style),
        Span::styled("[+ Create new agent]", create_style),
    ]);
    Paragraph::new(create_line).render(
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
        buf,
    );

    let max_visible = (inner.height as usize).saturating_sub(2);
    let start = state
        .list_scroll
        .min(state.definitions.len().saturating_sub(max_visible));

    for (i, def) in state.definitions[start..].iter().enumerate() {
        if i >= max_visible {
            break;
        }
        let abs_idx = start + i;
        let selected = state.selected_row == abs_idx + 1;
        let y = inner.y + 2 + i as u16;

        let prefix = if selected { "> " } else { "  " };
        let base = if selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let model_str = def.model.as_deref().unwrap_or("default");
        let shadow_suffix = if def.shadowed_by.is_some() { " ⚠" } else { "" };

        let line = Line::from(vec![
            Span::styled(prefix, base),
            Span::styled(def.name.clone(), base.fg(Color::White)),
            Span::styled(
                format!("  {} | {}{}", model_str, def.source, shadow_suffix),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        let row_area = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        Paragraph::new(line).render(row_area, buf);
    }
}

fn render_agent_detail(def: &AgentDefinition, area: Rect, buf: &mut Buffer) {
    let title = format!(" Agent: {} ", def.name);
    Block::default()
        .title(title.as_str())
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan))
        .render(area, buf);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Name:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            def.name.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  ({})", def.source),
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Model:  ", Style::default().fg(Color::DarkGray)),
        Span::raw(def.model.as_deref().unwrap_or("default").to_string()),
    ]));
    if let Some(mem) = &def.memory_scope {
        lines.push(Line::from(vec![
            Span::styled("Memory: ", Style::default().fg(Color::DarkGray)),
            Span::raw(mem.clone()),
        ]));
    }
    if !def.tools.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Tools:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(def.tools.join(", ")),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Tools:  ", Style::default().fg(Color::DarkGray)),
            Span::styled("All tools", Style::default().fg(Color::DarkGray)),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(vec![Span::styled(
        "Description:",
        Style::default().fg(Color::DarkGray),
    )]));
    for line in def.description.lines() {
        lines.push(Line::from(vec![Span::raw(format!("  {}", line))]));
    }

    if let Some(shadow) = &def.shadowed_by {
        lines.push(Line::default());
        lines.push(Line::from(vec![Span::styled(
            format!("⚠ Shadowed by: {}", shadow),
            Style::default().fg(Color::Yellow),
        )]));
    }

    lines.push(Line::default());
    lines.push(Line::from(vec![Span::styled(
        "[Esc] back",
        Style::default().fg(Color::DarkGray),
    )]));

    Paragraph::new(lines)
        .wrap(ratatui::widgets::Wrap { trim: false })
        .render(inner, buf);
}

fn render_agent_editor(def: Option<&AgentDefinition>, area: Rect, buf: &mut Buffer) {
    let title = if def.is_some() {
        " Edit Agent "
    } else {
        " Create Agent "
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Yellow))
        .render(area, buf);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let name = def.map(|d| d.name.as_str()).unwrap_or("my-agent");
    let model = def
        .and_then(|d| d.model.as_deref())
        .unwrap_or("claude-sonnet-4-6");
    let desc = def.map(|d| d.description.as_str()).unwrap_or("");

    let lines = vec![
        Line::from(vec![
            Span::styled("Name:   ", Style::default().fg(Color::DarkGray)),
            Span::raw(name.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Model:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(model.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Desc:   ", Style::default().fg(Color::DarkGray)),
            Span::raw(desc.to_string()),
        ]),
        Line::default(),
        Line::from(vec![Span::styled(
            "Edit the agent file directly in .claude/agents/<name>.md",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]),
        Line::default(),
        Line::from(vec![Span::styled(
            "[Esc] back",
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    Paragraph::new(lines).render(inner, buf);
}

// ---------------------------------------------------------------------------
// Rendering: Coordinator status inline widget
// ---------------------------------------------------------------------------

/// Render an inline coordinator + sub-agent status widget.
///
/// Shows: coordinator status, then each sub-agent with its current tool.
/// Suitable for embedding in the main TUI layout (e.g., below the message list).
pub fn render_coordinator_status(agents: &[AgentInfo], area: Rect, buf: &mut Buffer) {
    if agents.is_empty() {
        return;
    }

    Block::default()
        .title(" Active Agents ")
        .borders(Borders::TOP)
        .style(Style::default().fg(Color::DarkGray))
        .render(area, buf);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(1),
    };

    for (i, agent) in agents.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let y = inner.y + i as u16;
        let row_area = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };

        let prefix = if agent.is_coordinator { "● " } else { "  ○ " };
        let tool_str = agent
            .current_tool
            .as_deref()
            .map(|t| format!(" → {}", t))
            .unwrap_or_default();

        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(agent.status.color())),
            Span::styled(agent.name.clone(), Style::default().fg(Color::White)),
            Span::styled(
                format!(" [{}]", agent.status.label()),
                Style::default().fg(agent.status.color()),
            ),
            Span::styled(
                format!(" {} turns", agent.turns_completed),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(tool_str, Style::default().fg(Color::Yellow)),
        ]);

        Paragraph::new(line).render(row_area, buf);
    }
}
