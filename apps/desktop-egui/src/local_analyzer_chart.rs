//! PR58 four-ring geometry; omitted children never enlarge visible sectors.
use eframe::egui::{self, Color32};
use serde_json::Value;
use std::collections::HashMap;

struct Segment {
    id: String,
    kind: String,
    name: String,
    bytes: u64,
    depth: usize,
    start: f64,
    end: f64,
    family: usize,
}

fn segments(nodes: &[Value], root: &str, root_bytes: u64) -> Vec<Segment> {
    let mut children: HashMap<&str, Vec<&Value>> = HashMap::new();
    for node in nodes {
        children
            .entry(node["parent_id"].as_str().unwrap_or(""))
            .or_default()
            .push(node);
    }
    for group in children.values_mut() {
        group.sort_by(|a, b| {
            b["bytes"]
                .as_u64()
                .cmp(&a["bytes"].as_u64())
                .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
        });
    }
    let start = -std::f64::consts::FRAC_PI_2;
    let end = start + std::f64::consts::TAU;
    let mut pending = vec![(root.to_owned(), root_bytes, 0usize, start, end, 0usize)];
    let mut out = Vec::new();
    while let Some((parent, total, depth, start, end, family)) = pending.pop() {
        if depth >= 4 || total == 0 {
            continue;
        }
        let mut angle = start;
        if let Some(group) = children.get(parent.as_str()) {
            for (index, node) in group.iter().enumerate() {
                let bytes = node["bytes"].as_u64().unwrap_or(0);
                if bytes == 0 {
                    continue;
                }
                let next = (angle + (end - start) * (bytes as f64 / total as f64)).min(end);
                let id = node["id"].as_str().unwrap_or("").to_owned();
                let kind = node["kind"].as_str().unwrap_or("").to_owned();
                let family = if depth == 0 { index % 8 } else { family };
                if kind == "folder" {
                    pending.push((id.clone(), bytes, depth + 1, angle, next, family));
                }
                out.push(Segment {
                    id,
                    kind,
                    name: node["name"].as_str().unwrap_or("").to_owned(),
                    bytes,
                    depth,
                    start: angle,
                    end: next,
                    family,
                });
                angle = next;
            }
        }
        if angle < end - 1e-9 {
            let visible: u64 = children
                .get(parent.as_str())
                .into_iter()
                .flatten()
                .map(|n| n["bytes"].as_u64().unwrap_or(0))
                .sum();
            out.push(Segment {
                id: String::new(),
                kind: "omitted".into(),
                name: String::new(),
                bytes: total.saturating_sub(visible),
                depth,
                start: angle,
                end,
                family,
            });
        }
    }
    out
}

const PALETTE: [[u32; 4]; 8] = [
    [0x2563eb, 0x3b82f6, 0x60a5fa, 0x93c5fd],
    [0x0d9488, 0x14b8a6, 0x2dd4bf, 0x5eead4],
    [0xd97706, 0xf59e0b, 0xfbbf24, 0xfcd34d],
    [0x7c3aed, 0x8b5cf6, 0xa78bfa, 0xc4b5fd],
    [0xe11d48, 0xf43f5e, 0xfb7185, 0xfda4af],
    [0x0284c7, 0x0ea5e9, 0x38bdf8, 0x7dd3fc],
    [0x059669, 0x10b981, 0x34d399, 0x6ee7b7],
    [0x4f46e5, 0x6366f1, 0x818cf8, 0xa5b4fc],
];

pub(super) fn draw_tree_ring(
    ui: &mut egui::Ui,
    nodes: &[Value],
    breadcrumbs: &[Value],
    height: f32,
    totals: &Value,
    ru: bool,
) -> Option<(String, String)> {
    let width = ui.available_width().clamp(240.0, 480.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height.max(192.0)), egui::Sense::click());
    let center = rect.center();
    let outer = (rect.width().min(rect.height()) * 0.47).max(96.0);
    let inner = outer * 0.32;
    let ring = (outer - inner) / 4.0;
    let root = breadcrumbs
        .last()
        .and_then(|n| n["id"].as_str())
        .unwrap_or("folder:1");
    let bytes = totals["logical_bytes"].as_u64().unwrap_or(0);
    let sectors = segments(nodes, root, bytes);
    let painter = ui.painter_at(rect);
    for sector in &sectors {
        let color = if sector.kind == "omitted" {
            Color32::from_gray(67)
        } else {
            let hex = PALETTE[sector.family][sector.depth];
            Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
        };
        let r0 = inner + sector.depth as f32 * ring;
        super::draw_arc(
            &painter,
            center,
            r0 + ring - 1.5,
            r0,
            sector.start,
            sector.end,
            color,
        );
        if sector.end - sector.start < std::f64::consts::TAU - 1e-6 {
            for angle in [sector.start, sector.end] {
                let direction = egui::vec2(angle.cos() as f32, angle.sin() as f32);
                painter.line_segment(
                    [
                        center + direction * r0,
                        center + direction * (r0 + ring - 1.5),
                    ],
                    egui::Stroke::new(0.75, ui.visuals().panel_fill),
                );
            }
        }
    }
    let name = breadcrumbs
        .last()
        .and_then(|n| n["name"].as_str())
        .unwrap_or(if ru { "Папка" } else { "Folder" });
    painter.circle_filled(center, inner - 2.0, ui.visuals().panel_fill);
    let text = format!(
        "{}\n{}\n{} {}",
        name,
        super::format_bytes(bytes),
        totals["files"].as_u64().unwrap_or(0),
        if ru { "файл." } else { "files" }
    );
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(12.0),
        ui.visuals().text_color(),
    );
    if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
        let delta = pos - center;
        let radius = delta.length();
        if radius < inner && response.clicked() && breadcrumbs.len() > 1 {
            return breadcrumbs[breadcrumbs.len() - 2]["id"]
                .as_str()
                .map(|id| (id.to_owned(), "folder".into()));
        }
        if radius >= inner && radius <= outer {
            let depth = ((radius - inner) / ring).floor() as usize;
            let mut angle = (delta.y as f64).atan2(delta.x as f64);
            if angle < -std::f64::consts::FRAC_PI_2 {
                angle += std::f64::consts::TAU;
            }
            if let Some(sector) = sectors
                .iter()
                .find(|s| s.depth == depth && angle >= s.start && angle < s.end)
            {
                let name = if sector.kind == "omitted" {
                    if ru {
                        "Не показано"
                    } else {
                        "Not shown"
                    }
                } else {
                    &sector.name
                };
                response
                    .clone()
                    .on_hover_text(format!("{name}\n{}", super::format_bytes(sector.bytes)));
                if response.clicked() && sector.kind != "omitted" {
                    return Some((sector.id.clone(), sector.kind.clone()));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn omitted_children_keep_their_angular_share() {
        let nodes = vec![json!({"id":"file:1","parent_id":"folder:1","kind":"file","bytes":25})];
        let arcs = segments(&nodes, "folder:1", 100);
        assert_eq!(arcs.len(), 2);
        assert!((arcs[0].end - arcs[0].start - std::f64::consts::TAU * 0.25).abs() < 1e-9);
        assert_eq!(arcs[1].kind, "omitted");
        assert_eq!(arcs[1].bytes, 75);
    }
    #[test]
    fn children_extend_outward_without_inventing_file_descendants() {
        let nodes = vec![
            json!({"id":"folder:2","parent_id":"folder:1","kind":"folder","bytes":100}),
            json!({"id":"file:1","parent_id":"folder:2","kind":"file","bytes":25}),
        ];
        let arcs = segments(&nodes, "folder:1", 100);
        assert_eq!(arcs.iter().find(|a| a.id == "folder:2").unwrap().depth, 0);
        assert_eq!(arcs.iter().find(|a| a.id == "file:1").unwrap().depth, 1);
        assert_eq!(arcs.iter().filter(|a| a.kind == "omitted").count(), 1);
        assert!(arcs.iter().all(|a| a.depth < 2));
    }
}
