use color_eyre::Result;
use dir_size::get_size_in_abbr_human_bytes;
use homedir::my_home;
use jwalk::WalkDir;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Margin, Rect},
    style::{self, Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Block, BorderType, Cell, HighlightSpacing, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
};
use rayon::prelude::*;
use std::{
    fs::{self, remove_dir_all},
    path::Path,
};
use style::palette::tailwind;
use unicode_width::UnicodeWidthStr;

const PALETTES: [tailwind::Palette; 4] = [
    tailwind::BLUE,
    tailwind::EMERALD,
    tailwind::INDIGO,
    tailwind::RED,
];
const INFO_TEXT: [&str; 2] = [
    "(Esc) quit | (↑) move up | (↓) move down | <BS>(→) next color | (←) previous color",
    "(Enter) select/deselect | (D) delete selected",
];

const ITEM_HEIGHT: usize = 4;

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app_result = App::new().run(terminal);
    ratatui::restore();
    app_result
}
struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    selected_row_style_fg: Color,
    selected_column_style_fg: Color,
    selected_cell_style_fg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
    footer_border_color: Color,
}

impl TableColors {
    const fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: tailwind::SLATE.c950,
            header_bg: color.c900,
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_row_style_fg: color.c400,
            selected_column_style_fg: color.c400,
            selected_cell_style_fg: color.c600,
            normal_row_color: tailwind::SLATE.c950,
            alt_row_color: tailwind::SLATE.c900,
            footer_border_color: color.c400,
        }
    }
}

#[derive(Debug, Clone)]
struct Data {
    name: String,
    parent_node_module: String,
    selected_for_deletion: String,
}

impl Data {
    const fn ref_array(&self) -> [&String; 3] {
        [
            &self.selected_for_deletion,
            &self.name,
            &self.parent_node_module,
        ]
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn parent(&self) -> &str {
        &self.parent_node_module
    }

    fn select(&self) -> &str {
        &self.selected_for_deletion
    }
}

struct App {
    state: TableState,
    items: Vec<Data>,
    longest_item_lens: (u16, u16, u16), // order is (name, address, email)
    scroll_state: ScrollbarState,
    colors: TableColors,
    color_index: usize,
    delete_folder: Vec<bool>,
}

impl App {
    fn new() -> Self {
        let data_vec = generate_data();
        let mut delete_files: Vec<bool> = Vec::new();
        for _ in 0..data_vec.len() {
            delete_files.push(false);
        }
        let mut scroll_bar_length = 0;
        if data_vec.len() != 0 {
            scroll_bar_length = data_vec.len() - 1;
        }
        Self {
            state: TableState::default().with_selected(0),
            longest_item_lens: constraint_len_calculator(&data_vec),
            scroll_state: ScrollbarState::new(scroll_bar_length * ITEM_HEIGHT),
            colors: TableColors::new(&PALETTES[0]),
            color_index: 0,
            items: data_vec,
            delete_folder: delete_files,
        }
    }
    pub fn next_row(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    pub fn previous_row(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    pub fn select_for_deletion(&mut self) {
        let i = match self.state.selected() {
            Some(i) => i,
            None => 0,
        };
        if self.delete_folder[i] {
            self.delete_folder[i] = false;
            self.items[i].selected_for_deletion = String::from("  ☐");
        } else {
            self.delete_folder[i] = true;
            self.items[i].selected_for_deletion = String::from("  ☑");
        }
    }

    pub fn next_color(&mut self) {
        self.color_index = (self.color_index + 1) % PALETTES.len();
    }

    pub fn previous_color(&mut self) {
        let count = PALETTES.len();
        self.color_index = (self.color_index + count - 1) % count;
    }

    pub fn set_colors(&mut self) {
        self.colors = TableColors::new(&PALETTES[self.color_index]);
    }

    pub fn remove_directories(&mut self) {
        // Collect the names of items to remove
        let items_to_remove: Vec<String> = self
            .items
            .clone()
            .into_par_iter()
            .filter_map(|i| {
                if i.selected_for_deletion == "  ☑" {
                    let _ = remove_dir_all(Path::new(&i.name));
                    Some(i.name)
                } else {
                    None
                }
            })
            .collect();

        // Now remove the items from `self.items` sequentially
        self.items
            .retain(|data| !items_to_remove.contains(&data.name));
    }

    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => self.next_row(),
                        KeyCode::Char('k') | KeyCode::Up => self.previous_row(),
                        KeyCode::Char('l') | KeyCode::Right => self.next_color(),
                        KeyCode::Char('h') | KeyCode::Left => {
                            self.previous_color();
                        }
                        KeyCode::Enter => self.select_for_deletion(),
                        KeyCode::Char('d') => self.remove_directories(),
                        _ => {}
                    }
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let vertical = &Layout::vertical([Constraint::Min(5), Constraint::Length(4)]);
        let rects = vertical.split(frame.area());

        self.set_colors();

        self.render_table(frame, rects[0]);
        self.render_scrollbar(frame, rects[0]);
        self.render_footer(frame, rects[1]);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default()
            .fg(self.colors.header_fg)
            .bg(self.colors.header_bg);
        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(self.colors.selected_row_style_fg);
        let selected_col_style = Style::default().fg(self.colors.selected_column_style_fg);
        let selected_cell_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(self.colors.selected_cell_style_fg);

        let header = ["Selected", "Name", "Size"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);
        let rows = self.items.iter().enumerate().map(|(i, data)| {
            let color = match i % 2 {
                0 => self.colors.normal_row_color,
                _ => self.colors.alt_row_color,
            };
            let item = data.ref_array();
            item.into_iter()
                .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                .collect::<Row>()
                .style(Style::new().fg(self.colors.row_fg).bg(color))
                .height(4)
        });
        //if self.delete_folder
        let bar = "";
        let t = Table::new(
            rows,
            [
                // + 1 is for padding.
                Constraint::Length(8),
                Constraint::Min(self.longest_item_lens.1 + 1),
                Constraint::Min(1),
            ],
        )
        .header(header)
        .row_highlight_style(selected_row_style)
        .column_highlight_style(selected_col_style)
        .cell_highlight_style(selected_cell_style)
        .highlight_symbol(Text::from(vec![
            "".into(),
            bar.into(),
            bar.into(),
            "".into(),
        ]))
        .bg(self.colors.buffer_bg)
        .highlight_spacing(HighlightSpacing::Always);
        frame.render_stateful_widget(t, area, &mut self.state);
    }

    fn render_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.scroll_state,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let info_footer = Paragraph::new(Text::from_iter(INFO_TEXT))
            .style(
                Style::new()
                    .fg(self.colors.row_fg)
                    .bg(self.colors.buffer_bg),
            )
            .centered()
            .block(
                Block::bordered()
                    .border_type(BorderType::Double)
                    .border_style(Style::new().fg(self.colors.footer_border_color)),
            );
        frame.render_widget(info_footer, area);
    }
}

fn generate_data() -> Vec<Data> {
    get_array()
        .into_par_iter()
        .filter_map(|mut i| {
            let name = i.clone().to_string();
            let name_len = i.len();
            let parent_len = name_len - 13;
            while i.len() != parent_len {
                i.pop();
            }
            //let string_offset = i;
            if i.contains("node_modules")
                || i.contains(".cache")
                || i.contains(".vscode")
                || i.contains(".local")
                || i.contains(".npm")
                || i.contains(".nvm")
                || i.contains(".steam")
                || i.contains(".var")
                || i.contains(".cargo")
                || i.contains("/caches/")
                || i.contains("/Caches/")
            {
                return None;
            }
            let parent = get_size_in_abbr_human_bytes(&Path::new(&i)).expect("REASON");
            Some(Data {
                name,
                parent_node_module: parent,
                selected_for_deletion: String::from("  ☐"),
            })
        })
        .collect()
}

fn constraint_len_calculator(items: &[Data]) -> (u16, u16, u16) {
    let name_len = items
        .par_iter()
        .map(Data::name)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let parent_len = items
        .par_iter()
        .map(Data::parent)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let selected_len = items
        .par_iter()
        .map(Data::select)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);

    #[allow(clippy::cast_possible_truncation)]
    (selected_len as u16, name_len as u16, parent_len as u16)
}

fn get_array() -> Vec<String> {
    let mut node_modules: Vec<String> = Vec::new();
    println!("Loading...");
    for entry in WalkDir::new(my_home().unwrap().unwrap())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() && entry.path().ends_with("node_modules/") {
            //println!("{}  ", entry.path().display());
            node_modules.push(entry.path().to_str().unwrap_or("").to_string())
        }
    }
    node_modules
}

#[cfg(test)]
mod tests {
    use crate::Data;

    #[test]
    fn constraint_len_calculator() {
        let test_data = vec![
            Data {
                name: "Emirhan Tala".to_string(),
                parent_node_module: "Cambridgelaan 6XX\n3584 XX Utrecht".to_string(),
                selected_for_deletion: "true".to_string(),
            },
            Data {
                name: "thistextis26characterslong".to_string(),
                parent_node_module:
                    "this line is 31 characters long\nbottom line is 33 characters long".to_string(),
                selected_for_deletion: "true".to_string(),
            },
        ];
        let (longest_name_len, longest_address_len, _longest_selection_len) =
            crate::constraint_len_calculator(&test_data);

        assert_eq!(26, longest_name_len);
        assert_eq!(33, longest_address_len);
    }
}
