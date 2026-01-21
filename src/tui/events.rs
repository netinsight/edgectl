use crate::tui::app::{App, ViewMode};
use crate::tui::resources::{ResourceItem, ResourceType};
use crate::tui::ui::draw_ui;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

pub fn run_event_loop(mut app: App) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    loop {
        // Draw UI
        terminal.draw(|f| draw_ui(f, app))?;

        // Handle events with a timeout for refresh checking
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Handle Ctrl+C globally to exit
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        app.should_quit = true;
                    } else if app.navigate_mode {
                        // Handle navigate mode input regardless of view mode
                        handle_navigate_mode_input(app, key.code)?;
                    } else {
                        match app.view_mode {
                            ViewMode::List => handle_list_mode_input(app, key)?,
                            ViewMode::Describe => handle_describe_mode_input(app, key.code)?,
                            ViewMode::ConfirmDelete => handle_delete_confirm_input(app, key.code)?,
                            ViewMode::Help => handle_help_mode_input(app, key.code)?,
                            ViewMode::About => handle_about_mode_input(app, key.code)?,
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    handle_mouse_event(app, mouse.kind)?;
                }
                _ => {}
            }
        }

        // Check if we should quit
        if app.should_quit {
            break;
        }

        // Check if we should refresh (only if auto-refresh is enabled)
        if app.auto_refresh_enabled && app.should_refresh() {
            let _ = app.refresh_data();
            if app.view_mode == ViewMode::Describe {
                app.fetch_thumbnail_for_current_item();
            }
        }
    }

    Ok(())
}

fn handle_list_mode_input(app: &mut App, key: event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('?') => {
            app.enter_view_mode(ViewMode::Help);
        }
        KeyCode::Char('d') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.enter_view_mode(ViewMode::Describe);
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.enter_view_mode(ViewMode::ConfirmDelete);
        }
        KeyCode::Enter => {
            if app.current_resource_type == ResourceType::Context {
                if let Some(ResourceItem::Context(ctx)) = app.selected_item() {
                    let context_name = ctx.name.clone();
                    if let Err(e) = app.switch_context(&context_name) {
                        app.error_message = Some(format!("Failed to switch context: {}", e));
                    }
                }
            } else {
                app.enter_view_mode(ViewMode::Describe);
            }
        }
        KeyCode::Char(':') => {
            app.enter_navigate_mode();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_selection_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_selection_down();
        }
        KeyCode::Char('r') => {
            app.toggle_auto_refresh();
        }
        _ => {}
    }
    Ok(())
}

fn handle_navigate_mode_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Char(c) => {
            app.handle_command_input(c);
        }
        KeyCode::Backspace => {
            app.handle_command_backspace();
        }
        KeyCode::Tab | KeyCode::Right => {
            app.accept_completion();
        }
        KeyCode::Enter => {
            app.execute_command()?;
        }
        KeyCode::Esc => {
            app.exit_navigate_mode();
        }
        _ => {}
    }
    Ok(())
}

fn handle_describe_mode_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Char(':') => {
            app.enter_navigate_mode();
        }
        KeyCode::Esc => {
            if !app.current_resource_type.is_single_item() {
                app.exit_to_list_view();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll_down_current_view();
        }
        _ => {}
    }
    Ok(())
}

fn handle_delete_confirm_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::Char('h') | KeyCode::Char('l') => {
            app.delete_button_selected = 1 - app.delete_button_selected;
        }
        KeyCode::Enter => {
            if app.delete_button_selected == 1 {
                app.confirm_action()?;
            } else {
                app.exit_to_list_view();
            }
        }
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            app.confirm_action()?;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.exit_to_list_view();
        }
        _ => {}
    }
    Ok(())
}

fn handle_help_mode_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Char(':') => {
            app.enter_navigate_mode();
        }
        KeyCode::Esc => {
            app.exit_to_list_view();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll_down_current_view();
        }
        _ => {}
    }
    Ok(())
}

fn handle_about_mode_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Char(':') => {
            app.enter_navigate_mode();
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.exit_to_list_view();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll_down_current_view();
        }
        _ => {}
    }
    Ok(())
}

fn handle_mouse_event(app: &mut App, kind: MouseEventKind) -> Result<()> {
    if app.navigate_mode {
        return Ok(());
    }

    match kind {
        MouseEventKind::ScrollUp => match app.view_mode {
            ViewMode::List => app.move_selection_up(),
            ViewMode::Describe | ViewMode::Help | ViewMode::About => app.scroll_up(),
            ViewMode::ConfirmDelete => {}
        },
        MouseEventKind::ScrollDown => match app.view_mode {
            ViewMode::List => app.move_selection_down(),
            ViewMode::Describe | ViewMode::Help | ViewMode::About => app.scroll_down_current_view(),
            ViewMode::ConfirmDelete => {}
        },
        _ => {}
    }

    Ok(())
}
