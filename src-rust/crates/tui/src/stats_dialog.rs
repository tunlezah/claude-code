//! Stats dialog — mirrors src/components/Stats.tsx
//!
//! Three-tab overlay: Overview | Daily Tokens | Cost Heatmap
//! Data source: ~/.claude/stats.jsonl (append-only per-turn usage log)

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A single entry in ~/.claude/stats.jsonl
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatsEntry {
    pub timestamp_ms: u64,
    pub session_id: Option<String>,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    /// Cost in USD cents (f64)
    pub cost_cents: f64,
    pub project: Option<String>,
}

/// Aggregated stats for display.
#[derive(Debug, Clone, Default)]
pub struct AggregatedStats {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_cents: f64,
    pub by_model: HashMap<String, ModelStats>,
    /// (date_str "YYYY-MM-DD", tokens) pairs sorted by date
    pub daily_tokens: Vec<(String, u64)>,
    /// (date_str "YYYY-MM-DD", cost_cents) for heatmap
    pub daily_costs: HashMap<String, f64>,
    pub peak_day: Option<String>,
    pub peak_day_tokens: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ModelStats {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_cents: f64,
    pub turns: u64,
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

/// Load and aggregate stats from ~/.claude/stats.jsonl
pub fn load_stats() -> AggregatedStats {
    let path = dirs::home_dir()
        .map(|h| h.join(".claude").join("stats.jsonl"))
        .unwrap_or_default();

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return AggregatedStats::default(),
    };

    let mut agg = AggregatedStats::default();
    let mut daily: HashMap<String, u64> = HashMap::new();

    for line in content.lines() {
        let Ok(entry) = serde_json::from_str::<StatsEntry>(line) else { continue };

        let total_tokens = entry.input_tokens + entry.output_tokens;
        agg.total_input_tokens += entry.input_tokens;
        agg.total_output_tokens += entry.output_tokens;
        agg.total_cost_cents += entry.cost_cents;

        let model_entry = agg.by_model.entry(entry.model.clone()).or_default();
        model_entry.input_tokens += entry.input_tokens;
        model_entry.output_tokens += entry.output_tokens;
        model_entry.cost_cents += entry.cost_cents;
        model_entry.turns += 1;

        // Date from timestamp
        let date = timestamp_to_date(entry.timestamp_ms);
        *daily.entry(date.clone()).or_insert(0) += total_tokens;
        *agg.daily_costs.entry(date).or_insert(0.0) += entry.cost_cents;
    }

    // Build sorted daily_tokens
    let mut daily_sorted: Vec<(String, u64)> = daily.into_iter().collect();
    daily_sorted.sort_by(|a, b| a.0.cmp(&b.0));
    agg.peak_day = daily_sorted.iter().max_by_key(|d| d.1).map(|d| d.0.clone());
    agg.peak_day_tokens = daily_sorted.iter().map(|d| d.1).max().unwrap_or(0);
    agg.daily_tokens = daily_sorted;

    agg
}

fn timestamp_to_date(ts_ms: u64) -> String {
    // Simple ISO date from Unix timestamp in ms
    let secs = ts_ms / 1000;
    let days_since_epoch = secs / 86400;
    // Rough Gregorian calendar calculation
    let year = 1970 + (days_since_epoch * 4 + 2) / 1461;
    let day_of_year = days_since_epoch - (year - 1970) * 365 - (year - 1970 - 1) / 4;
    let (month, day) = day_of_year_to_month_day(day_of_year as u32, is_leap_year(year as u32));
    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn is_leap_year(year: u32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn day_of_year_to_month_day(doy: u32, leap: bool) -> (u32, u32) {
    let months = if leap {
        [31u32, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u32, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut remaining = doy;
    for (i, &m) in months.iter().enumerate() {
        if remaining < m {
            return (i as u32 + 1, remaining + 1);
        }
        remaining -= m;
    }
    (12, 31)
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsTab {
    Overview,
    DailyTokens,
    CostHeatmap,
}

#[derive(Debug, Clone)]
pub struct StatsDialogState {
    pub open: bool,
    pub tab: StatsTab,
    pub range_days: u32,  // 7, 30, or 0 = all
    pub data: Option<AggregatedStats>,
    pub scroll: u16,
}

impl StatsDialogState {
    pub fn new() -> Self {
        Self {
            open: false,
            tab: StatsTab::Overview,
            range_days: 30,
            data: None,
            scroll: 0,
        }
    }

    pub fn open(&mut self) {
        self.data = Some(load_stats());
        self.open = true;
        self.scroll = 0;
    }

    pub fn close(&mut self) { self.open = false; }

    pub fn next_tab(&mut self) {
        self.tab = match self.tab {
            StatsTab::Overview => StatsTab::DailyTokens,
            StatsTab::DailyTokens => StatsTab::CostHeatmap,
            StatsTab::CostHeatmap => StatsTab::Overview,
        };
        self.scroll = 0;
    }

    pub fn prev_tab(&mut self) {
        self.tab = match self.tab {
            StatsTab::Overview => StatsTab::CostHeatmap,
            StatsTab::DailyTokens => StatsTab::Overview,
            StatsTab::CostHeatmap => StatsTab::DailyTokens,
        };
        self.scroll = 0;
    }

    pub fn cycle_range(&mut self) {
        self.range_days = match self.range_days {
            7 => 30,
            30 => 0,
            _ => 7,
        };
    }
}

impl Default for StatsDialogState {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render the stats dialog overlay.
pub fn render_stats_dialog(state: &StatsDialogState, area: Rect, buf: &mut Buffer) {
    if !state.open { return; }

    let w = (area.width * 4 / 5).max(40).min(area.width);
    let h = (area.height * 4 / 5).max(15).min(area.height);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    let dialog = Rect { x, y, width: w, height: h };

    Clear.render(dialog, buf);
    Block::default()
        .title(" Stats [Tab: switch tabs, Esc: close] ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan))
        .render(dialog, buf);

    let inner = Rect {
        x: dialog.x + 1,
        y: dialog.y + 1,
        width: dialog.width.saturating_sub(2),
        height: dialog.height.saturating_sub(2),
    };

    // Tab bar
    let tab_line = Line::from(vec![
        tab_span("Overview", state.tab == StatsTab::Overview),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        tab_span("Daily Tokens", state.tab == StatsTab::DailyTokens),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        tab_span("Cost Heatmap", state.tab == StatsTab::CostHeatmap),
    ]);
    Paragraph::new(tab_line).render(
        Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 },
        buf,
    );

    let content_area = Rect {
        x: inner.x,
        y: inner.y + 2,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };

    let Some(data) = &state.data else {
        Paragraph::new("Loading\u{2026}")
            .style(Style::default().fg(Color::DarkGray))
            .render(content_area, buf);
        return;
    };

    match state.tab {
        StatsTab::Overview => render_overview(data, content_area, buf),
        StatsTab::DailyTokens => render_daily_tokens(data, state.range_days, content_area, buf),
        StatsTab::CostHeatmap => render_cost_heatmap(data, content_area, buf),
    }
}

fn tab_span(label: &str, active: bool) -> Span<'static> {
    if active {
        Span::styled(
            label.to_string(),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
    } else {
        Span::styled(label.to_string(), Style::default().fg(Color::DarkGray))
    }
}

fn render_overview(data: &AggregatedStats, area: Rect, buf: &mut Buffer) {
    let total_tokens = data.total_input_tokens + data.total_output_tokens;
    let mut lines = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("Total tokens: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format_tokens(total_tokens), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Input:    ", Style::default().fg(Color::DarkGray)),
        Span::raw(format_tokens(data.total_input_tokens)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Output:   ", Style::default().fg(Color::DarkGray)),
        Span::raw(format_tokens(data.total_output_tokens)),
    ]));
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("Total cost: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("${:.2}", data.total_cost_cents / 100.0), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]));

    if let Some(peak) = &data.peak_day {
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("Peak day: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} ({} tokens)", peak, format_tokens(data.peak_day_tokens)), Style::default().fg(Color::Yellow)),
        ]));
    }

    if !data.by_model.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from(vec![Span::styled("By model:", Style::default().fg(Color::DarkGray))]));
        let mut models: Vec<_> = data.by_model.iter().collect();
        models.sort_by(|a, b| b.1.cost_cents.partial_cmp(&a.1.cost_cents).unwrap_or(std::cmp::Ordering::Equal));
        for (model, stats) in models.iter().take(5) {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:40} ", model), Style::default().fg(Color::Cyan)),
                Span::styled(format!("{} turns  {}", stats.turns, format_tokens(stats.input_tokens + stats.output_tokens)), Style::default().fg(Color::White)),
                Span::styled(format!("  ${:.2}", stats.cost_cents / 100.0), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    Paragraph::new(lines).render(area, buf);
}

fn render_daily_tokens(data: &AggregatedStats, range_days: u32, area: Rect, buf: &mut Buffer) {
    // Filter to range
    let filtered: Vec<_> = if range_days == 0 {
        data.daily_tokens.iter().collect()
    } else {
        data.daily_tokens.iter().rev().take(range_days as usize).collect::<Vec<_>>().into_iter().rev().collect()
    };

    if filtered.is_empty() {
        Paragraph::new("No data yet.").style(Style::default().fg(Color::DarkGray)).render(area, buf);
        return;
    }

    let range_label = match range_days {
        7 => "7 days",
        30 => "30 days",
        _ => "all time",
    };
    let label_line = Line::from(vec![
        Span::styled(format!("Range: {} [r: cycle]", range_label), Style::default().fg(Color::DarkGray)),
    ]);
    Paragraph::new(label_line).render(
        Rect { x: area.x, y: area.y, width: area.width, height: 1 },
        buf,
    );

    let chart_area = Rect { x: area.x, y: area.y + 2, width: area.width, height: area.height.saturating_sub(2) };

    // Build bar chart data
    let max_val = filtered.iter().map(|d| d.1).max().unwrap_or(1).max(1);
    let bar_data: Vec<(&str, u64)> = filtered
        .iter()
        .map(|d| {
            let label: &str = if d.0.len() >= 5 { &d.0[5..] } else { d.0.as_str() };
            (label, d.1 * (chart_area.height as u64 - 1) / max_val)
        })
        .collect();

    // Render ASCII bar chart manually (ratatui BarChart needs 'static strs)
    for (i, (label, height)) in bar_data.iter().enumerate() {
        let x = chart_area.x + i as u16 * 6;
        if x + 5 >= chart_area.x + chart_area.width { break; }
        let bar_height = (*height as u16).min(chart_area.height.saturating_sub(1));
        for row in 0..bar_height {
            let y = chart_area.y + chart_area.height - 1 - row;
            let cell = buf.cell_mut((x + 1, y));
            if let Some(c) = cell {
                c.set_symbol("\u{2588}");
                c.set_style(Style::default().fg(Color::Cyan));
            }
            let cell2 = buf.cell_mut((x + 2, y));
            if let Some(c) = cell2 {
                c.set_symbol("\u{2588}");
                c.set_style(Style::default().fg(Color::Cyan));
            }
        }
        // Label
        let y = chart_area.y + chart_area.height - 1;
        let label_short: String = label.chars().take(4).collect();
        for (j, ch) in label_short.chars().enumerate() {
            let cell = buf.cell_mut((x + j as u16, y));
            if let Some(c) = cell {
                c.set_symbol(&ch.to_string());
                c.set_style(Style::default().fg(Color::DarkGray));
            }
        }
    }
}

fn render_cost_heatmap(data: &AggregatedStats, area: Rect, buf: &mut Buffer) {
    if data.daily_costs.is_empty() {
        Paragraph::new("No cost data yet.").style(Style::default().fg(Color::DarkGray)).render(area, buf);
        return;
    }

    let max_cost = data.daily_costs.values().cloned().fold(0.0_f64, f64::max).max(0.01);

    Paragraph::new(Line::from(vec![
        Span::styled("Cost Heatmap (last 12 weeks)  \u{25a1} = low  \u{25a0} = high", Style::default().fg(Color::DarkGray)),
    ])).render(Rect { x: area.x, y: area.y, width: area.width, height: 1 }, buf);

    let heatmap_area = Rect { x: area.x, y: area.y + 2, width: area.width, height: area.height.saturating_sub(3) };

    // Day-of-week labels
    let dow = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
    for (i, d) in dow.iter().enumerate() {
        let y = heatmap_area.y + i as u16;
        if y >= heatmap_area.y + heatmap_area.height { break; }
        let cell_line = Line::from(vec![Span::styled(d.to_string(), Style::default().fg(Color::DarkGray))]);
        Paragraph::new(cell_line).render(Rect { x: heatmap_area.x, y, width: 2, height: 1 }, buf);
    }

    // 12 weeks x 7 days grid
    let sorted_dates: Vec<_> = {
        let mut v: Vec<_> = data.daily_costs.iter().collect();
        v.sort_by(|a, b| a.0.cmp(b.0));
        v
    };

    for (week_idx, chunk) in sorted_dates.chunks(7).rev().take(12).enumerate() {
        let x = heatmap_area.x + 3 + (11 - week_idx) as u16 * 2;
        for (day_idx, (_, cost)) in chunk.iter().enumerate() {
            let y = heatmap_area.y + day_idx as u16;
            if y >= heatmap_area.y + heatmap_area.height { break; }
            let intensity = (*cost / max_cost).min(1.0);
            let (symbol, color) = if intensity < 0.01 {
                ("\u{25a1}", Color::DarkGray)
            } else if intensity < 0.25 {
                ("\u{25aa}", Color::Yellow)
            } else if intensity < 0.60 {
                ("\u{25a0}", Color::LightYellow)
            } else {
                ("\u{25a0}", Color::Red)
            };
            let cell = buf.cell_mut((x, y));
            if let Some(c) = cell {
                c.set_symbol(symbol);
                c.set_style(Style::default().fg(color));
            }
        }
    }
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1_000_000.0) }
    else if n >= 10_000 { format!("{:.0}K", n as f64 / 1_000.0) }
    else if n >= 1_000 { format!("{:.1}K", n as f64 / 1_000.0) }
    else { n.to_string() }
}
