//! TUI module for displaying cached disk usage statistics
//!
//! Provides an ncdu-like interface for navigating and viewing directory sizes

use std::io::{self, stdout};
use std::path::PathBuf;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::format_size;
use crate::scanner::DirStat;

/// Entry in the TUI directory list
struct DirEntry {
    path: PathBuf,
    name: String,
    size: u64,
    file_count: u64,
    has_children: bool,
}

/// State for the TUI application
struct App<'a> {
    /// Stored roots for lookup when at root level
    roots: Vec<&'a DirStat>,
    /// Stack of directory stats for navigation (parent directories)
    path_stack: Vec<&'a DirStat>,
    /// Current directory being viewed (None means we're at the roots list)
    current: Option<&'a DirStat>,
    /// List of entries in the current directory
    entries: Vec<DirEntry>,
    /// Currently selected index
    list_state: ListState,
    /// Should quit
    should_quit: bool,
}

impl<'a> App<'a> {
    fn new(roots: Vec<&'a DirStat>) -> Self {
        let entries: Vec<DirEntry> = roots
            .iter()
            .map(|stat| DirEntry {
                path: stat.path().to_path_buf(),
                name: stat
                    .path()
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| stat.path().display().to_string()),
                size: stat.total_size(),
                file_count: stat.file_count(),
                has_children: !stat.children().is_empty(),
            })
            .collect();

        let mut app = Self {
            roots,
            path_stack: Vec::new(),
            current: None,
            entries,
            list_state: ListState::default(),
            should_quit: false,
        };

        // Sort by size descending
        app.entries.sort_by(|a, b| b.size.cmp(&a.size));

        // Select first item if available
        if !app.entries.is_empty() {
            app.list_state.select(Some(0));
        }

        app
    }

    fn from_stat(stat: &'a DirStat) -> Self {
        let mut app = Self {
            roots: vec![stat],
            path_stack: Vec::new(),
            current: Some(stat),
            entries: Vec::new(),
            list_state: ListState::default(),
            should_quit: false,
        };

        app.populate_entries_from_current();
        app
    }

    fn populate_entries_from_current(&mut self) {
        if let Some(stat) = self.current {
            self.entries = stat
                .children()
                .values()
                .map(|child| DirEntry {
                    path: child.path().to_path_buf(),
                    name: child
                        .path()
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| child.path().display().to_string()),
                    size: child.total_size(),
                    file_count: child.file_count(),
                    has_children: !child.children().is_empty(),
                })
                .collect();
        }

        // Sort by size descending
        self.entries.sort_by(|a, b| b.size.cmp(&a.size));

        // Select first item if available
        if !self.entries.is_empty() {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
    }

    fn populate_entries_from_roots(&mut self) {
        self.entries = self
            .roots
            .iter()
            .map(|stat| DirEntry {
                path: stat.path().to_path_buf(),
                name: stat
                    .path()
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| stat.path().display().to_string()),
                size: stat.total_size(),
                file_count: stat.file_count(),
                has_children: !stat.children().is_empty(),
            })
            .collect();

        // Sort by size descending
        self.entries.sort_by(|a, b| b.size.cmp(&a.size));

        // Select first item if available
        if !self.entries.is_empty() {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
    }

    fn get_current_path(&self) -> String {
        if let Some(stat) = self.current {
            stat.path().display().to_string()
        } else {
            "Cached Roots".to_string()
        }
    }

    fn get_current_total_size(&self) -> u64 {
        if let Some(stat) = self.current {
            stat.total_size()
        } else {
            self.entries.iter().map(|e| e.size).sum()
        }
    }

    fn move_up(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected > 0 {
                self.list_state.select(Some(selected - 1));
            }
        }
    }

    fn move_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected < self.entries.len().saturating_sub(1) {
                self.list_state.select(Some(selected + 1));
            }
        }
    }

    fn enter_selected(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected < self.entries.len() && self.entries[selected].has_children {
                let selected_path = &self.entries[selected].path;

                // Find the DirStat for the selected entry
                let child_stat = if let Some(current) = self.current {
                    // We're inside a directory, look in its children
                    current.children().get(selected_path)
                } else {
                    // We're at root level, look in the roots
                    self.roots
                        .iter()
                        .find(|r| r.path() == selected_path)
                        .copied()
                };

                if let Some(stat) = child_stat {
                    // Push current to stack and navigate to child
                    if let Some(current) = self.current {
                        self.path_stack.push(current);
                    }
                    self.current = Some(stat);
                    self.populate_entries_from_current();
                }
            }
        }
    }

    fn go_back(&mut self) {
        if let Some(parent) = self.path_stack.pop() {
            self.current = Some(parent);
            self.populate_entries_from_current();
        } else if self.current.is_some() {
            // We're at a root, go back to roots list
            self.current = None;
            self.populate_entries_from_roots();
        }
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.enter_selected(),
            KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => self.go_back(),
            _ => {}
        }
    }
}

/// Run the TUI with a single DirStat (the root of a scanned directory)
pub fn run_tui(stat: &DirStat) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::from_stat(stat);

    // Main loop
    loop {
        terminal.draw(|frame| render(&app, frame))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                app.handle_key(key.code);
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Cleanup
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

/// Run the TUI with multiple cached roots
pub fn run_tui_with_roots(roots: Vec<&DirStat>) -> io::Result<()> {
    if roots.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No cached directories found",
        ));
    }

    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new(roots);

    // Main loop
    loop {
        terminal.draw(|frame| render(&app, frame))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                app.handle_key(key.code);
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Cleanup
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Create layout with header, main content, and footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Footer
        ])
        .split(area);

    // Header with current path and total size
    let total_size = format_size(app.get_current_total_size(), true);
    let header_text = format!("{} (Total: {})", app.get_current_path(), total_size);
    let header = Paragraph::new(header_text)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("acme-disk-use"),
        );
    frame.render_widget(header, chunks[0]);

    // Content - directory listing
    let items: Vec<ListItem> = app
        .entries
        .iter()
        .map(|entry| {
            let size_str = format_size(entry.size, true);
            let indicator = if entry.has_children { "/" } else { "" };
            let line = format!(
                "{:>12}  {:>6} files  {}{}",
                size_str, entry.file_count, entry.name, indicator
            );
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Directories"))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, chunks[1], &mut app.list_state.clone());

    // Footer with help text
    let help_text = "↑/k: Up | ↓/j: Down | Enter/→/l: Open | Backspace/←/h: Back | q/Esc: Quit";
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title("Help"));
    frame.render_widget(footer, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::SystemTime;

    fn create_test_stat() -> DirStat {
        DirStat {
            path: PathBuf::from("/test"),
            total_size: 1000,
            file_count: 10,
            last_scan: SystemTime::now(),
            children: HashMap::new(),
        }
    }

    #[test]
    fn test_dir_entry_sorting() {
        // Create test stats with different sizes
        let stat1 = DirStat {
            path: PathBuf::from("/test/small"),
            total_size: 100,
            file_count: 1,
            last_scan: SystemTime::now(),
            children: HashMap::new(),
        };

        let stat2 = DirStat {
            path: PathBuf::from("/test/large"),
            total_size: 1000,
            file_count: 10,
            last_scan: SystemTime::now(),
            children: HashMap::new(),
        };

        let roots: Vec<&DirStat> = vec![&stat1, &stat2];
        let app = App::new(roots);

        // Verify entries are sorted by size descending
        assert_eq!(app.entries.len(), 2);
        assert_eq!(app.entries[0].size, 1000); // Large first
        assert_eq!(app.entries[1].size, 100); // Small second
    }

    #[test]
    fn test_app_navigation() {
        let stat = create_test_stat();
        let mut app = App::from_stat(&stat);

        // Test that navigation doesn't crash with empty entries
        app.move_up();
        app.move_down();
        app.enter_selected();
        app.go_back();
    }

    #[test]
    fn test_go_back_to_roots() {
        // Create a root stat with children
        let child = DirStat {
            path: PathBuf::from("/test/child"),
            total_size: 500,
            file_count: 5,
            last_scan: SystemTime::now(),
            children: HashMap::new(),
        };

        let mut children = HashMap::new();
        children.insert(PathBuf::from("/test/child"), child);

        let root = DirStat {
            path: PathBuf::from("/test"),
            total_size: 1000,
            file_count: 10,
            last_scan: SystemTime::now(),
            children,
        };

        let roots: Vec<&DirStat> = vec![&root];
        let mut app = App::new(roots);

        // Start at roots list
        assert!(app.current.is_none());
        assert_eq!(app.get_current_path(), "Cached Roots");

        // Navigate into root
        app.enter_selected();
        assert!(app.current.is_some());
        assert_eq!(app.get_current_path(), "/test");

        // Go back to roots
        app.go_back();
        assert!(app.current.is_none());
        assert_eq!(app.get_current_path(), "Cached Roots");
    }
}
