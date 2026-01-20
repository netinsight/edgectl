use crate::config::Config;
use crate::edge::{new_client, EdgeClient};
use crate::tui::resources::{
    clear_resource, delete_resource, fetch_resources, ResourceAction, ResourceItem, ResourceType,
};
use anyhow::Result;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Describe,
    ConfirmDelete,
    Help,
    About,
}

pub struct ThumbnailEntry {
    pub protocol: ratatui_image::protocol::StatefulProtocol,
    pub channel_id: Option<u32>,
}

pub struct App {
    pub client: EdgeClient,
    pub current_resource_type: ResourceType,
    pub items: Vec<ResourceItem>,
    pub selected_index: usize,
    pub view_mode: ViewMode,
    pub last_refresh: Instant,
    pub error_message: Option<String>,
    pub navigate_mode: bool,
    pub command_input: String,
    pub completion_suggestion: Option<String>,
    pub scroll_offset: usize,
    pub loading: bool,
    pub should_quit: bool,
    pub auto_refresh_enabled: bool,
    pub content_area_height: u16,
    pub delete_button_selected: usize,
    pub thumbnails: Vec<ThumbnailEntry>,
    pub inactive_channels: Vec<u32>,
    pub config: Config,
}

impl App {
    pub fn new(client: EdgeClient) -> Result<Self> {
        let mut app = Self {
            client,
            current_resource_type: ResourceType::Input,
            items: Vec::new(),
            selected_index: 0,
            view_mode: ViewMode::List,
            last_refresh: Instant::now(),
            error_message: None,
            navigate_mode: false,
            command_input: String::new(),
            completion_suggestion: None,
            scroll_offset: 0,
            loading: false,
            should_quit: false,
            auto_refresh_enabled: true,
            content_area_height: 24,
            delete_button_selected: 0,
            thumbnails: Vec::new(),
            inactive_channels: Vec::new(),
            config: Config::load(),
        };

        app.refresh_data()?;
        Ok(app)
    }

    pub fn switch_context(&mut self, context_name: &str) -> Result<()> {
        self.config.set_current_context(context_name.to_owned())?;
        self.config.save()?;

        self.client = new_client();

        self.config = Config::load();

        self.refresh_data()?;

        Ok(())
    }

    pub fn refresh_data(&mut self) -> Result<()> {
        self.loading = true;
        self.error_message = None;

        match fetch_resources(&self.client, self.current_resource_type) {
            Ok(items) => {
                self.items = items;
                self.last_refresh = Instant::now();
                self.loading = false;

                if self.selected_index >= self.items.len() && !self.items.is_empty() {
                    self.selected_index = self.items.len() - 1;
                }

                if self.current_resource_type.is_single_item()
                    && !self.items.is_empty()
                    && self.view_mode == ViewMode::List
                {
                    self.view_mode = ViewMode::Describe;
                }
            }
            Err(e) => {
                self.error_message = Some(format!(
                    "Failed to fetch {}: {}",
                    self.current_resource_type, e
                ));
                self.loading = false;
            }
        }

        Ok(())
    }

    pub fn switch_resource(&mut self, resource_type: ResourceType) -> Result<()> {
        self.current_resource_type = resource_type;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.view_mode = ViewMode::List;
        self.refresh_data()?;

        if resource_type.is_single_item() && !self.items.is_empty() {
            self.view_mode = ViewMode::Describe;
        }

        Ok(())
    }

    pub fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;

            // Adjust scroll offset if selection moved above visible area
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        if !self.items.is_empty() && self.selected_index < self.items.len() - 1 {
            self.selected_index += 1;

            // Calculate visible rows (content area height - borders - header row)
            let visible_rows = (self.content_area_height.saturating_sub(3)) as usize;

            // Adjust scroll offset if selection moved below visible area
            if self.selected_index >= self.scroll_offset + visible_rows {
                self.scroll_offset = self.selected_index.saturating_sub(visible_rows - 1);
            }
        }
    }

    pub fn enter_view_mode(&mut self, mode: ViewMode) {
        if mode == ViewMode::Describe && self.items.is_empty() {
            return;
        }
        if mode == ViewMode::ConfirmDelete {
            if self.items.is_empty() {
                return;
            }
            // Check if selected item is actually deletable/clearable
            if let Some(item) = self.selected_item() {
                if item.deletable_action().is_none() {
                    return; // Don't enter delete mode for non-deletable resources
                }
            } else {
                return;
            }
            self.delete_button_selected = 0;
        }
        self.view_mode = mode;
        self.scroll_offset = 0;

        if mode == ViewMode::Describe {
            self.fetch_thumbnail_for_current_item();
        }
    }

    pub fn exit_to_list_view(&mut self) {
        self.view_mode = ViewMode::List;
        self.scroll_offset = 0;
        self.thumbnails.clear();
        self.inactive_channels.clear();
    }

    pub fn enter_navigate_mode(&mut self) {
        self.navigate_mode = true;
        self.command_input.clear();
        self.completion_suggestion = None;
    }

    pub fn exit_navigate_mode(&mut self) {
        self.navigate_mode = false;
        self.command_input.clear();
        self.completion_suggestion = None;
    }

    pub fn handle_command_input(&mut self, c: char) {
        self.command_input.push(c);
        self.calculate_completion();
    }

    pub fn handle_command_backspace(&mut self) {
        self.command_input.pop();
        self.calculate_completion();
    }

    pub fn calculate_completion(&mut self) {
        if self.command_input.is_empty() {
            self.completion_suggestion = None;
            return;
        }

        let input = self.command_input.as_str();
        let commands = [
            "input",
            "i",
            "output",
            "o",
            "output-list",
            "ol",
            "group-list",
            "gl",
            "appliance",
            "a",
            "group",
            "g",
            "region",
            "r",
            "node",
            "n",
            "tunnel",
            "t",
            "settings",
            "s",
            "alarm",
            "al",
            "alarm-history",
            "ah",
            "context",
            "ctx",
            "help",
            "about",
            "version",
            "q",
            "q!",
        ];

        let matches: Vec<&&str> = commands
            .iter()
            .filter(|r| r.starts_with(input) && r.len() > input.len())
            .collect();

        // Only set completion if exactly one match
        if matches.len() == 1 {
            self.completion_suggestion = Some(matches[0].to_string());
        } else {
            self.completion_suggestion = None;
        }
    }

    pub fn accept_completion(&mut self) {
        if let Some(ref completion) = self.completion_suggestion {
            self.command_input = completion.clone();
            self.completion_suggestion = None;
        }
    }

    pub fn execute_command(&mut self) -> Result<()> {
        let command = self.command_input.trim();

        match command {
            "q" | "q!" => {
                self.should_quit = true;
            }
            "help" => {
                self.exit_navigate_mode();
                self.enter_view_mode(ViewMode::Help);
            }
            "about" | "version" => {
                self.exit_navigate_mode();
                self.enter_view_mode(ViewMode::About);
            }
            _ => {
                if let Some(resource_type) = ResourceType::from_str(command) {
                    self.exit_navigate_mode();
                    self.switch_resource(resource_type)?;
                } else {
                    self.error_message = Some(format!("Unknown command: {}", command));
                    self.exit_navigate_mode();
                }
            }
        }

        Ok(())
    }

    pub fn confirm_action(&mut self) -> Result<()> {
        if self.items.is_empty() {
            return Ok(());
        }

        let item = &self.items[self.selected_index];

        match item.deletable_action() {
            Some(ResourceAction::Delete) => match delete_resource(&self.client, item) {
                Ok(_) => {
                    self.view_mode = ViewMode::List;
                    self.refresh_data()?;
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to delete: {}", e));
                    self.view_mode = ViewMode::List;
                }
            },
            Some(ResourceAction::Clear) => match clear_resource(&self.client, item) {
                Ok(_) => {
                    self.view_mode = ViewMode::List;
                    self.refresh_data()?;
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to clear: {}", e));
                    self.view_mode = ViewMode::List;
                }
            },
            None => {
                self.error_message = Some("This resource cannot be deleted or cleared".to_string());
                self.view_mode = ViewMode::List;
            }
        }

        Ok(())
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self, content_lines: usize, visible_height: usize) {
        let max_scroll = content_lines.saturating_sub(visible_height);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }

    pub fn scroll_down_current_view(&mut self) {
        let content_lines = match self.view_mode {
            ViewMode::Describe => self.get_describe_content_lines(),
            ViewMode::Help => self.get_help_content_lines(),
            ViewMode::About => self.get_about_content_lines(),
            _ => return,
        };

        let visible_height = self.content_area_height.saturating_sub(2) as usize;
        self.scroll_down(content_lines, visible_height);
    }

    pub fn get_describe_content_lines(&self) -> usize {
        if let Some(item) = self.selected_item() {
            item.to_yaml().lines().count()
        } else {
            0
        }
    }

    pub fn get_help_content_lines(&self) -> usize {
        51
    }

    pub fn get_about_content_lines(&self) -> usize {
        18
    }

    pub fn selected_item(&self) -> Option<&ResourceItem> {
        self.items.get(self.selected_index)
    }

    pub fn should_refresh(&self) -> bool {
        self.last_refresh.elapsed().as_secs() >= 5
    }

    pub fn toggle_auto_refresh(&mut self) {
        self.auto_refresh_enabled = !self.auto_refresh_enabled;
    }

    pub fn fetch_thumbnail_for_current_item(&mut self) {
        use crate::edge::{Input, ThumbnailMode};
        use ratatui_image::picker::Picker;

        self.thumbnails.clear();
        self.inactive_channels.clear();

        let input: Option<Input> = match self.selected_item() {
            Some(ResourceItem::Input(i)) => Some(i.clone()),
            Some(ResourceItem::Output(o)) => o
                .input
                .as_ref()
                .and_then(|id| self.client.get_input(id).ok()),
            _ => None,
        };

        let Some(input) = input else { return };

        match input.thumbnail_mode {
            ThumbnailMode::Core => {
                let path = format!("thumb/{}", input.id);
                if let Some(bytes) = self.client.fetch_thumbnail(&path) {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let picker =
                            Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
                        let protocol = picker.new_resize_protocol(img);
                        self.thumbnails.push(ThumbnailEntry {
                            protocol,
                            channel_id: None,
                        });
                    }
                }
            }
            ThumbnailMode::Edge => {
                let channels = input.get_all_channels();
                for channel in channels {
                    if channel.active {
                        let path = format!("thumbnailer/{}", channel.channel_id);
                        if let Some(bytes) = self.client.fetch_thumbnail(&path) {
                            if let Ok(img) = image::load_from_memory(&bytes) {
                                let picker = Picker::from_query_stdio()
                                    .unwrap_or_else(|_| Picker::halfblocks());
                                let protocol = picker.new_resize_protocol(img);
                                self.thumbnails.push(ThumbnailEntry {
                                    protocol,
                                    channel_id: Some(channel.channel_id),
                                });
                            }
                        }
                    } else {
                        self.inactive_channels.push(channel.channel_id);
                    }
                }
            }
            ThumbnailMode::None => {}
        }
    }
}
