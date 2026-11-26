//! Disk overview within the existing PR58 central pane.
use super::*;

impl DesktopApp {
    pub(super) fn catalog_overview_table(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let entries = self
            .volumes
            .iter()
            .fold(0_u64, |sum, v| sum.saturating_add(v.entry_count));
        ui.heading(self.t("Диски каталога", "Catalog disks"));
        ui.label(format!(
            "{}: {} · {}: {}",
            self.t("Диски", "Disks"),
            self.volumes.len(),
            self.t("Записей", "Entries"),
            entries
        ));
        ui.add_space(6.0);
        let mut rows = self.volumes.clone();
        sort_register(&mut rows, self.register_order);
        let mut open_volume = None;
        TableBuilder::new(ui)
            .id_salt("catalog_disk_overview")
            .striped(true)
            .sense(egui::Sense::click())
            .resizable(true)
            .column(Column::initial(54.0).at_least(44.0))
            .column(Column::remainder().at_least(120.0))
            .column(Column::initial(100.0).at_least(75.0))
            .column(Column::initial(150.0).at_least(110.0))
            .header(26.0, |mut header| {
                for label in [
                    "#",
                    self.t("Диск каталога", "Catalog disk"),
                    self.t("Записей", "Entries"),
                    self.t("Объём индекса", "Indexed size"),
                ] {
                    header.col(|ui| {
                        ui.strong(label);
                    });
                }
            })
            .body(|body| {
                body.rows(26.0, rows.len(), |mut row| {
                    let volume = &rows[row.index()];
                    row.col(|ui| {
                        ui.label(format!("#{}", volume.id));
                    });
                    row.col(|ui| {
                        let label = self.display_name(volume);
                        if ui
                            .add(egui::Button::selectable(false, label).truncate())
                            .on_hover_text(label)
                            .clicked()
                        {
                            open_volume = Some(volume.id);
                        }
                    });
                    row.col(|ui| {
                        ui.label(volume.entry_count.to_string());
                    });
                    row.col(|ui| {
                        ui.label(bytes_compact(volume.total_bytes))
                            .on_hover_text(self.t(
                            "Логический объём записанных метаданных, не физическая ёмкость диска",
                            "Logical indexed size, not physical disk capacity",
                        ));
                    });
                    if row.response().clicked() {
                        open_volume = Some(volume.id);
                    }
                });
            });
        if let Some(volume_id) = open_volume {
            self.browse_volume_root(volume_id, ctx);
        }
    }
}
