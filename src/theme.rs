use eframe::egui::{
    self, Color32, ColorImage, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect,
    Rounding, Sense, Stroke, TextureHandle, TextureOptions, Vec2, Visuals,
};
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub dark: bool,
    pub base: Color32,
    pub surface: Color32,
    pub panel: Color32,
    pub panel_hover: Color32,
    pub border: Color32,
    pub border_bright: Color32,
    pub accent: Color32,
    pub primary: Color32,
    pub primary_hover: Color32,
    pub primary_text: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub green: Color32,
    pub amber: Color32,
    pub red: Color32,
}

impl ThemePalette {
    pub fn get(dark: bool) -> Self {
        if dark {
            // Modern Obsidian & Cyan/Azure palette
            Self {
                dark: true,
                base: Color32::from_rgb(15, 17, 23),
                surface: Color32::from_rgb(22, 27, 34),
                panel: Color32::from_rgb(30, 36, 46),
                panel_hover: Color32::from_rgb(40, 48, 62),
                border: Color32::from_rgb(48, 54, 61),
                border_bright: Color32::from_rgb(75, 85, 102),
                accent: Color32::from_rgb(56, 189, 248),
                primary: Color32::from_rgb(37, 99, 235),
                primary_hover: Color32::from_rgb(59, 130, 246),
                primary_text: Color32::WHITE,
                text: Color32::from_rgb(240, 243, 246),
                muted: Color32::from_rgb(139, 148, 158),
                green: Color32::from_rgb(52, 211, 153),
                amber: Color32::from_rgb(251, 191, 36),
                red: Color32::from_rgb(248, 113, 113),
            }
        } else {
            // Soft Cool Slate Studio Light palette (low-glare, high legibility)
            Self {
                dark: false,
                base: Color32::from_rgb(218, 226, 237),
                surface: Color32::from_rgb(235, 240, 247),
                panel: Color32::from_rgb(209, 220, 234),
                panel_hover: Color32::from_rgb(191, 206, 226),
                border: Color32::from_rgb(165, 181, 202),
                border_bright: Color32::from_rgb(17, 118, 224),
                accent: Color32::from_rgb(12, 108, 214),
                primary: Color32::from_rgb(17, 118, 224),
                primary_hover: Color32::from_rgb(9, 96, 194),
                primary_text: Color32::WHITE,
                text: Color32::from_rgb(24, 30, 40),
                muted: Color32::from_rgb(84, 98, 118),
                green: Color32::from_rgb(21, 128, 61),
                amber: Color32::from_rgb(154, 103, 0),
                red: Color32::from_rgb(190, 28, 40),
            }
        }
    }
}

pub fn apply_theme(ctx: &egui::Context, dark: bool) -> ThemePalette {
    let p = ThemePalette::get(dark);
    let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };

    let control_fill = if dark {
        Color32::from_rgb(38, 46, 60)
    } else {
        p.panel
    };
    let control_border = if dark {
        Color32::from_rgb(76, 89, 110)
    } else {
        Color32::from_rgb(152, 170, 194)
    };
    let input_well = if dark {
        Color32::from_rgb(20, 25, 33)
    } else {
        Color32::from_rgb(225, 233, 243)
    };

    visuals.window_fill = p.surface;
    visuals.panel_fill = p.base;
    visuals.faint_bg_color = Color32::from_rgba_unmultiplied(p.panel.r(), p.panel.g(), p.panel.b(), 90);
    visuals.extreme_bg_color = input_well;
    visuals.code_bg_color = p.panel;
    visuals.override_text_color = Some(p.text);
    visuals.slider_trailing_fill = true;
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(
        p.accent.r(),
        p.accent.g(),
        p.accent.b(),
        if dark { 125 } else { 115 },
    );
    visuals.selection.stroke = Stroke::new(1.0_f32, p.accent);
    visuals.window_rounding = Rounding::same(6.0);
    visuals.window_stroke = Stroke::new(1.0_f32, p.border);

    let rounding = Rounding::same(5.0);
    visuals.widgets.noninteractive.bg_fill = p.surface;
    visuals.widgets.noninteractive.weak_bg_fill = p.surface;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
    visuals.widgets.noninteractive.rounding = rounding;

    visuals.widgets.inactive.bg_fill = control_fill;
    visuals.widgets.inactive.weak_bg_fill = control_fill;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, control_border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.2_f32, p.text);
    visuals.widgets.inactive.rounding = rounding;

    visuals.widgets.hovered.bg_fill = p.panel_hover;
    visuals.widgets.hovered.weak_bg_fill = p.panel_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, p.text);
    visuals.widgets.hovered.rounding = rounding;

    visuals.widgets.active.bg_fill = p.border_bright;
    visuals.widgets.active.weak_bg_fill = p.border_bright;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, p.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5_f32, p.text);
    visuals.widgets.active.rounding = rounding;

    visuals.widgets.open.bg_fill = p.panel_hover;
    visuals.widgets.open.weak_bg_fill = p.panel_hover;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, p.accent);
    visuals.widgets.open.rounding = rounding;

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.window_margin = egui::Margin::same(14.0);
    style.spacing.interact_size.y = 24.0;
    ctx.set_style(style);

    p
}

/// Loads Windows system Segoe UI (with Tahoma / Arial fallback) so extended Latin,
/// `™` (U+2122), and `●` (U+25CF) render crisply alongside built-in fallback glyphs.
pub fn configure_fonts(ctx: &egui::Context) {
    let candidates = [
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\tahoma.ttf",
        r"C:\Windows\Fonts\arial.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = fs::read(path) {
            let mut fonts = FontDefinitions::default();
            fonts.font_data.insert(
                "win_ui".to_owned(),
                FontData::from_owned(bytes),
            );
            if let Some(fam) = fonts.families.get_mut(&FontFamily::Proportional) {
                fam.insert(0, "win_ui".to_owned());
            }
            ctx.set_fonts(fonts);
            break;
        }
    }
}

pub fn primary_button(
    ui: &mut egui::Ui,
    label: &str,
    pal: ThemePalette,
    size: Option<Vec2>,
) -> egui::Response {
    let mut btn = egui::Button::new(egui::RichText::new(label).color(pal.primary_text))
        .fill(pal.primary)
        .stroke(Stroke::new(1.0_f32, pal.primary_hover))
        .rounding(Rounding::same(5.0));
    if let Some(sz) = size {
        btn = btn.min_size(sz);
    }
    let resp = ui.add(btn);
    if resp.hovered() && ui.is_enabled() {
        // Lighten slightly via painter overlay for responsive feel
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

pub fn render_lock_icon_button(ui: &mut egui::Ui, pal: ThemePalette) -> bool {
    let size = Vec2::new(34.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let bg = if hovered { pal.panel_hover } else { pal.panel };
    let border = if hovered { pal.border_bright } else { pal.border };
    let icon_col = if hovered { pal.accent } else { pal.text };

    let painter = ui.painter();
    painter.rect(rect, Rounding::same(5.0), bg, Stroke::new(1.0_f32, border));

    let center = rect.center();
    let body_w = 12.0;
    let body_h = 9.0;
    let body_min_y = center.y - 0.5;
    let body_max_y = body_min_y + body_h;

    // 1. Shackle arch (inverted U curving upward)
    let shackle_r = 3.6;
    let arch_center_y = body_min_y - 1.8;
    let mut pts = Vec::with_capacity(19);
    pts.push(Pos2::new(center.x - shackle_r, body_min_y + 2.0));
    for step in 0..=16 {
        let angle = std::f32::consts::PI + (step as f32 / 16.0) * std::f32::consts::PI;
        pts.push(Pos2::new(
            center.x + shackle_r * angle.cos(),
            arch_center_y + shackle_r * angle.sin(),
        ));
    }
    pts.push(Pos2::new(center.x + shackle_r, body_min_y + 2.0));
    painter.add(egui::Shape::line(pts, Stroke::new(1.8_f32, icon_col)));

    // 2. Padlock body
    let body_rect = Rect::from_min_max(
        Pos2::new(center.x - body_w * 0.5, body_min_y),
        Pos2::new(center.x + body_w * 0.5, body_max_y),
    );
    painter.rect_filled(body_rect, Rounding::same(2.0), icon_col);

    // 3. Keyhole cutout
    let keyhole_y = body_min_y + 3.5;
    painter.circle_filled(Pos2::new(center.x, keyhole_y), 1.3, bg);
    painter.line_segment(
        [
            Pos2::new(center.x, keyhole_y),
            Pos2::new(center.x, body_min_y + 6.8),
        ],
        Stroke::new(1.4_f32, bg),
    );

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text("Lock Vault").clicked()
}

pub fn render_theme_icon_button(ui: &mut egui::Ui, dark_theme: bool, pal: ThemePalette) -> bool {
    let size = Vec2::new(34.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let bg = if hovered { pal.panel_hover } else { pal.panel };
    let border = if hovered { pal.border_bright } else { pal.border };
    let icon_col = if hovered { pal.accent } else { pal.text };

    let painter = ui.painter();
    painter.rect(rect, Rounding::same(5.0), bg, Stroke::new(1.0_f32, border));

    let center = rect.center();
    let dome_c = Pos2::new(center.x, center.y - 2.5);
    let bulb_col = if !dark_theme {
        Color32::from_rgba_unmultiplied(255, 215, 60, if hovered { 255 } else { 220 })
    } else if hovered {
        Color32::from_rgb(115, 125, 145)
    } else {
        Color32::from_rgb(70, 78, 94)
    };

    painter.circle_filled(dome_c, 4.4, bulb_col);
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(center.x - 3.2, center.y + 0.5),
            Pos2::new(center.x + 3.2, center.y + 0.5),
            Pos2::new(center.x + 2.0, center.y + 3.6),
            Pos2::new(center.x - 2.0, center.y + 3.6),
        ],
        bulb_col,
        Stroke::NONE,
    ));

    if !dark_theme {
        let ray_col = Color32::from_rgb(255, 195, 30);
        let rays = [
            ([0.0, -8.5], [0.0, -11.0]),
            ([-5.5, -6.5], [-7.5, -8.5]),
            ([5.5, -6.5], [7.5, -8.5]),
            ([-6.5, -2.5], [-9.0, -2.5]),
            ([6.5, -2.5], [9.0, -2.5]),
        ];
        for (a, b) in rays {
            painter.line_segment(
                [
                    Pos2::new(center.x + a[0], center.y + a[1]),
                    Pos2::new(center.x + b[0], center.y + b[1]),
                ],
                Stroke::new(1.5_f32, ray_col),
            );
        }
    }

    // Screw base
    painter.line_segment(
        [
            Pos2::new(center.x - 2.0, center.y + 4.8),
            Pos2::new(center.x + 2.0, center.y + 4.8),
        ],
        Stroke::new(1.5_f32, icon_col),
    );
    painter.line_segment(
        [
            Pos2::new(center.x - 1.4, center.y + 6.2),
            Pos2::new(center.x + 1.4, center.y + 6.2),
        ],
        Stroke::new(1.5_f32, icon_col),
    );

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let tip = if dark_theme {
        "Switch to Light Theme"
    } else {
        "Switch to Dark Theme"
    };
    resp.on_hover_text(tip).clicked()
}

pub const SEND_ICON_SVG: &[u8] = include_bytes!("../assets/send.svg");
pub const EDIT_ICON_SVG: &[u8] = include_bytes!("../assets/edit.svg");
pub const DELETE_ICON_SVG: &[u8] = include_bytes!("../assets/delete.svg");

/// Rasterizes a standalone SVG icon into unpremultiplied RGBA [`egui::IconData`].
pub fn rasterize_app_icon_svg(svg_bytes: &[u8], target_px: u32) -> Option<egui::IconData> {
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_bytes, &opts).ok()?;
    let svg_size = tree.size();
    let px = target_px.clamp(16, 1024);
    let scale = (px as f32 / svg_size.width().max(1.0))
        .min(px as f32 / svg_size.height().max(1.0));

    let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();

    Some(egui::IconData {
        rgba,
        width: px,
        height: px,
    })
}

#[derive(Default)]
pub struct SvgIconCache {
    textures: HashMap<(&'static str, u32), TextureHandle>,
}

impl SvgIconCache {
    pub fn icon_texture(
        &mut self,
        ctx: &egui::Context,
        id: &'static str,
        svg_bytes: &[u8],
        size_pt: f32,
    ) -> Option<&TextureHandle> {
        let ppp = ctx.pixels_per_point().clamp(1.0, 4.0);
        let px = (size_pt * ppp).round().clamp(8.0, 256.0) as u32;
        let key = (id, px);

        if let std::collections::hash_map::Entry::Vacant(e) = self.textures.entry(key) {
            let opts = usvg::Options::default();
            let tree = usvg::Tree::from_data(svg_bytes, &opts).ok()?;
            let svg_size = tree.size();
            let scale = ((px as f32) / svg_size.width().max(1.0))
                .min((px as f32) / svg_size.height().max(1.0));
            let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
            resvg::render(
                &tree,
                tiny_skia::Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
            let image =
                ColorImage::from_rgba_premultiplied([px as usize, px as usize], &pixmap.take());
            let handle =
                ctx.load_texture(format!("orlogin_svg_{id}_{px}"), image, TextureOptions::NEAREST);
            e.insert(handle);
        }

        self.textures.get(&key)
    }
}

/// Renders a semantic-colored action button (`Send` = green, `Edit` = blue, `Delete` = red)
/// with a crisp `resvg`-rasterized SVG icon + label centered inside.
#[allow(clippy::too_many_arguments)]
pub fn svg_action_button(
    ui: &mut egui::Ui,
    icons: &mut SvgIconCache,
    icon_id: &'static str,
    svg_bytes: &[u8],
    label: &str,
    accent: Color32,
    pal: ThemePalette,
    size: Vec2,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let hovered = enabled && resp.hovered();

    let (fill_a, border_mix, fg_color) = if !enabled {
        let dimmed = Color32::from_rgba_unmultiplied(pal.muted.r(), pal.muted.g(), pal.muted.b(), 120);
        (10u8, 0.25_f32, dimmed)
    } else if pal.dark {
        if hovered {
            (56u8, 0.88_f32, accent)
        } else {
            (26u8, 0.55_f32, accent)
        }
    } else if hovered {
        (42u8, 0.88_f32, accent)
    } else {
        (20u8, 0.60_f32, accent)
    };
    let tint_src = if enabled { accent } else { pal.muted };
    let fill = Color32::from_rgba_unmultiplied(tint_src.r(), tint_src.g(), tint_src.b(), fill_a);
    let lerp_u8 = |a: u8, b: u8, t: f32| -> u8 { ((a as f32) + ((b as f32) - (a as f32)) * t).round().clamp(0.0, 255.0) as u8 };
    let border = Color32::from_rgb(
        lerp_u8(pal.surface.r(), tint_src.r(), border_mix),
        lerp_u8(pal.surface.g(), tint_src.g(), border_mix),
        lerp_u8(pal.surface.b(), tint_src.b(), border_mix),
    );

    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point().max(1.0);
    let snap = |v: f32| (v * ppp).round() / ppp;
    let t = 1.0_f32 / ppp; // exact 1 physical pixel thickness on any DPI scale
    let snapped = Rect::from_min_max(
        Pos2::new(snap(rect.min.x), snap(rect.min.y)),
        Pos2::new(snap(rect.max.x), snap(rect.max.y)),
    );

    let painter = ui.painter();
    let inner = Rect::from_min_max(
        Pos2::new(snapped.min.x + t, snapped.min.y + t),
        Pos2::new(snapped.max.x - t, snapped.max.y - t),
    );
    painter.rect_filled(inner, Rounding::ZERO, fill);
    // Draw 4 non-overlapping pixel-snapped 1px border bars so all sides have identical width & color.
    painter.rect_filled(
        Rect::from_min_max(snapped.min, Pos2::new(snapped.max.x, snapped.min.y + t)),
        Rounding::ZERO,
        border,
    );
    painter.rect_filled(
        Rect::from_min_max(Pos2::new(snapped.min.x, snapped.max.y - t), snapped.max),
        Rounding::ZERO,
        border,
    );
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(snapped.min.x, snapped.min.y + t),
            Pos2::new(snapped.min.x + t, snapped.max.y - t),
        ),
        Rounding::ZERO,
        border,
    );
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(snapped.max.x - t, snapped.min.y + t),
            Pos2::new(snapped.max.x, snapped.max.y - t),
        ),
        Rounding::ZERO,
        border,
    );

    let icon_pt = 13.0_f32;
    let gap = 4.0_f32;
    let font_id = FontId::proportional(13.0);
    let galley = painter.layout_no_wrap(label.to_owned(), font_id, fg_color);
    let total_w = icon_pt + gap + galley.size().x;
    let start_x = snapped.center().x - total_w * 0.5;

    if let Some(tex) = icons.icon_texture(&ctx, icon_id, svg_bytes, icon_pt) {
        let icon_y = snap(snapped.center().y - icon_pt * 0.5);
        let icon_x = snap(start_x);
        let icon_rect = Rect::from_min_size(Pos2::new(icon_x, icon_y), Vec2::splat(icon_pt));
        painter.image(
            tex.id(),
            icon_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            fg_color,
        );
    }

    let text_pos = Pos2::new(
        start_x + icon_pt + gap,
        snapped.center().y - galley.size().y * 0.5,
    );
    painter.galley(text_pos, galley, fg_color);

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}
