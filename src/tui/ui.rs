use crate::tui::app::{App, ViewMode};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Row, Table},
    Frame,
};
use unicode_width::UnicodeWidthStr;

pub fn draw_ui(f: &mut Frame, app: &mut App) {
    let chunks = if app.navigate_mode {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Top bar
                Constraint::Length(3), // Command input
                Constraint::Min(0),    // Main content
                Constraint::Length(3), // Status bar
            ])
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Top bar
                Constraint::Min(0),    // Main content
                Constraint::Length(3), // Status bar
            ])
            .split(f.area())
    };

    draw_top_bar(f, app, chunks[0]);

    if app.navigate_mode {
        draw_command_input(f, app, chunks[1]);
        app.content_area_height = chunks[2].height;
        match app.view_mode {
            ViewMode::List => draw_list_view(f, app, chunks[2]),
            ViewMode::Describe => draw_describe_view(f, app, chunks[2]),
            ViewMode::ConfirmDelete => {
                draw_list_view(f, app, chunks[2]);
                draw_delete_confirm(f, app, chunks[2]);
            }
            ViewMode::Help => draw_help_view(f, app, chunks[2]),
            ViewMode::About => draw_about_view(f, app, chunks[2]),
        }
        draw_status_bar(f, app, chunks[3]);
    } else {
        app.content_area_height = chunks[1].height;
        match app.view_mode {
            ViewMode::List => draw_list_view(f, app, chunks[1]),
            ViewMode::Describe => draw_describe_view(f, app, chunks[1]),
            ViewMode::ConfirmDelete => {
                draw_list_view(f, app, chunks[1]);
                draw_delete_confirm(f, app, chunks[1]);
            }
            ViewMode::Help => draw_help_view(f, app, chunks[1]),
            ViewMode::About => draw_about_view(f, app, chunks[1]),
        }
        draw_status_bar(f, app, chunks[2]);
    }
}

fn draw_top_bar(f: &mut Frame, app: &mut App, area: Rect) {
    use crate::tui::resources::ResourceAction;

    let context_display = std::env::var("EDGE_URL")
        .ok()
        .or(app.config.get_current_context().map(|c| c.url.clone()))
        .unwrap_or("Unknown context".to_string());

    let shortcuts = match app.view_mode {
        ViewMode::List => {
            // Build dynamic shortcuts based on current resource
            let action_shortcut = if let Some(item) = app.selected_item() {
                match item.deletable_action() {
                    Some(ResourceAction::Delete) => "<ctrl-d> delete    ",
                    Some(ResourceAction::Clear) => "<ctrl-d> clear     ",
                    None => "",
                }
            } else {
                ""
            };

            format!(
                "<↑↓> scroll    <d> describe    {}<r> toggle refresh    <:> navigate    <?> help",
                action_shortcut
            )
        }
        ViewMode::Describe => {
            if app.current_resource_type.is_single_item() {
                "<↑↓> scroll    <:> navigate".to_string()
            } else {
                "<esc> back    <↑↓> scroll    <:> navigate".to_string()
            }
        }
        ViewMode::ConfirmDelete => "<←→/tab> select    <enter> confirm    <esc> cancel".to_string(),
        ViewMode::Help => "<esc> back    <↑↓> scroll    <:> navigate".to_string(),
        ViewMode::About => "<esc> back    <↑↓> scroll    <:> navigate".to_string(),
    };

    let text = vec![Line::from(vec![
        Span::styled(&context_display, Style::default().fg(Color::Yellow)),
        Span::raw("  |  "),
        Span::styled(&shortcuts, Style::default().fg(Color::Gray)),
    ])];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" edgectl ");

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}

fn draw_list_view(f: &mut Frame, app: &mut App, area: Rect) {
    if app.items.is_empty() {
        let message = if app.loading {
            "Loading..."
        } else {
            "No items found"
        };

        let paragraph = Paragraph::new(message).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(format!(" {} ", app.current_resource_type.display_name())),
        );

        f.render_widget(paragraph, area);
        return;
    }

    // Get columns from first item
    let columns = app.items[0].columns();
    let header = Row::new(columns.iter().map(|c| c.as_str())).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    // Calculate column widths based on available width
    let available_width = area.width.saturating_sub(2) as usize; // subtract borders
    let num_columns = columns.len();
    let column_spacing = num_columns.saturating_sub(1); // spaces between columns
    let usable_width = available_width.saturating_sub(column_spacing);
    let col_width = usable_width.checked_div(num_columns).unwrap_or(0);

    // Calculate visible rows and apply scroll offset
    let visible_rows = (area.height.saturating_sub(3)) as usize; // subtract borders and header

    // Build rows with padded cells for full-width highlighting
    let rows: Vec<Row> = app
        .items
        .iter()
        .enumerate()
        .skip(app.scroll_offset)
        .take(visible_rows)
        .map(|(i, item)| {
            let row_data = item.row_data();
            let status_color = item.status_color();

            let style = if i == app.selected_index {
                // When highlighted: black text, status-colored background
                Style::default()
                    .bg(status_color)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                // When not highlighted: status-colored text
                Style::default().fg(status_color)
            };

            // Pad each cell to fill column width using unicode-aware width
            let cells: Vec<String> = row_data
                .into_iter()
                .map(|content| {
                    let display_width = content.width();
                    if display_width >= col_width {
                        let mut truncated = content.clone();
                        while truncated.width() > col_width && !truncated.is_empty() {
                            truncated.pop();
                        }
                        let padding = col_width.saturating_sub(truncated.width());
                        format!("{}{}", truncated, " ".repeat(padding))
                    } else {
                        let padding = col_width.saturating_sub(display_width);
                        format!("{}{}", content, " ".repeat(padding))
                    }
                })
                .collect();

            Row::new(cells).style(style).height(1)
        })
        .collect();

    // Use fixed length constraints for predictable widths
    let widths: Vec<Constraint> = (0..num_columns)
        .map(|_| Constraint::Length(col_width as u16))
        .collect();

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(format!(
                    " {} ({}) ",
                    app.current_resource_type.display_name(),
                    app.items.len()
                )),
        )
        .column_spacing(1);

    f.render_widget(table, area);
}

fn draw_describe_view(f: &mut Frame, app: &mut App, area: Rect) {
    if let Some(item) = app.selected_item() {
        let has_thumbnails = !app.thumbnails.is_empty() || !app.inactive_channels.is_empty();

        let (yaml_area, thumbnail_area) = if has_thumbnails {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(area);
            (chunks[0], Some(chunks[1]))
        } else {
            (area, None)
        };

        let yaml_content = item.to_yaml();
        let lines: Vec<&str> = yaml_content.lines().collect();

        // Apply scroll offset and style YAML keys
        let visible_lines: Vec<Line> = lines
            .iter()
            .skip(app.scroll_offset)
            .take(area.height.saturating_sub(2) as usize)
            .map(|line| {
                // Parse YAML line to style keys
                if let Some(colon_pos) = line.find(':') {
                    // Split into key and value parts
                    let (before_colon, after_colon) = line.split_at(colon_pos);

                    // Check if this looks like a YAML key (not a colon in a value)
                    // Keys should start with optional whitespace followed by alphanumeric/underscore/hyphen
                    let trimmed = before_colon.trim_start();

                    // Handle list items that start with "- "
                    let key_part = if let Some(stripped) = trimmed.strip_prefix("- ") {
                        stripped
                    } else {
                        trimmed
                    };

                    let is_key = !key_part.is_empty()
                        && key_part
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '-');

                    if is_key {
                        let indent = &before_colon[..before_colon.len() - trimmed.len()];

                        if let Some(stripped) = trimmed.strip_prefix("- ") {
                            // List item: render "- " separately, then highlight the key
                            Line::from(vec![
                                Span::raw(indent),
                                Span::raw("- "),
                                Span::styled(
                                    stripped,
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::raw(after_colon),
                            ])
                        } else {
                            // Regular key
                            Line::from(vec![
                                Span::raw(indent),
                                Span::styled(
                                    trimmed,
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::raw(after_colon),
                            ])
                        }
                    } else {
                        Line::from(line.to_string())
                    }
                } else {
                    Line::from(line.to_string())
                }
            })
            .collect();

        let paragraph = Paragraph::new(visible_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(format!(
                    " {} - {} ",
                    app.current_resource_type.display_name(),
                    item.name()
                )),
        );

        f.render_widget(paragraph, yaml_area);

        if let Some(thumb_area) = thumbnail_area {
            draw_thumbnail_panel(f, app, thumb_area);
        }
    }
}

fn draw_thumbnail_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Thumbnails ");

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if app.thumbnails.is_empty() && app.inactive_channels.is_empty() {
        return;
    }

    // Calculate layout: each thumbnail gets space + 1 line for label
    // Reserve space at the bottom for inactive channels if any
    let inactive_height = if app.inactive_channels.is_empty() {
        0
    } else {
        2 // 1 line for "Inactive:" label + 1 line for channel list
    };

    let thumbnail_count = app.thumbnails.len();
    let available_height = inner_area.height.saturating_sub(inactive_height as u16);

    if thumbnail_count > 0 {
        // Each thumbnail gets equal share of space, with 1 line reserved for the label below
        let height_per_thumbnail = available_height / thumbnail_count as u16;

        for (i, entry) in app.thumbnails.iter_mut().enumerate() {
            // Calculate layout
            let thumb_height = height_per_thumbnail.saturating_sub(1); // Reserve 1 line for label
            if thumb_height == 0 {
                continue;
            }
            let y_offset = i as u16 * height_per_thumbnail;
            if y_offset >= available_height {
                break;
            }

            // Draw channel label below the thumbnail
            if let Some(channel_id) = entry.channel_id {
                let label_rect = Rect {
                    x: inner_area.x,
                    y: inner_area.y + y_offset,
                    width: inner_area.width,
                    height: 1,
                };

                let label = format!("Channel {}", channel_id);

                let label_widget = Paragraph::new(Line::from(Span::styled(
                    label,
                    Style::default().fg(Color::Gray),
                )));

                f.render_widget(label_widget, label_rect);
            }

            // Draw thumbnail
            let thumb_rect = Rect {
                x: inner_area.x,
                y: inner_area.y + y_offset + 1,
                width: inner_area.width,
                height: thumb_height,
            };
            let image_widget =
                ratatui_image::StatefulImage::default().resize(ratatui_image::Resize::Scale(None));
            f.render_stateful_widget(image_widget, thumb_rect, &mut entry.protocol);
        }
    }

    // Draw inactive channels section at the bottom
    if !app.inactive_channels.is_empty() {
        let inactive_y = inner_area.y + available_height;

        // "Inactive:" label
        let label_rect = Rect {
            x: inner_area.x,
            y: inactive_y,
            width: inner_area.width,
            height: 1,
        };
        let label_widget = Paragraph::new(Line::from(Span::styled(
            "Inactive channels:",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
        f.render_widget(label_widget, label_rect);

        // Channel IDs list
        let channels_rect = Rect {
            x: inner_area.x,
            y: inactive_y + 1,
            width: inner_area.width,
            height: 1,
        };
        let channel_list: Vec<String> = app
            .inactive_channels
            .iter()
            .map(|id| id.to_string())
            .collect();
        let channels_text = channel_list.join(", ");
        let channels_widget = Paragraph::new(Line::from(Span::styled(
            channels_text,
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(channels_widget, channels_rect);
    }
}

fn draw_delete_confirm(f: &mut Frame, app: &mut App, area: Rect) {
    use crate::tui::resources::ResourceAction;

    if let Some(item) = app.selected_item() {
        let (action_verb, title) = match item.deletable_action() {
            Some(ResourceAction::Delete) => ("delete", " Delete "),
            Some(ResourceAction::Clear) => ("clear", " Clear "),
            None => ("", " Error "),
        };

        let message = format!(
            "Are you sure you want to {} '{}'?",
            action_verb,
            item.name()
        );

        // Cap dialog width at 60 characters (including borders)
        let max_dialog_width = 60u16.min(area.width);
        // Content width is dialog width minus borders
        let content_width = max_dialog_width.saturating_sub(2);
        let wrapped_lines = wrap_text(&message, content_width as usize);
        let message_line_count = wrapped_lines.len() as u16;

        // Calculate required height:
        // - 2 for borders (top and bottom)
        // - message_line_count for the message
        // - 1 for spacing
        // - 1 for buttons
        let dialog_height = 2 + message_line_count + 1 + 1;

        // Create centered dialog with calculated dimensions
        let dialog_area = centered_rect_fixed(max_dialog_width, dialog_height, area);

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(title);

        f.render_widget(outer_block, dialog_area);

        let inner_area = Rect {
            x: dialog_area.x + 1,
            y: dialog_area.y + 1,
            width: dialog_area.width.saturating_sub(2),
            height: dialog_area.height.saturating_sub(2),
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(message_line_count + 1), // message + spacing
                Constraint::Length(1),                      // buttons
            ])
            .split(inner_area);

        let message_lines: Vec<Line> = wrapped_lines.into_iter().map(Line::from).collect();

        let message_paragraph = Paragraph::new(message_lines)
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center)
            .block(Block::default());

        f.render_widget(message_paragraph, chunks[0]);

        let cancel_text = if app.delete_button_selected == 0 {
            Span::styled(
                "  Cancel  ",
                Style::default()
                    .bg(Color::White)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw("  Cancel  ")
        };

        let ok_text = if app.delete_button_selected == 1 {
            Span::styled(
                "  OK  ",
                Style::default()
                    .bg(Color::Red)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw("  OK  ")
        };

        let button_line = Line::from(vec![cancel_text, Span::raw("     "), ok_text]);

        let buttons_paragraph = Paragraph::new(button_line)
            .alignment(Alignment::Center)
            .block(Block::default());

        f.render_widget(buttons_paragraph, chunks[1]);
    }
}

fn draw_status_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Status ");

    if let Some(ref error) = app.error_message {
        let status_text = vec![Line::from(vec![
            Span::styled(
                "Error: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(error, Style::default().fg(Color::Red)),
        ])];
        let paragraph = Paragraph::new(status_text).block(block);
        f.render_widget(paragraph, area);
    } else if app.loading {
        let status_text = vec![Line::from(Span::styled(
            "Loading...",
            Style::default().fg(Color::Yellow),
        ))];
        let paragraph = Paragraph::new(status_text).block(block);
        f.render_widget(paragraph, area);
    } else {
        // Create inner area without borders
        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        // Split into text area and progress bar area
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(30),    // Text area
                Constraint::Length(30), // Progress bar area (fixed width)
            ])
            .split(inner_area);

        // Draw the block (borders and title)
        f.render_widget(block, area);

        // Determine auto-refresh status text
        let status_msg = if app.auto_refresh_enabled {
            "Auto-refresh enabled"
        } else {
            "Auto-refresh disabled"
        };

        // Draw status text
        let status_text = vec![Line::from(vec![
            Span::styled("Ready", Style::default().fg(Color::Green)),
            Span::raw(" | "),
            Span::raw(status_msg),
        ])];
        let paragraph = Paragraph::new(status_text);
        f.render_widget(paragraph, chunks[0]);

        // Draw progress bar only if auto-refresh is enabled
        if app.auto_refresh_enabled {
            let elapsed_secs = app.last_refresh.elapsed().as_secs_f64();
            let refresh_interval = 5.0;
            let progress_ratio = (elapsed_secs / refresh_interval).min(1.0);
            let time_remaining = (refresh_interval - elapsed_secs).max(0.0).ceil() as u64;

            let gauge = Gauge::default()
                .gauge_style(
                    Style::default()
                        .fg(Color::Rgb(100, 100, 100))
                        .bg(Color::Rgb(50, 50, 50)),
                )
                .label(format!("Refresh in {} seconds", time_remaining))
                .ratio(progress_ratio);

            f.render_widget(gauge, chunks[1]);
        }
    }
}

fn draw_command_input(f: &mut Frame, app: &mut App, area: Rect) {
    // Build the command line with inline completion
    let mut spans = vec![Span::styled(":", Style::default().fg(Color::Yellow))];

    // Add user input in yellow/bold
    spans.push(Span::styled(
        &app.command_input,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));

    // Add completion suggestion in gray if available
    if let Some(ref completion) = app.completion_suggestion {
        if let Some(suffix) = completion.strip_prefix(&app.command_input) {
            spans.push(Span::styled(suffix, Style::default().fg(Color::DarkGray)));
        }
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Navigate "),
    );

    f.render_widget(paragraph, area);
}

fn draw_help_view(f: &mut Frame, app: &mut App, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled(
            "KEYBOARD SHORTCUTS",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "List View:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  ↑ / ↓ / j / k Navigate through items"),
        Line::from("  d             Describe selected item (detailed YAML view)"),
        Line::from("  Enter         Describe selected item (same as 'd')"),
        Line::from("  Ctrl-D        Delete/Clear selected item (if supported, with confirmation)"),
        Line::from("  :             Enter navigate mode"),
        Line::from("  r             Toggle auto-refresh (enabled by default)"),
        Line::from("  ?             Show this help"),
        Line::from(""),
        Line::from(Span::styled(
            "Describe View:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  ↑ / ↓ / j / k Scroll content"),
        Line::from("  :             Enter navigate mode"),
        Line::from("  Esc           Return to list view"),
        Line::from(""),
        Line::from(Span::styled(
            "Delete Confirmation:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  y             Confirm deletion"),
        Line::from("  n / Esc       Cancel deletion"),
        Line::from(""),
        Line::from(Span::styled(
            "Navigate Mode:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  :             Enter navigate mode"),
        Line::from("  Tab           Accept completion suggestion"),
        Line::from("  Enter         Execute command"),
        Line::from("  Esc           Close navigate mode"),
        Line::from(""),
        Line::from(Span::styled(
            "Global:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  Ctrl-C        Quit edgectl"),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "NAVIGATE MODE",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Command         ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Description                  ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Aliases",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from("  ────────────────────────────────────────────────────────────────────────"),
        Line::from(vec![
            Span::styled("  :input          ", Style::default().fg(Color::Green)),
            Span::raw("Switch to inputs view        "),
            Span::styled(":i :inputs", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :output         ", Style::default().fg(Color::Green)),
            Span::raw("Switch to outputs view       "),
            Span::styled(":o :outputs", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :output-list    ", Style::default().fg(Color::Green)),
            Span::raw("Switch to output lists view  "),
            Span::styled(":ol :output-lists", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :group-list     ", Style::default().fg(Color::Green)),
            Span::raw("Switch to group lists view   "),
            Span::styled(":gl :group-lists", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :appliance      ", Style::default().fg(Color::Green)),
            Span::raw("Switch to appliances view    "),
            Span::styled(":a :appliances", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :group          ", Style::default().fg(Color::Green)),
            Span::raw("Switch to groups view        "),
            Span::styled(":g :groups", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :region         ", Style::default().fg(Color::Green)),
            Span::raw("Switch to regions view       "),
            Span::styled(":r :regions", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :node           ", Style::default().fg(Color::Green)),
            Span::raw("Switch to nodes view         "),
            Span::styled(":n :nodes", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :tunnel         ", Style::default().fg(Color::Green)),
            Span::raw("Switch to tunnels view       "),
            Span::styled(":t :tunnels", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :settings       ", Style::default().fg(Color::Green)),
            Span::raw("Switch to settings view      "),
            Span::styled(":s", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :alarm          ", Style::default().fg(Color::Green)),
            Span::raw("Switch to active alarms view "),
            Span::styled(":al :alarms", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :alarm-history  ", Style::default().fg(Color::Green)),
            Span::raw("Switch to alarm history view "),
            Span::styled(":ah", Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  :context        ", Style::default().fg(Color::Green)),
            Span::raw("Switch to contexts view      "),
            Span::styled(":c :ctx :contexts", Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  :help           ", Style::default().fg(Color::Green)),
            Span::raw("Show this help               "),
        ]),
        Line::from(vec![
            Span::styled("  :about          ", Style::default().fg(Color::Green)),
            Span::raw("Show about information       "),
            Span::styled(":version", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  :q              ", Style::default().fg(Color::Green)),
            Span::raw("Quit application             "),
            Span::styled(":q!", Style::default().fg(Color::Green)),
        ]),
    ];

    // Apply scroll offset
    let visible_lines: Vec<Line> = help_text
        .into_iter()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(2) as usize)
        .collect();

    let paragraph = Paragraph::new(visible_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Help "),
    );

    f.render_widget(paragraph, area);
}

fn draw_about_view(f: &mut Frame, app: &mut App, area: Rect) {
    let version = option_env!("VERSION").unwrap_or("unknown");

    let about_text = vec![
        Line::from(""),
        Line::from("                $$\\                                 $$\\     $$\\ "),
        Line::from("                $$ |                                $$ |    $$ |"),
        Line::from(" $$$$$$\\   $$$$$$$ | $$$$$$\\   $$$$$$\\   $$$$$$$\\ $$$$$$\\   $$ |"),
        Line::from("$$  __$$\\ $$  __$$ |$$  __$$\\ $$  __$$\\ $$  _____|\\_$$  _|  $$ |"),
        Line::from("$$$$$$$$ |$$ /  $$ |$$ /  $$ |$$$$$$$$ |$$ /        $$ |    $$ |"),
        Line::from("$$   ____|$$ |  $$ |$$ |  $$ |$$   ____|$$ |        $$ |$$\\ $$ |"),
        Line::from("\\$$$$$$$\\ \\$$$$$$$ |\\$$$$$$$ |\\$$$$$$$\\ \\$$$$$$$\\   \\$$$$  |$$ |"),
        Line::from(" \\_______| \\_______| \\____$$ | \\_______| \\_______|   \\____/ \\__|"),
        Line::from("                    $$\\   $$ |                                  "),
        Line::from("                    \\$$$$$$  |                                  "),
        Line::from("                     \\______/                                   "),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "edgectl ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(version, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![Span::raw("Copyright (c) Net Insight AB")]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "GitHub: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "https://github.com/netinsight/edgectl",
                Style::default().fg(Color::Blue),
            ),
        ]),
    ];

    // Apply scroll offset
    let visible_lines: Vec<Line> = about_text
        .into_iter()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(2) as usize)
        .collect();

    let paragraph = Paragraph::new(visible_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" About "),
    );

    f.render_widget(paragraph, area);
}

// Helper function to wrap text to a specific width
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = word.width();

        if current_width == 0 {
            // First word on the line
            current_line = word.to_string();
            current_width = word_width;
        } else if current_width + 1 + word_width <= max_width {
            // Add space and word to current line
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_width;
        } else {
            // Start new line
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

// Helper function to create a centered rectangle with fixed dimensions
fn centered_rect_fixed(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;

    Rect {
        x,
        y,
        width: width.min(r.width),
        height: height.min(r.height),
    }
}
