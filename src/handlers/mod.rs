use crate::app::App;
use crate::config::ConnectionMode;
use crate::fl;
use crossterm::event::MouseEvent;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use std::io;

pub struct AppLayout {
    pub list_area: Rect,
    pub tabs_area: Rect,
}

pub fn get_layout(size: Rect) -> AppLayout {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search
            Constraint::Length(3), // Tabs
            Constraint::Min(0),    // Main
            Constraint::Length(1), // Status
        ])
        .split(size);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(67), // List
            Constraint::Min(0),         // Details
        ])
        .split(chunks[2]);

    AppLayout {
        tabs_area: chunks[1],
        list_area: main_chunks[0],
    }
}

pub fn is_in_rect(x: u16, y: u16, rect: Rect) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

pub fn handle_mouse_event(mouse: MouseEvent, app: &mut App, size: Rect) -> io::Result<bool> {
    // Un popup est ouvert : ignorer le clic pour ne pas changer la sélection
    // sous l'overlay (ex. SCP qui partirait vers un autre serveur).
    if app.has_modal_overlay() {
        return Ok(false);
    }

    let layout = get_layout(size);

    if is_in_rect(mouse.column, mouse.row, layout.tabs_area) {
        let tab_direct = fl!("tab-direct");
        let tab_jump = fl!("tab-jump");
        let tab_wallix = fl!("tab-wallix");
        let titles = [tab_direct, tab_jump, tab_wallix];
        let separator_width = 1;

        // Tabs block has Borders::ALL, so content starts at x+1
        let start_x = layout.tabs_area.x + 1;

        if mouse.column < start_x {
            return Ok(false);
        }

        let rel_x = (mouse.column - start_x) as usize;
        let mut current_x = 0;

        for (i, title) in titles.iter().enumerate() {
            let width = title.chars().count();
            // Check if click is strictly within the title text
            if rel_x >= current_x && rel_x < current_x + width {
                app.connection_mode = ConnectionMode::from_index(i);
                return Ok(true);
            }
            // Advance cursor (title + separator)
            current_x += width + separator_width;
        }

        return Ok(true);
    } else if is_in_rect(mouse.column, mouse.row, layout.list_area) {
        // Determine item index
        // List renders inside the block. Block has Borders::ALL -> 1px padding
        let inner_y = layout.list_area.y + 1;
        let inner_h = layout.list_area.height.saturating_sub(2);

        if mouse.row >= inner_y && mouse.row < inner_y + inner_h {
            let row_idx = (mouse.row - inner_y) as usize;

            let offset = app.list_state.offset();
            let target_index = offset + row_idx;

            // Check bounds
            if target_index < app.get_visible_items().len() {
                app.select(target_index);
                // Toggle expansion on single click if it's a group or env
                app.toggle_expansion();
                return Ok(true);
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    // ── is_in_rect ───────────────────────────────────────────────────────────

    #[test]
    fn is_in_rect_top_left_corner() {
        let r = Rect::new(10, 10, 20, 20);
        assert!(is_in_rect(10, 10, r));
    }

    #[test]
    fn is_in_rect_bottom_right_corner_exclusive() {
        let r = Rect::new(10, 10, 20, 20);
        // x + width = 30, y + height = 30 → exclusive boundary.
        assert!(!is_in_rect(30, 30, r));
        assert!(is_in_rect(29, 29, r));
    }

    #[test]
    fn is_in_rect_outside_left() {
        let r = Rect::new(10, 10, 20, 20);
        assert!(!is_in_rect(9, 15, r));
    }

    #[test]
    fn is_in_rect_outside_right() {
        let r = Rect::new(10, 10, 20, 20);
        assert!(!is_in_rect(30, 15, r));
    }

    #[test]
    fn is_in_rect_outside_top() {
        let r = Rect::new(10, 10, 20, 20);
        assert!(!is_in_rect(15, 9, r));
    }

    #[test]
    fn is_in_rect_outside_bottom() {
        let r = Rect::new(10, 10, 20, 20);
        assert!(!is_in_rect(15, 30, r));
    }

    // ── get_layout ───────────────────────────────────────────────────────────

    #[test]
    fn get_layout_areas_within_bounds() {
        let size = Rect::new(0, 0, 200, 50);
        let layout = get_layout(size);

        // Both areas must be within the terminal bounds.
        assert!(layout.list_area.x + layout.list_area.width <= size.width);
        assert!(layout.tabs_area.x + layout.tabs_area.width <= size.width);
        assert!(layout.list_area.y + layout.list_area.height <= size.height);
        assert!(layout.tabs_area.y + layout.tabs_area.height <= size.height);
    }

    #[test]
    fn get_layout_list_narrower_than_terminal() {
        let size = Rect::new(0, 0, 120, 40);
        let layout = get_layout(size);
        // List pane is 67% of horizontal space → strictly narrower than full width.
        assert!(layout.list_area.width < size.width);
    }

    #[test]
    fn get_layout_tabs_height_is_three() {
        let size = Rect::new(0, 0, 100, 30);
        let layout = get_layout(size);
        assert_eq!(layout.tabs_area.height, 3);
    }
}
