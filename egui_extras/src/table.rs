//! Table view with (optional) fixed header and scrolling body.
//! Cell widths are precalculated with given size hints so we can have tables like this:
//! | fixed size | all available space/minimum | 30% of available width | fixed size |
//! Takes all available height, so if you want something below the table, put it in a strip.

use crate::{
    layout::{CellDirection, CellSize},
    sizing::Sizing,
    Layout, Size,
};

use egui::{Response, Ui};
use std::cmp;

/// Builder for creating a new [`Table`].
pub struct TableBuilder<'a> {
    ui: &'a mut Ui,
    sizing: Sizing,
    scroll: bool,
    striped: bool,
}

impl<'a> TableBuilder<'a> {
    /// Build a table with (optional) fixed header and scrolling body.
    ///
    /// Cell widths are precalculated with given size hints so we can have tables like this:
    ///
    /// | fixed size | all available space/minimum | 30% of available width | fixed size |
    ///
    /// In contrast to normal egui behavior, columns/rows do *not* grow with its children!
    /// Takes all available height, so if you want something below the table, put it in a strip.
    ///
    /// Rows may optionally specify a background color
    ///
    /// ### Example
    /// ```
    /// # egui::__run_test_ui(|ui| {
    /// use egui_extras::{TableBuilder, Size};
    /// TableBuilder::new(ui)
    ///     .column(Size::RemainderMinimum(100.0))
    ///     .column(Size::Absolute(40.0))
    ///     .header(20.0, |mut header| {
    ///         header.col(|ui| {
    ///             ui.heading("Growing");
    ///         });
    ///         header.col(|ui| {
    ///             ui.heading("Fixed");
    ///         });
    ///     })
    ///     .body(|mut body| {
    ///         body.row(30.0, Some(egui::Color32::from_rgb(255,0,255)), |mut row| {
    ///             row.col(|ui| {
    ///                 ui.label("first row growing cell");
    ///             });
    ///             row.col_clip(|ui| {
    ///                 ui.button("action");
    ///             });
    ///         });
    ///     });
    /// # });
    /// ```
    pub fn new(ui: &'a mut Ui) -> Self {
        let sizing = Sizing::new();

        Self {
            ui,
            sizing,
            scroll: true,
            striped: false,
        }
    }

    /// Enable scrollview in body (default: true)
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }

    /// Enable striped row background (default: false)
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// Add size hint for column
    pub fn column(mut self, width: Size) -> Self {
        self.sizing.add(width);
        self
    }

    /// Add size hint for column `count` times
    pub fn columns(mut self, size: Size, count: usize) -> Self {
        for _ in 0..count {
            self.sizing.add(size);
        }
        self
    }

    /// Create a header row which always stays visible and at the top
    pub fn header(self, height: f32, header: impl FnOnce(TableRow<'_, '_>)) -> Table<'a> {
        let widths = self.sizing.into_lengths(
            self.ui.available_rect_before_wrap().width()
                - self.ui.spacing().item_spacing.x
                - if self.scroll {
                    self.ui.spacing().scroll_bar_width
                } else {
                    0.0
                },
            self.ui.spacing().item_spacing.x,
        );
        let ui = self.ui;
        {
            let mut layout = Layout::new(ui, CellDirection::Horizontal);
            {
                let row = TableRow {
                    layout: &mut layout,
                    widths: widths.clone(),
                    striped: false,
                    height,
                    bg_color: None,
                    clicked: false,
                };
                header(row);
            }
            layout.set_rect();
        }

        Table {
            ui,
            widths,
            scroll: self.scroll,
            striped: self.striped,
        }
    }

    /// Create table body without a header row
    pub fn body<F>(self, body: F)
    where
        F: for<'b> FnOnce(TableBody<'b>),
    {
        let widths = self.sizing.into_lengths(
            self.ui.available_rect_before_wrap().width(),
            self.ui.spacing().item_spacing.x,
        );

        Table {
            ui: self.ui,
            widths,
            scroll: self.scroll,
            striped: self.striped,
        }
        .body(body);
    }
}

/// Table struct which can construct a [`TableBody`].
/// Is created by [`TableBuilder`] by either calling `body` or after creating a header row with `header`.
pub struct Table<'a> {
    ui: &'a mut Ui,
    widths: Vec<f32>,
    scroll: bool,
    striped: bool,
}

impl<'a> Table<'a> {
    /// Create table body after adding a header row
    pub fn body<F>(self, body: F)
    where
        F: for<'b> FnOnce(TableBody<'b>),
    {
        let ui = self.ui;
        let widths = self.widths;
        let striped = self.striped;
        let start_y = ui.available_rect_before_wrap().top();
        let end_y = ui.available_rect_before_wrap().bottom();

        egui::ScrollArea::new([false, self.scroll]).show(ui, move |ui| {
            let layout = Layout::new(ui, CellDirection::Horizontal);

            body(TableBody {
                layout,
                widths,
                striped,
                row_nr: 0,
                start_y,
                end_y,
            });
        });
    }
}

/// The body of a table.
/// Is created by calling `body` on a [`Table`] (after adding a header row) or [`TableBuilder`] (without a header row).
pub struct TableBody<'a> {
    layout: Layout<'a>,
    widths: Vec<f32>,
    striped: bool,
    row_nr: usize,
    start_y: f32,
    end_y: f32,
}

impl<'a> TableBody<'a> {
    /// Add rows with same height.
    ///
    /// Is a lot more performant than adding each individual row as non visible rows must not be rendered
    pub fn rows(mut self, height: f32, rows: usize, mut row: impl FnMut(usize, TableRow<'_, '_>)) {
        let delta = self.layout.current_y() - self.start_y;
        let mut start = 0;

        if delta < 0.0 {
            start = (-delta / height).floor() as usize;

            let skip_height = start as f32 * height;
            TableRow {
                layout: &mut self.layout,
                widths: self.widths.clone(),
                striped: false,
                bg_color: None,
                height: skip_height,
                clicked: false,
            }
            .col(|_| ()); // advances the cursor
        }

        let max_height = self.end_y - self.start_y;
        let count = (max_height / height).ceil() as usize;
        let end = cmp::min(start + count, rows);

        for idx in start..end {
            row(
                idx,
                TableRow {
                    layout: &mut self.layout,
                    widths: self.widths.clone(),
                    striped: self.striped && idx % 2 == 0,
                    bg_color: None,
                    height,
                    clicked: false,
                },
            );
        }

        if rows - end > 0 {
            let skip_height = (rows - end) as f32 * height;

            TableRow {
                layout: &mut self.layout,
                widths: self.widths.clone(),
                striped: false,
                bg_color: None,
                height: skip_height,
                clicked: false,
            }
            .col(|_| ()); // advances the cursor
        }
    }

    /// Add row with individual height
    pub fn row(
        &mut self,
        height: f32,
        bg_color: Option<egui::Color32>,
        row: impl FnOnce(TableRow<'a, '_>),
    ) {
        row(TableRow {
            layout: &mut self.layout,
            widths: self.widths.clone(),
            striped: self.striped && self.row_nr % 2 == 0,
            bg_color,
            height,
            clicked: false,
        });

        self.row_nr += 1;
    }
}

impl<'a> Drop for TableBody<'a> {
    fn drop(&mut self) {
        self.layout.set_rect();
    }
}

/// The row of a table.
/// Is created by [`TableRow`] for each created [`TableBody::row`] or each visible row in rows created by calling [`TableBody::rows`].
pub struct TableRow<'a, 'b> {
    layout: &'b mut Layout<'a>,
    widths: Vec<f32>,
    striped: bool,
    bg_color: Option<egui::Color32>,
    height: f32,
    clicked: bool,
}

impl<'a, 'b> TableRow<'a, 'b> {
    /// Check if row was clicked
    pub fn clicked(&self) -> bool {
        self.clicked
    }

    fn _col(&mut self, clip: bool, add_contents: impl FnOnce(&mut Ui)) -> Response {
        assert!(
            !self.widths.is_empty(),
            "Tried using more table columns then available."
        );

        let width = CellSize::Absolute(self.widths.remove(0));
        let height = CellSize::Absolute(self.height);

        let response;

        if self.bg_color.is_none() {
            if self.striped {
                response = self.layout.add_striped(width, height, clip, add_contents);
            } else {
                response = self.layout.add(width, height, clip, add_contents);
            }
        } else {
            response =
                self.layout
                    .add_colored(width, height, clip, self.bg_color.unwrap(), add_contents);
        }

        if response.clicked() {
            self.clicked = true;
        }

        response
    }

    /// Add column, content is wrapped
    pub fn col(&mut self, add_contents: impl FnOnce(&mut Ui)) -> Response {
        self._col(false, add_contents)
    }

    /// Add column, content is clipped
    pub fn col_clip(&mut self, add_contents: impl FnOnce(&mut Ui)) -> Response {
        self._col(true, add_contents)
    }
}

impl<'a, 'b> Drop for TableRow<'a, 'b> {
    fn drop(&mut self) {
        self.layout.end_line();
    }
}
