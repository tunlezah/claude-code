// cc-tui: Terminal UI using ratatui + crossterm for the Claude Code Rust port.
//
// This crate provides the interactive terminal interface including:
// - Message display with syntax highlighting
// - Input prompt with history
// - Streaming response rendering
// - Tool execution progress display
// - Permission dialogs
// - Cost/token tracking display
// - Notification banners
// - Help, history-search, message-selector, and rewind overlays
// - Bridge connection status badge
// - Plugin hint banners

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------

/// Figure/icon constants matching src/constants/figures.ts
pub mod figures;
/// Clawd robot mascot rendering.
pub mod clawd;
/// Application state and main event loop.
pub mod app;
/// Input helpers: slash command parsing.
pub mod input;
/// All ratatui rendering logic.
pub mod render;
/// Permission dialogs and confirmation dialogs.
pub mod dialogs;
/// Notification / banner system.
pub mod notifications;
/// Help overlay, history search, message selector, rewind flow.
pub mod overlays;
/// Bridge connection state and status badge.
pub mod bridge_state;
/// Plugin hint/recommendation UI.
pub mod plugin_views;
/// Full-screen tabbed settings interface.
pub mod settings_screen;
/// Theme picker overlay.
pub mod theme_screen;
/// Privacy settings dialog.
pub mod privacy_screen;
/// Diff viewer dialog (two-pane: file list + unified diff detail).
pub mod diff_viewer;
/// Virtual scrollable list for efficient message rendering.
pub mod virtual_list;
/// Message type renderers (assistant, user, tool use, etc.).
pub mod messages;
/// Agent definitions list and coordinator progress view.
pub mod agents_view;
/// Stats dialog with token usage and cost charts.
pub mod stats_dialog;
/// MCP server management UI.
pub mod mcp_view;
/// Complete prompt input with vim mode, history, typeahead, and paste handling.
pub mod prompt_input;

// ---------------------------------------------------------------------------
// Public re-exports
// ---------------------------------------------------------------------------

pub use app::App;
pub use input::{is_slash_command, parse_slash_command};
pub use diff_viewer::{DiffViewerState, DiffPane, DiffType, load_git_diff, parse_unified_diff, render_diff_dialog};
pub use agents_view::{AgentInfo, AgentStatus, AgentsMenuState, AgentDefinition, render_agents_menu, render_coordinator_status, load_agent_definitions};
pub use stats_dialog::{StatsDialogState, StatsTab, load_stats, render_stats_dialog};
pub use mcp_view::{McpViewState, McpServerView, McpToolView, McpViewStatus, render_mcp_view};
pub use prompt_input::{PromptInputState, VimMode, InputMode, render_prompt_input, handle_paste, compute_typeahead};

// ---------------------------------------------------------------------------
// Terminal initialization / teardown helpers (public API)
// ---------------------------------------------------------------------------

/// Set up the terminal for TUI mode (raw mode + alternate screen).
pub fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to its original state.
pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use app::{App, HistorySearch, ToolStatus, ToolUseBlock};
    use dialogs::PermissionRequest;
    use cc_core::config::Config;
    use cc_core::cost::CostTracker;
    use cc_core::types::{ContentBlock, Role, ToolResultContent};

    fn make_app() -> App {
        App::new(Config::default(), CostTracker::new())
    }

    // ---- input helpers ---------------------------------------------------

    #[test]
    fn test_is_slash_command() {
        assert!(input::is_slash_command("/help"));
        assert!(input::is_slash_command("/compact args"));
        assert!(!input::is_slash_command("//comment"));
        assert!(!input::is_slash_command("hello"));
        assert!(!input::is_slash_command(""));
    }

    #[test]
    fn test_parse_slash_command_no_args() {
        let (cmd, args) = input::parse_slash_command("/help");
        assert_eq!(cmd, "help");
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_slash_command_with_args() {
        let (cmd, args) = input::parse_slash_command("/compact  --force ");
        assert_eq!(cmd, "compact");
        assert_eq!(args, "--force");
    }

    #[test]
    fn test_parse_slash_command_non_slash() {
        let (cmd, args) = input::parse_slash_command("hello world");
        assert_eq!(cmd, "");
        assert_eq!(args, "");
    }

    // ---- App::take_input ------------------------------------------------

    #[test]
    fn test_take_input_pushes_history() {
        let mut app = make_app();
        app.input = "hello".to_string();
        let result = app.take_input();
        assert_eq!(result, "hello");
        assert_eq!(app.input, "");
        assert_eq!(app.prompt_input.text, "");
        assert_eq!(app.input_history, vec!["hello"]);
        assert_eq!(app.prompt_input.history, vec!["hello"]);
        assert_eq!(app.cursor_pos, 0);
    }

    #[test]
    fn test_take_input_empty_does_not_push_history() {
        let mut app = make_app();
        let result = app.take_input();
        assert_eq!(result, "");
        assert!(app.input_history.is_empty());
    }

    // ---- add_message / set_model ----------------------------------------

    #[test]
    fn test_add_message() {
        let mut app = make_app();
        app.add_message(Role::User, "hi".to_string());
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].role, Role::User);
    }

    #[test]
    fn test_set_model() {
        let mut app = make_app();
        app.set_model("claude-opus-4-5".to_string());
        assert_eq!(app.model_name, "claude-opus-4-5");
    }

    #[test]
    fn test_stats_slash_command_opens_dialog_and_closes_other_views() {
        let mut app = make_app();
        app.mcp_view.open(vec![]);
        app.agents_menu.open = true;

        assert!(app.intercept_slash_command("stats"));
        assert!(app.stats_dialog.open);
        assert!(!app.mcp_view.open);
        assert!(!app.agents_menu.open);
        assert!(!app.diff_viewer.open);
    }

    #[test]
    fn test_agents_slash_command_populates_active_agents() {
        let mut app = make_app();
        app.agent_status = vec![
            ("Mendel".to_string(), "running".to_string()),
            ("Aristotle".to_string(), "waiting".to_string()),
            ("Plato".to_string(), "done".to_string()),
        ];

        assert!(app.intercept_slash_command("agents"));
        assert!(app.agents_menu.open);
        assert_eq!(app.agents_menu.active_agents.len(), 3);
        assert_eq!(app.agents_menu.active_agents[0].status, AgentStatus::Running);
        assert_eq!(
            app.agents_menu.active_agents[1].status,
            AgentStatus::WaitingForTool
        );
        assert_eq!(app.agents_menu.active_agents[2].status, AgentStatus::Complete);
    }

    // ---- key handling ----------------------------------------------------

    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_ctrl_c_quits_when_idle() {
        let mut app = make_app();
        app.handle_key_event(ctrl(KeyCode::Char('c')));
        assert!(app.should_quit);
    }

    #[test]
    fn test_ctrl_c_cancels_streaming() {
        let mut app = make_app();
        app.is_streaming = true;
        app.streaming_text = "partial".to_string();
        app.handle_key_event(ctrl(KeyCode::Char('c')));
        assert!(!app.is_streaming);
        assert!(!app.should_quit);
        assert!(app.streaming_text.is_empty());
    }

    #[test]
    fn test_ctrl_d_quits_on_empty_input() {
        let mut app = make_app();
        app.handle_key_event(ctrl(KeyCode::Char('d')));
        assert!(app.should_quit);
    }

    #[test]
    fn test_ctrl_d_does_not_quit_with_input() {
        let mut app = make_app();
        app.input = "abc".to_string();
        app.handle_key_event(ctrl(KeyCode::Char('d')));
        assert!(!app.should_quit);
    }

    #[test]
    fn test_enter_returns_true() {
        let mut app = make_app();
        let submit = app.handle_key_event(key(KeyCode::Enter));
        assert!(submit);
    }

    #[test]
    fn test_enter_blocked_while_streaming() {
        let mut app = make_app();
        app.is_streaming = true;
        let submit = app.handle_key_event(key(KeyCode::Enter));
        assert!(!submit);
    }

    #[test]
    fn test_char_input_appends() {
        let mut app = make_app();
        app.handle_key_event(key(KeyCode::Char('h')));
        app.handle_key_event(key(KeyCode::Char('i')));
        assert_eq!(app.input, "hi");
        assert_eq!(app.prompt_input.text, "hi");
    }

    #[test]
    fn test_backspace_removes_char() {
        let mut app = make_app();
        app.input = "hello".to_string();
        app.cursor_pos = 5;
        app.handle_key_event(key(KeyCode::Backspace));
        assert_eq!(app.input, "hell");
        assert_eq!(app.prompt_input.text, "hell");
    }

    #[test]
    fn test_history_navigation() {
        let mut app = make_app();
        app.input_history = vec!["first".to_string(), "second".to_string()];
        app.handle_key_event(key(KeyCode::Up));
        assert_eq!(app.input, "second");
        app.handle_key_event(key(KeyCode::Up));
        assert_eq!(app.input, "first");
        app.handle_key_event(key(KeyCode::Down));
        assert_eq!(app.input, "second");
        app.handle_key_event(key(KeyCode::Down));
        assert_eq!(app.input, "");
        assert!(app.history_index.is_none());
    }

    #[test]
    fn test_history_navigation_restores_draft() {
        let mut app = make_app();
        app.input_history = vec!["first".to_string(), "second".to_string()];
        app.input = "draft".to_string();
        app.cursor_pos = app.input.len();

        app.handle_key_event(key(KeyCode::Up));
        assert_eq!(app.input, "second");

        app.handle_key_event(key(KeyCode::Down));
        assert_eq!(app.input, "draft");
        assert_eq!(app.prompt_input.text, "draft");
        assert!(app.history_index.is_none());
    }

    #[test]
    fn test_tab_accepts_slash_suggestion() {
        let mut app = make_app();
        app.handle_key_event(key(KeyCode::Char('/')));
        app.handle_key_event(key(KeyCode::Char('a')));
        app.handle_key_event(key(KeyCode::Tab));

        assert_eq!(app.input, "/agents");
        assert_eq!(app.prompt_input.text, "/agents");
        assert_eq!(app.cursor_pos, "/agents".len());
    }

    #[test]
    fn test_ctrl_p_opens_global_search() {
        let mut app = make_app();
        app.handle_key_event(ctrl(KeyCode::Char('p')));
        assert!(app.global_search.open);
    }

    #[test]
    fn test_global_search_enter_inserts_selected_ref() {
        let mut app = make_app();
        app.global_search.open();
        app.global_search.results = vec![overlays::SearchResult {
            file: "src/main.rs".to_string(),
            line: 42,
            col: 1,
            text: "fn main() {}".to_string(),
            context_before: Vec::new(),
            context_after: Vec::new(),
        }];
        app.handle_key_event(key(KeyCode::Enter));

        assert!(!app.global_search.open);
        assert_eq!(app.input, "src/main.rs:42");
        assert_eq!(app.prompt_input.text, "src/main.rs:42");
    }

    #[test]
    fn test_page_scroll() {
        let mut app = make_app();
        app.handle_key_event(key(KeyCode::PageUp));
        assert_eq!(app.scroll_offset, 10);
        app.handle_key_event(key(KeyCode::PageDown));
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_f1_toggles_help() {
        let mut app = make_app();
        assert!(!app.show_help);
        app.handle_key_event(key(KeyCode::F(1)));
        assert!(app.show_help);
        app.handle_key_event(key(KeyCode::F(1)));
        assert!(!app.show_help);
    }

    #[test]
    fn test_stats_dialog_keys_switch_tab_and_close() {
        let mut app = make_app();
        app.stats_dialog.open = true;

        app.handle_key_event(key(KeyCode::Right));
        assert_eq!(app.stats_dialog.tab, StatsTab::DailyTokens);

        app.handle_key_event(key(KeyCode::Esc));
        assert!(!app.stats_dialog.open);
    }

    #[test]
    fn test_mcp_view_keys_search_and_close() {
        let mut app = make_app();
        app.mcp_view.open(vec![McpServerView {
            name: "filesystem".to_string(),
            transport: "stdio".to_string(),
            status: McpViewStatus::Connected,
            tool_count: 1,
            resource_count: 0,
            prompt_count: 0,
            error_message: None,
            tools: vec![McpToolView {
                name: "read_file".to_string(),
                server: "filesystem".to_string(),
                description: "Read a file".to_string(),
                input_schema: None,
            }],
        }]);
        app.mcp_view.switch_pane();

        app.handle_key_event(key(KeyCode::Char('r')));
        assert_eq!(app.mcp_view.tool_search, "");
        assert_eq!(
            app.status_message.as_deref(),
            Some("Reconnect is not wired yet in the Rust TUI.")
        );

        app.handle_key_event(key(KeyCode::Char('f')));
        assert_eq!(app.mcp_view.tool_search, "f");

        app.handle_key_event(key(KeyCode::Backspace));
        assert_eq!(app.mcp_view.tool_search, "");

        app.handle_key_event(key(KeyCode::Esc));
        assert!(!app.mcp_view.open);
    }

    #[test]
    fn test_message_renderer_includes_tool_use_and_thinking_blocks() {
        let msg = cc_core::types::Message::assistant_blocks(vec![
            ContentBlock::Thinking {
                thinking: "reasoning".to_string(),
                signature: "sig".to_string(),
            },
            ContentBlock::ToolUse {
                id: "toolu_1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({ "path": "README.md" }),
            },
            ContentBlock::Text {
                text: "Done".to_string(),
            },
        ]);

        let rendered = messages::render_message(&msg, &messages::RenderContext::default());
        let text = rendered
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.clone()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("Thinking"));
        assert!(text.contains("read_file"));
        assert!(text.contains("Done"));
    }

    #[test]
    fn test_message_renderer_includes_tool_result_errors() {
        let msg = cc_core::types::Message::user_blocks(vec![ContentBlock::ToolResult {
            tool_use_id: "toolu_1".to_string(),
            content: ToolResultContent::Text("boom".to_string()),
            is_error: Some(true),
        }]);

        let rendered = messages::render_message(&msg, &messages::RenderContext::default());
        let text = rendered
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.clone()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("Error"));
        assert!(text.contains("boom"));
    }

    // ---- QueryEvent handling --------------------------------------------

    #[test]
    fn test_handle_status_event() {
        let mut app = make_app();
        app.handle_query_event(cc_query::QueryEvent::Status("working".to_string()));
        assert_eq!(app.status_message.as_deref(), Some("working"));
    }

    #[test]
    fn test_handle_error_event() {
        let mut app = make_app();
        app.is_streaming = true;
        app.handle_query_event(cc_query::QueryEvent::Error("oops".to_string()));
        assert!(!app.is_streaming);
        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0].get_all_text().contains("oops"));
    }

    #[test]
    fn test_handle_tool_start_and_end() {
        let mut app = make_app();
        app.handle_query_event(cc_query::QueryEvent::ToolStart {
            tool_name: "Bash".to_string(),
            tool_id: "t1".to_string(),
        });
        assert_eq!(app.tool_use_blocks.len(), 1);
        assert_eq!(app.tool_use_blocks[0].status, ToolStatus::Running);

        app.handle_query_event(cc_query::QueryEvent::ToolEnd {
            tool_name: "Bash".to_string(),
            tool_id: "t1".to_string(),
            result: "output".to_string(),
            is_error: false,
        });
        assert_eq!(app.tool_use_blocks[0].status, ToolStatus::Done);
    }

    #[test]
    fn test_handle_tool_end_error() {
        let mut app = make_app();
        app.tool_use_blocks.push(ToolUseBlock {
            id: "t2".to_string(),
            name: "Read".to_string(),
            status: ToolStatus::Running,
            output_preview: None,
        });
        app.handle_query_event(cc_query::QueryEvent::ToolEnd {
            tool_name: "Read".to_string(),
            tool_id: "t2".to_string(),
            result: "file not found".to_string(),
            is_error: true,
        });
        assert_eq!(app.tool_use_blocks[0].status, ToolStatus::Error);
        assert!(app.status_message.is_some());
    }

    #[test]
    fn test_turn_complete_flushes_streaming_text() {
        let mut app = make_app();
        app.is_streaming = true;
        app.streaming_text = "partial response".to_string();
        app.handle_query_event(cc_query::QueryEvent::TurnComplete {
            turn: 1,
            stop_reason: "end_turn".to_string(),
            usage: None,
        });
        assert!(!app.is_streaming);
        assert!(app.streaming_text.is_empty());
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].get_all_text(), "partial response");
    }

    // ---- HistorySearch --------------------------------------------------

    #[test]
    fn test_history_search_matches() {
        let history = vec![
            "git commit".to_string(),
            "git push".to_string(),
            "cargo build".to_string(),
        ];
        let mut hs = HistorySearch::new();
        hs.query = "git".to_string();
        hs.update_matches(&history);
        assert_eq!(hs.matches.len(), 2);
        assert_eq!(hs.matches[0], 0);
        assert_eq!(hs.matches[1], 1);
    }

    #[test]
    fn test_history_search_no_matches() {
        let history = vec!["hello".to_string()];
        let mut hs = HistorySearch::new();
        hs.query = "xyz".to_string();
        hs.update_matches(&history);
        assert!(hs.matches.is_empty());
    }

    // ---- PermissionRequest --------------------------------------------

    #[test]
    fn test_permission_request_standard() {
        let pr = PermissionRequest::standard(
            "tu1".to_string(),
            "Bash".to_string(),
            "Run a shell command".to_string(),
        );
        assert_eq!(pr.options.len(), 4);
        assert_eq!(pr.options[0].key, 'y');
        assert_eq!(pr.options[1].key, 'Y');
        assert_eq!(pr.options[2].key, 'p');
        assert_eq!(pr.options[3].key, 'n');
    }
}
