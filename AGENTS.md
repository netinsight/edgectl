# AGENTS.md

This file provides guidance to AI agents such as Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**edgectl** is a Rust CLI tool for managing [Nimbra Edge](https://netinsight.net/nimbra-edge/) installations. It provides commands for controlling edge resources: inputs, outputs, appliances, nodes, regions, groups, tunnels, and settings.

## Development Commands

### Building

```bash
# Development build
cargo build

# Release build
cargo build --release

# Cross-compile for Linux (x86_64-unknown-linux-musl) using cargo-zigbuild
make target/x86_64-unknown-linux-musl/release/edgectl

# Cross-compile for macOS (aarch64-apple-darwin) using cargo-zigbuild
make target/aarch64-apple-darwin/release/edgectl
```

### Testing

```bash
# Run all tests
cargo test

# Run a single test by name
cargo test test_name

# Run tests in a specific module
cargo test module_name::
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint code
cargo clippy
```

**IMPORTANT: Code Quality Standards**

All code must meet these strict requirements:

1. **Zero Warnings Policy** - Treat all compiler warnings as errors. Code must compile with no warnings.

2. **No Dead Code** - Never leave unused structs, functions, methods, or imports. If code is not used, remove it immediately. Do not use `#[allow(dead_code)]` or similar attributes.

3. **Minimal Comments** - Only include comments when absolutely necessary to explain non-obvious logic or business rules. Do not add:
   - Self-explanatory comments (e.g., "// Find the list by name" before `client.find_lists(name)`)
   - Obvious descriptions of what the code does
   - TODO comments without immediate implementation
   - Commented-out code

4. **Clean Build Output** - Every build should be clean with no warnings or errors.

5. **Minimal Output** - Follow the output conventions:
   - Create/Add/Remove operations: Silent on success (no output)
   - Delete operations: Simple text like `Deleted <resource> '<name>'`
   - No emoji or color indicators (`green!("✓")`, `red!("✗")`, etc.)
   - Plain error messages without decorative symbols

**CRITICAL: Pre-Completion Checklist**

Before declaring any work complete, AI agents MUST run these commands in order:

```bash
# 1. Format code
cargo fmt

# 2. Check for linting issues
cargo clippy

# 3. Verify build succeeds
cargo build
```

ALL THREE must complete successfully with no warnings before declaring the task done. No exceptions.

### Building Debian Package

```bash
# Build .deb package (automatically installs cargo-zigbuild via Docker)
make deb

# Output: build/edgectl-v<version>.deb
```

### Release Process

```bash
# 1. Create and push a git tag
git tag -a v0.0.0-rc0
git push --tags

# 2. Build and publish release artifacts (requires gh CLI)
make release
```

This builds Linux (x86_64-unknown-linux-musl), macOS (aarch64-apple-darwin) binaries and a Debian package, then creates a GitHub release.

### Configuration for Development

```bash
export EDGE_URL="https://your-edge-api-endpoint"
export EDGE_PASSWORD="your-admin-password"
# EDGE_USER is optional, defaults to "admin"
```

## Architecture

### High-Level Structure

```
src/
├── main.rs           # Entry point, dispatches to subcommands
├── cli.rs            # Clap command tree builder
├── edge.rs           # Core API client, all data models, HTTP logic
├── colors.rs         # Terminal color macros (legacy, do not use in new code)
├── tui/              # Interactive TUI (see TUI Architecture below)
└── <resource>.rs     # Per-resource modules (input, output, appliance, etc.)
```

Run `ls src/` for the current module list. Shell completion has no module of its
own — it is dynamic, via `clap_complete::CompleteEnv` in `main.rs`.

### Key Architectural Patterns

#### 1. Centralized API Client (edge.rs)

All HTTP communication, data models, and serialization logic lives in `edge.rs` (~2,000+ lines). This is the single source of truth for:
- API client (`EdgeClient` struct with blocking reqwest client)
- All data structures (Input, Output, Appliance, Node, Region, etc.)
- Custom serde deserializers for API type conversions (u8 ↔ enums)
- Error types and HTTP status handling

**Do not** create API-related code outside edge.rs. All new resources, fields, or API methods must be added here.

#### 2. Module-per-Resource Pattern

Each resource domain (input, output, appliance, etc.) follows this structure:

```rust
// output_list.rs example
pub fn subcommand() -> Command {
    // Clap command definition with all subcommands
    Command::new("output-list")
        .subcommand(Command::new("list").about("List output lists"))
        .subcommand(Command::new("show").arg(/* ... */))
        // ...
}

pub fn run(args: &ArgMatches) {
    // Dispatch based on subcommand name
    match args.subcommand() {
        Some(("list", _)) => list(),
        Some(("show", sub_args)) => show(sub_args),
        Some(("create", sub_args)) => create(sub_args),
        Some(("delete", sub_args)) => delete(sub_args),
        // ...
    }
}

fn list() {
    let client = edge::new_client();  // Creates authenticated client
    let items = client.list_output_recipient_lists();
    // Format and print output
}
```

**Key points:**
- Each module exports `subcommand()` (builds Clap command tree) and `run()` (dispatcher)
- Handler functions use simple names: `list()`, `show()`, `create()`, `delete()`, `add_*()`, `remove_*()`
- Handlers call `edge::new_client()` to get an authenticated EdgeClient
- No async - uses `reqwest::blocking::Client`
- Handlers exit process on error (via expect/unwrap or explicit process::exit)

#### 3. Error Handling Strategy

The codebase uses **fast-fail** error handling:
- Configuration errors (missing env vars) → immediate exit with message
- Authentication failures → immediate exit with message
- API errors → unwrap/expect causes panic and exit
- No graceful error recovery or retries

When adding API methods to `edge.rs`:
- Return `Result<T, reqwest::Error>` for HTTP operations
- Let callers handle errors (usually via unwrap)
- API errors propagate as HTTP response body text

#### 4. CLI Construction (cli.rs)

The command tree is built in `cli.rs` by calling each module's `subcommand()` function:

```rust
pub(crate) fn build() -> Command {
    Command::new("edgectl")
        .about("Nimbra Edge CLI")
        .subcommand_required(true)
        .subcommand(input::subcommand())
        .subcommand(output::subcommand())
        // ...
}
```

Dispatching happens in `main.rs` using pattern matching on subcommand names.

#### 5. Output Formatting

**Table Output:** Uses the `tabled` crate with empty styling and title case headers:

```rust
use tabled::{builder::Builder, settings::Style};

let mut builder = Builder::default();
builder.push_record(["ID", "Name", "Description"]);  // Title case headers

for item in items {
    builder.push_record([&item.id, &item.name, &item.description]);
}

let mut table = builder.build();
table.with(Style::empty());
println!("{}", table);
```

**Output Conventions:**
- **Silent on success**: Create, add, and remove operations produce no output on success
- **Simple messages**: Delete operations output plain text: `Deleted <resource> '<name>'`
- **No emoji or color**: Avoid using `green!("✓")`, `red!("✗")` or similar status indicators
- **Error messages**: Plain text without emoji, e.g., `eprintln!("Resource '{}' not found", name);`

#### 6. Authentication & Session Management

Authentication is eager and cookie-based:
1. `edge::new_client()` creates client with cookie jar
2. Immediately calls `client.login(username, password)`
3. API returns session cookie, stored in cookie jar
4. All subsequent requests include cookie automatically
5. No explicit session refresh - relies on cookie validity

## TUI (Terminal User Interface) Architecture

The TUI provides an interactive, vim-inspired terminal interface for managing Nimbra Edge resources, accessible via `edgectl tui`. It offers real-time monitoring, keyboard-driven navigation, and comprehensive resource management without leaving the terminal.

### Overview

**Access:** `edgectl tui`

**Technology Stack:**
- **ratatui** (v0.29) - Terminal UI framework (widgets, rendering, layouts)
- **crossterm** (v0.28) - Cross-platform terminal manipulation (raw mode, events, mouse)
- **tokio** (v1) - Async runtime (features: rt, time, sync, macros)
- **serde_saphyr** (v0.0.8) - YAML serialization for detailed resource views
- **unicode-width** (v0.2.0) - Unicode-aware string width calculations

### File Structure

```
src/tui/
├── mod.rs         # Entry point, creates authenticated client and app
├── app.rs         # Core application state and business logic
├── events.rs      # Event loop, input handling, terminal setup/teardown
├── resources.rs   # Resource type definitions and API integration
└── ui.rs          # Rendering logic for all views
```

### Module Responsibilities

#### mod.rs - Entry Point
- Validates environment variables (EDGE_URL, EDGE_PASSWORD, EDGE_USER)
- Creates authenticated EdgeClient using `edge::new_client()`
- Initializes App state
- Delegates to event loop in `events::run_app()`

#### app.rs - Application State & Business Logic

**Core State Structure:**
```rust
pub struct App {
    pub client: EdgeClient,              // Authenticated API client
    pub current_resource_type: ResourceType,
    pub items: Vec<ResourceItem>,        // Current resource list
    pub selected_index: usize,           // Selected item in list
    pub view_mode: ViewMode,             // Current view (List/Describe/etc)
    pub last_refresh: Instant,           // For auto-refresh timing
    pub error_message: Option<String>,   // Display errors in status bar
    pub navigate_mode: bool,             // Vim-style navigate mode active
    pub command_input: String,           // Command input buffer
    pub completion_suggestion: Option<String>, // TAB completion
    pub scroll_offset: usize,            // Scroll position in describe view
    pub loading: bool,                   // Loading state indicator
    pub should_quit: bool,               // Exit flag
    pub auto_refresh_enabled: bool,      // 10-second auto-refresh toggle
}
```

**View Modes:**
```rust
pub enum ViewMode {
    List,            // Main list view of resources
    Describe,        // Detailed YAML view of selected resource
    ConfirmDelete,   // Delete/clear confirmation dialog
    Help,            // Help screen with keybindings
    About,           // About/version screen
}
```

**Key Methods:**
- `new(client)` - Initialize with authenticated client, default to Input resource
- `refresh_data()` - Fetch fresh data for current resource type
- `switch_resource(type)` - Change resource type and reset state
- `move_selection_up/down()` - Navigate list items
- `enter_view_mode(mode)` - Transition between views (validates selected item exists)
- `enter/exit_navigate_mode()` - Toggle vim-style navigate mode
- `calculate_completion()` - Provide TAB completion for commands
- `execute_command(cmd)` - Handle command execution (navigation, quit, help)
- `confirm_action()` - Execute delete/clear operations
- `toggle_auto_refresh()` - Enable/disable 10-second auto-refresh

#### events.rs - Event Loop & Input Handling

**Terminal Lifecycle:**
1. Enable raw mode (character-by-character input, no echo)
2. Enter alternate screen (preserves user's terminal)
3. Enable mouse capture
4. Run event loop
5. Restore terminal state on exit (panic-safe via scopeguard)

**Event Polling:**
- 100ms timeout for refresh checking
- Handles keyboard events (KeyCode), mouse events (scroll), Ctrl+C globally
- Dispatches to mode-specific input handlers

**Input Handlers by Mode:**
- `handle_list_mode_input()` - Up/Down navigation, d/Enter (describe), Ctrl-D (delete), r (refresh), a (toggle auto-refresh), : (navigate mode), ? (help)
- `handle_navigate_mode_input()` - Text input, TAB (completion), Enter (execute), Esc (cancel)
- `handle_describe_mode_input()` - Up/Down scrolling, Esc (back to list), : (navigate mode)
- `handle_delete_confirm_input()` - y (confirm), n/Esc (cancel)
- `handle_help_mode_input()` / `handle_about_mode_input()` - Up/Down scrolling, Esc (exit)
- `handle_mouse_event()` - Mouse wheel scrolling in all scrollable views

**Auto-Refresh Logic:**
- Checks every 100ms if 10 seconds elapsed since last refresh
- Silently refreshes data (errors ignored during auto-refresh)
- Can be toggled with `a` key

#### resources.rs - Resource Abstraction Layer

**Supported Resource Types:**
```rust
pub enum ResourceType {
    Input, Output, OutputList, GroupList,
    Appliance, Group, Region, Node,
    Tunnel, Settings,
    Alarm,          // Active alarms
    AlarmHistory,   // Historical alarms
}
```

**Resource Item Wrapper:**
```rust
pub enum ResourceItem {
    Input(Input),
    Output(Output),
    OutputList(OutputListWithOutputs),      // Enriched with member outputs
    GroupList(GroupListWithGroups),         // Enriched with member groups
    Appliance(Appliance),
    Group(Group),
    Region(Region),
    Node(KubernetesNode),
    Tunnel(Tunnel),
    Settings(GlobalSettings),
    Alarm(AlarmWithEntities),               // Enriched with input/output names
    AlarmHistory(AlarmHistoryWithEntities), // Enriched with entity names
}
```

**Resource Actions:**
```rust
pub enum ResourceAction {
    Delete,  // Permanent deletion (most resources)
    Clear,   // Dismiss/acknowledge (alarms only)
}
```

**Key Methods:**
- `name()` - Extract display name from resource
- `columns()` - Return table column headers for resource type
- `row_data()` - Extract row values for table display
- `to_yaml()` - Serialize resource to YAML string for describe view
- `status_color()` - Return color based on health state (Blue=healthy, Orange=warning, Red=error, White=neutral)
- `deletable_action()` - Return Delete/Clear action or None if resource is read-only

**API Integration:**
- `fetch_resources(client, type)` - Fetch all items for resource type from EdgeClient
- `delete_resource(client, item)` - Delete resource via appropriate API method
- `clear_resource(client, item)` - Clear alarm via `client.clear_alarm()`

**Special Handling:**
- **OutputList/GroupList**: Fetches member data separately and enriches with full output/group details
- **Alarms**: Resolves input/output IDs to names by fetching full lists and building lookup maps
- **AlarmHistory**: Fetches last 100 entries with entity name resolution

#### ui.rs - Rendering Layer

**Layout Structure:**
```
┌────────────────────────────────────────────┐
│ Top Bar (EDGE_URL + dynamic shortcuts)    │
├────────────────────────────────────────────┤
│ Command Input (only in navigate mode)     │  ← Optional
├────────────────────────────────────────────┤
│                                            │
│         Main Content Area                  │
│  (list/describe/confirm/help/about views)  │
│                                            │
├────────────────────────────────────────────┤
│ Status Bar (error/loading/ready + timer)  │
└────────────────────────────────────────────┘
```

**Rendering Functions:**
- `draw_ui(frame, app)` - Main dispatcher based on command_mode and view_mode
- `draw_top_bar(frame, area, app)` - EDGE_URL + dynamic shortcuts (changes based on view/resource capabilities)
- `draw_list_view(frame, area, app)` - Table with status-colored rows, full-width highlighting for selected item
- `draw_describe_view(frame, area, app)` - Scrollable YAML view of selected resource
- `draw_delete_confirm(frame, area, app)` - Centered confirmation dialog
- `draw_command_input(frame, area, app)` - Command line with inline gray completion suffix
- `draw_status_bar(frame, area, app)` - Error (red) / Loading / Ready state + refresh countdown gauge
- `draw_help_view(frame, area, app)` - Scrollable help documentation with keybindings
- `draw_about_view(frame, area, app)` - ASCII art logo + version/author info

**Styling Conventions:**
- **Colors**: Cyan borders, Yellow highlights, Blue (healthy), Orange (warning), Red (error), White (neutral)
- **Selected row**: Status-colored background, black text, bold
- **Unselected row**: Status-colored foreground text
- **Table style**: `Style::empty()` for clean, borderless tables
- **Full-width highlighting**: Pad cells to fill column width for selected row
- **No emoji**: Plain text only for terminal compatibility

### User Interaction Patterns

#### Navigate Mode (vim-style)
- Press `:` to enter navigate mode
- Type command with TAB completion (shows gray suffix)
- Available commands: `input`, `output`, `output-list`, `group-list`, `appliance`, `group`, `region`, `node`, `tunnel`, `settings`, `alarm`, `alarm-history`, `help`, `about`, `version`, `q`, `q!`
- Press Enter to execute, Esc to cancel

#### List View Keybindings
- `↑/↓` or mouse wheel - Navigate items
- `d` or `Enter` - Describe selected item (view details)
- `Ctrl-D` - Delete/clear selected item (if supported by resource type)
- `r` - Manual refresh
- `a` - Toggle auto-refresh (10-second interval)
- `:` - Enter navigate mode
- `?` - Show help
- `Esc` - Cancel/back

#### Describe View
- `↑/↓` or mouse wheel - Scroll content
- `Esc` - Return to list view
- `:` - Enter navigate mode

#### Delete Confirmation Dialog
- `y` - Confirm action
- `n` or `Esc` - Cancel

#### Global
- `Ctrl-C` - Quit immediately

### State Management

**Single-Threaded, Synchronous:**
- No async/await in TUI code (uses blocking EdgeClient)
- Single event loop with 100ms polling interval
- State mutations happen inline during event handling
- No complex state machine - simple mode transitions

**Error Handling:**
- Errors stored in `app.error_message: Option<String>`
- Displayed in red in status bar
- Cleared on next successful operation
- Manual refresh retries failed operations

**Loading States:**
- `app.loading` flag set during API calls
- Displays "Loading..." in status bar
- No spinner animation (keeps UI simple)

### Adding a New Resource Type to TUI

To add support for a new resource (e.g., `Stream`):

1. **Add to ResourceType enum** (resources.rs):
   ```rust
   pub enum ResourceType {
       // ... existing types
       Stream,
   }
   ```

2. **Implement string parsing** in `ResourceType::from_str()`:
   ```rust
   "stream" | "streams" => Some(Self::Stream),
   ```

3. **Add display name** in `ResourceType::display_name()`:
   ```rust
   Self::Stream => "Streams",
   ```

4. **Add to ResourceItem enum**:
   ```rust
   pub enum ResourceItem {
       // ... existing variants
       Stream(Stream),
   }
   ```

5. **Implement ResourceItem trait methods**:
   ```rust
   // In name() method:
   Self::Stream(s) => &s.name,

   // In columns() method:
   Self::Stream(_) => vec!["Name", "URL", "Status"],

   // In row_data() method:
   Self::Stream(s) => vec![s.name.clone(), s.url.clone(), format!("{:?}", s.status)],

   // In to_yaml() method:
   Self::Stream(s) => serde_saphyr::to_string(s).unwrap_or_default(),

   // In status_color() method:
   Self::Stream(s) => match s.status {
       StreamStatus::Active => Color::Blue,
       StreamStatus::Error => Color::Red,
       _ => Color::White,
   },

   // In deletable_action() method:
   Self::Stream(_) => Some(ResourceAction::Delete),
   ```

6. **Add fetch logic** in `fetch_resources()`:
   ```rust
   ResourceType::Stream => {
       let items = client.list_streams()?;
       Ok(items.into_iter().map(ResourceItem::Stream).collect())
   }
   ```

7. **Add delete logic** in `delete_resource()` (if deletable):
   ```rust
   ResourceItem::Stream(s) => {
       client.delete_stream(&s.id)?;
       Ok(())
   }
   ```

8. **Update command completion** in `App::calculate_completion()`:
   ```rust
   let commands = [
       // ... existing commands
       "stream", "streams",
   ];
   ```

9. **Ensure API methods exist** in `edge.rs`:
   ```rust
   impl EdgeClient {
       pub fn list_streams(&self) -> Result<Vec<Stream>, reqwest::Error> { /* ... */ }
       pub fn delete_stream(&self, id: &str) -> Result<(), reqwest::Error> { /* ... */ }
   }
   ```

10. **Add to help page** in `draw_help_view()` function in `ui.rs`:
    ```rust
    // In the NAVIGATE MODE section, add the new command entry
    Line::from(vec![
        Span::styled("  :stream     ", Style::default().fg(Color::Green)),
        Span::raw("Switch to streams view       "),
        Span::styled(":streams", Style::default().fg(Color::Green)),
    ]),
    ```
    **CRITICAL**: Every new resource type MUST be added to the help page. The help page documents all available navigation commands for users. Missing commands in the help page will confuse users who cannot discover features.

### Integration with Main CLI

**CLI Registration** (cli.rs:40):
```rust
.subcommand(Command::new("tui").about("Open interactive TUI"))
```

**Main Dispatcher** (main.rs):
```rust
Some(("tui", _)) => {
    if let Err(e) = tui::run() {
        eprintln!("Error running TUI: {}", e);
        process::exit(1);
    }
}
```

**Environment Requirements:**
- `EDGE_URL` - Required, base API URL
- `EDGE_PASSWORD` - Required, admin password
- `EDGE_USER` - Optional, defaults to "admin"

### Development Guidelines

#### Adding a New View Mode

1. Add to `ViewMode` enum (app.rs)
2. Create input handler in events.rs (e.g., `handle_your_mode_input()`)
3. Add dispatcher case in `run_app()` event loop
4. Create rendering function in ui.rs (e.g., `draw_your_view()`)
5. Update shortcuts in `draw_top_bar()` to show mode-specific actions

#### Styling Guidelines

- **Use status colors consistently**: Blue (healthy/active), Orange (warning/degraded), Red (error/failed), White (neutral/unknown)
- **No emoji in TUI**: Plain text only for maximum terminal compatibility
- **Unicode-aware layout**: Use `unicode_width::UnicodeWidthStr::width()` for string width calculations
- **Empty table style**: Always use `Style::empty()` for clean, borderless tables
- **Full-width highlighting**: Pad table cells to fill column width for consistent row highlighting

#### Testing Considerations

- **Terminal compatibility**: Test on different terminals (iTerm2, Terminal.app, xterm, tmux, screen)
- **Color rendering**: Ensure RGB colors work or verify graceful fallback
- **Mouse support**: Test scroll behavior in list, describe, and help views
- **Window resize**: ratatui handles automatically, but verify layout doesn't break
- **Long content**: Test scrolling with resources that have many items or large YAML output
- **Error recovery**: Test network failures, API errors, authentication expiration
- **Auto-refresh**: Verify 10-second refresh works and can be toggled

### Key Files by Absolute Path

- `src/tui/mod.rs` - Entry point (80 lines)
- `src/tui/app.rs` - Application state (450+ lines)
- `src/tui/events.rs` - Event handling (350+ lines)
- `src/tui/resources.rs` - Resource abstraction (600+ lines)
- `src/tui/ui.rs` - Rendering (900+ lines)

### Dependencies in Cargo.toml

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["rt", "time", "sync", "macros"] }
serde-saphyr = "0.0.8"
unicode-width = "0.2.0"
```

### Architecture Benefits

The TUI follows a clean separation of concerns:
- **app.rs** - Pure state management, business logic, no rendering or input handling
- **events.rs** - Pure input handling and event loop, no rendering or business logic
- **resources.rs** - Pure API abstraction, no UI concerns
- **ui.rs** - Pure rendering, stateless functions that only read from App

This makes the TUI highly maintainable and testable. Each module can be understood and modified independently.

## Adding a New Command

To add a new command to an existing resource (e.g., `edgectl output-list add-output`):

1. **Add API method to `edge.rs`**:
   ```rust
   impl EdgeClient {
       pub fn add_output_to_list(
           &self,
           list_id: &str,
           list_name: &str,
           output_ids: Vec<String>,
       ) -> Result<(), EdgeError> {
           let url = format!("{}/api/outputRecipientList/{}", self.url, list_id);
           // ... implementation
       }
   }
   ```

2. **Add subcommand in resource module** (e.g., `output_list.rs`):
   ```rust
   pub fn subcommand() -> Command {
       Command::new("output-list")
           // ... existing subcommands
           .subcommand(
               Command::new("add-output")
                   .about("Add an output to an output list")
                   .arg(Arg::new("list").required(true).help("The name of the output list"))
                   .arg(Arg::new("output").required(true).num_args(1..).help("The name of the outputs to add"))
           )
   }
   ```

3. **Add handler in dispatcher**:
   ```rust
   pub fn run(args: &ArgMatches) {
       match args.subcommand() {
           // ... existing matches
           Some(("add-output", sub_args)) => add_output(sub_args),
           // ...
       }
   }

   fn add_output(args: &ArgMatches) {
       let list_name = args.get_one::<String>("list").unwrap();
       let output_names = args.get_many::<String>("output").unwrap();
       let client = new_client();

       let lists = client.find_output_recipient_lists(list_name)
           .expect("Failed to find output list");
       if lists.is_empty() {
           eprintln!("Output list '{}' not found", list_name);
           std::process::exit(1);
       }

       let output_ids = get_output_ids_by_names(&client, output_names);
       client.add_output_to_list(&lists[0].id, &lists[0].name, output_ids)
           .expect("Failed to add output to list");
       // Silent on success - no output
   }
   ```

## Adding a New Resource Module

To add a completely new resource (e.g., `stream`):

1. **Create `src/stream.rs`** following the module-per-resource pattern
2. **Add data structures to `edge.rs`** (Stream, ListStreamsResponse, etc.)
3. **Add API methods to `EdgeClient` in `edge.rs`**
4. **Add `mod stream;` to `main.rs`**
5. **Register in `cli.rs`**: `.subcommand(stream::subcommand())`
6. **Add dispatcher case in `main.rs`**: `Some(("stream", subcmd)) => stream::run(subcmd),`

## Build System Notes

### Cross-Compilation

The project uses `cargo-zigbuild` via Docker for cross-platform builds:
- Runs in container: `ghcr.io/rust-cross/cargo-zigbuild:0.19.7`
- Targets: `x86_64-unknown-linux-musl` (Linux), `aarch64-apple-darwin` (macOS)
- OpenSSL is vendored (via `features = ["vendored"]`) to enable static linking

### Debian Package Structure

Generated .deb includes:
- Binary: `/usr/bin/edgectl`
- Bash completions: `/usr/share/bash-completion/completions/edgectl`
- Zsh completions: `/usr/local/share/zsh/site-functions/edgectl`

Completions are generated by running the built binary with `COMPLETE=<shell>` set (see Makefile).

## Important Conventions

### Data Serialization

API responses often use numeric enums. Convert them using custom Deserialize implementations in edge.rs:

```rust
#[derive(Debug)]
pub enum InputType {
    Srt,
    RtmpPush,
    // ...
}

impl<'de> Deserialize<'de> for InputType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(Self::Srt),
            1 => Ok(Self::RtmpPush),
            _ => Err(D::Error::unknown_variant(&value.to_string(), &["0", "1"])),
        }
    }
}
```

For serialization back to API, implement `Serialize` trait similarly.

### Environment Variables

- `EDGE_URL`: Base API URL (required)
- `EDGE_PASSWORD`: Admin password (required)
- `EDGE_USER`: Username (optional, defaults to "admin")

### Table Output

Always use empty styling with title case headers:

```rust
use tabled::{builder::Builder, settings::Style};

let mut builder = Builder::default();
builder.push_record(["ID", "Name", "Description"]);  // Title case

for item in items {
    builder.push_record([&item.id, &item.name, &item.description]);
}

let mut table = builder.build();
table.with(Style::empty());
println!("{}", table);
```

### Output Style

**Do not use emoji or color indicators in output:**
- ❌ `println!("{} Created successfully", green!("✓"));`
- ✅ Silent on success (no output)
- ❌ `eprintln!("{} Resource not found", red!("✗"));`
- ✅ `eprintln!("Resource '{}' not found", name);`

**Output conventions:**
- Create/Add/Remove operations: Silent on success
- Delete operations: Plain text like `Deleted <resource> '<name>'`
- List operations: Table output only
- Show operations: Plain text fields, then table if applicable

## CI/CD

GitHub Actions workflow (`.github/workflows/rust.yml`):
- Runs on every push and PR
- Executes `cargo build` and `cargo test`
- Uses Ubuntu runner
