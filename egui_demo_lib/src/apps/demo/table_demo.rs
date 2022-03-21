use egui::{Label, Vec2};
use egui_extras::{Size, StripBuilder, TableBuilder};

/// Shows off a table with dynamic layout
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Default)]
pub struct TableDemo {
    virtual_scrool: bool,
}

impl super::Demo for TableDemo {
    fn name(&self) -> &'static str {
        "☰ Table Demo"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .open(open)
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                use super::View as _;
                self.ui(ui);
            });
    }
}

impl super::View for TableDemo {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.virtual_scrool, "Virtual scroll demo");

        // The table is inside a grid as its container would otherwise grow slowly as it takes all available height
        ui.spacing_mut().item_spacing = Vec2::splat(4.0);
        StripBuilder::new(ui)
        .size(Size::Remainder)
        .size(Size::Absolute(10.0))
        .vertical(|mut grid| {
            grid.cell_clip(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(3.0);

                TableBuilder::new(ui)
                .striped(true)
                .column(Size::Absolute(120.0))
                .column(Size::RemainderMinimum(180.0))
                .column(Size::Absolute(100.0))
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.heading("Left");
                    });
                    header.col(|ui| {
                        ui.heading("Middle");
                    });
                    header.col(|ui| {
                        ui.heading("Right");
                    });
                })
                .body(|mut body| {
                    if self.virtual_scrool {
                        body.rows(20.0, 100_000, |index, mut row| {
                            row.col(|ui| {
                                ui.label(index.to_string());
                            });
                            row.col_clip(|ui| {
                                ui.add(
                                    Label::new("virtual scroll, easily with thousands of rows!")
                                        .wrap(false),
                                );
                            });
                            row.col(|ui| {
                                ui.label(index.to_string());
                            });
                        });
                    } else {
                        for i in 0..100 {
                            let height = match i % 8 {
                                0 => 25.0,
                                4 => 30.0,
                                _ => 20.0,
                            };
                            body.row(height, None, |mut row| {
                                row.col(|ui| {
                                    ui.label(i.to_string());
                                });
                                row.col_clip(|ui| {
                                    ui.add(
                                        Label::new(
                                            format!("Normal scroll, each row can have a different height. Height: {}", height),
                                        )
                                        .wrap(false),
                                    );
                                });
                                row.col(|ui| {
                                    ui.label(i.to_string());
                                });
                            });
                        }
                    }
                });
            });
            grid.cell(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add(crate::__egui_github_link_file!());
                });
            });
        });
    }
}
