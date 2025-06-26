use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use helios_common::network_spec::NetworkSpec;

use super::app::{App, BlockType, SyncStatus};

pub fn draw<N: NetworkSpec>(f: &mut Frame, app: &App<N>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),  // Header
                Constraint::Length(10), // Main stats
                Constraint::Min(10),    // Block history
                Constraint::Length(1),  // Footer
            ]
            .as_ref(),
        )
        .split(f.size());

    draw_header(f, app, chunks[0]);
    draw_main_stats(f, app, chunks[1]);
    draw_block_history(f, app, chunks[2]);
    draw_footer(f, chunks[3]);
}

fn draw_header<N: NetworkSpec>(f: &mut Frame, app: &App<N>, area: Rect) {
    let uptime = app.get_uptime();
    let hours = uptime.as_secs() / 3600;
    let minutes = (uptime.as_secs() % 3600) / 60;
    let seconds = uptime.as_secs() % 60;

    let (status_text, status_style, status_symbol) = match &app.sync_status {
        SyncStatus::Synced => (
            "Synced".to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            "✓",
        ),
        SyncStatus::Syncing => (
            "Syncing".to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            "⟳",
        ),
    };

    let header_text = vec![Line::from(vec![
        Span::raw("Helios - "),
        Span::styled(&app.chain_name, Style::default().fg(Color::Cyan)),
        Span::raw("  |  Status: "),
        Span::styled(format!("{status_text} {status_symbol}"), status_style),
        Span::raw(format!("  |  Uptime: {hours}h {minutes}m {seconds}s")),
    ])];

    let header = Paragraph::new(header_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White)),
        )
        .alignment(Alignment::Center);

    f.render_widget(header, area);
}

fn draw_main_stats<N: NetworkSpec>(f: &mut Frame, app: &App<N>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(area);

    draw_block_stats(f, app, chunks[0]);
    draw_latest_block_details(f, app, chunks[1]);
}

fn draw_block_stats<N: NetworkSpec>(f: &mut Frame, app: &App<N>, area: Rect) {
    let latest_age = app
        .block_history
        .iter()
        .find(|b| b.block_type == BlockType::Latest)
        .map(|b| format!("{}s", b.age().as_secs()))
        .unwrap_or_else(|| "N/A".to_string());

    // Show latest block
    let mut block_stats = vec![Line::from(vec![
        Span::raw("Latest:    "),
        Span::styled(
            app.latest_block
                .map(|n| n.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            Style::default().fg(Color::Green),
        ),
    ])];

    // Show safe block if available
    if let Some(safe_block) = app.safe_block {
        block_stats.push(Line::from(vec![
            Span::raw("Safe:      "),
            Span::styled(safe_block.to_string(), Style::default().fg(Color::Cyan)),
        ]));
    }

    // Show finalized block if available
    if let Some(finalized_block) = app.finalized_block {
        block_stats.push(Line::from(vec![
            Span::raw("Finalized: "),
            Span::styled(
                finalized_block.to_string(),
                Style::default().fg(Color::Blue),
            ),
        ]));
    }

    // Show latest block age
    block_stats.extend(vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("Age: "),
            Span::styled(latest_age, Style::default().fg(Color::Magenta)),
        ]),
    ]);

    let block = Paragraph::new(block_stats).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Blocks ")
            .style(Style::default().fg(Color::White)),
    );

    f.render_widget(block, area);
}

fn draw_latest_block_details<N: NetworkSpec>(f: &mut Frame, app: &App<N>, area: Rect) {
    let block_details = if let Some(latest_block) = app
        .block_history
        .iter()
        .find(|b| b.block_type == BlockType::Latest)
    {
        let timestamp_str = format_timestamp(latest_block.timestamp);
        let gas_percentage =
            (latest_block.gas_used as f64 / latest_block.gas_limit as f64 * 100.0) as u8;

        let mut details = vec![
            create_detail_line(
                "Number:   ",
                latest_block.number.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            create_detail_line(
                "Hash:     ",
                latest_block.hash.to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            create_detail_line(
                "Time:     ",
                timestamp_str,
                Style::default().fg(Color::White),
            ),
            create_detail_line(
                "Txs:      ",
                latest_block.transactions_count.to_string(),
                Style::default().fg(Color::Cyan),
            ),
            create_gas_line(
                gas_percentage,
                latest_block.gas_used,
                latest_block.gas_limit,
            ),
        ];

        if let Some(base_fee) = latest_block.base_fee {
            details.push(create_detail_line(
                "Base Fee: ",
                format!("{} gwei", base_fee / 1_000_000_000),
                Style::default().fg(Color::Magenta),
            ));
        }

        details
    } else {
        vec![]
    };

    let block = Paragraph::new(block_details).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Latest Block Details ")
            .style(Style::default().fg(Color::White)),
    );

    f.render_widget(block, area);
}

fn draw_block_history<N: NetworkSpec>(f: &mut Frame, app: &App<N>, area: Rect) {
    let history_items: Vec<ListItem> = app
        .block_history
        .iter()
        .map(|block| {
            let (block_type_symbol, block_type_text, type_style) = match block.block_type {
                BlockType::Latest => ("▶", "Latest", Style::default().fg(Color::Green)),
                BlockType::Finalized => ("■", "Final ", Style::default().fg(Color::Blue)),
                BlockType::Safe => ("◆", "Safe  ", Style::default().fg(Color::Cyan)),
            };

            let timestamp = format_timestamp(block.timestamp);

            let age = block.age();
            let age_str = if age.as_secs() < 60 {
                format!("{}s ago", age.as_secs())
            } else if age.as_secs() < 3600 {
                format!("{}m ago", age.as_secs() / 60)
            } else {
                format!("{}h ago", age.as_secs() / 3600)
            };

            let line = Line::from(vec![
                Span::styled(block_type_symbol, type_style.add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(timestamp, Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(block_type_text, type_style),
                Span::raw("  "),
                Span::styled(
                    block.number.to_string(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(age_str, Style::default().fg(Color::Gray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let history = List::new(history_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Recent Blocks ")
            .style(Style::default().fg(Color::White)),
    );

    f.render_widget(history, area);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let footer_text = Line::from(vec![
        Span::styled("[q]", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("uit"),
    ]);

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Left);

    f.render_widget(footer, area);
}

fn format_gas(gas: u64) -> String {
    if gas >= 1_000_000 {
        format!("{:.1}M", gas as f64 / 1_000_000.0)
    } else if gas >= 1_000 {
        format!("{:.1}k", gas as f64 / 1_000.0)
    } else {
        gas.to_string()
    }
}

fn format_timestamp(timestamp: u64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp as i64, 0)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "N/A".to_string())
}

fn get_gas_color(percentage: u8) -> Color {
    if percentage > 90 {
        Color::Red
    } else if percentage > 70 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn create_detail_line(label: &'static str, value: String, style: Style) -> Line<'static> {
    Line::from(vec![Span::raw(label), Span::styled(value, style)])
}

fn create_gas_line(gas_percentage: u8, gas_used: u64, gas_limit: u64) -> Line<'static> {
    let gas_color = get_gas_color(gas_percentage);
    Line::from(vec![
        Span::raw("Gas:      "),
        Span::styled(format!("{gas_percentage}%"), Style::default().fg(gas_color)),
        Span::raw(" "),
        Span::styled(
            format!("({}/{})", format_gas(gas_used), format_gas(gas_limit)),
            Style::default().fg(Color::DarkGray),
        ),
    ])
}
