//! Le visage d'Emacs, faces par défaut d'un `emacs -Q` : le tampon blanc,
//! une seule fonte à chasse fixe, la barre d'onglets et la ligne de mode
//! en gris, la zone d'écho dessous. Les champs sont ceux de Customize
//! (`widget-field`), les boutons ses `custom-button` en relief, les choix
//! des boutons radio `(*)`. Les couleurs portent les noms X11 d'Emacs.

use eframe::egui::{
    self, text::LayoutJob, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Painter,
    Pos2, Rect, Response, RichText, Sense, Stroke, TextFormat, Ui, Vec2,
};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// `default`.
pub const BG: Color32 = rgb(0xffffff);
pub const FG: Color32 = rgb(0x000000);
/// `shadow` : grey50.
pub const SHADOW: Color32 = rgb(0x7f7f7f);
/// `mode-line` : grey75 ; `header-line` : grey90.
pub const MODE_LINE: Color32 = rgb(0xbfbfbf);
pub const GREY90: Color32 = rgb(0xe5e5e5);
pub const GREY20: Color32 = rgb(0x333333);
/// `tab-bar`, `widget-field` : grey85.
pub const GREY85: Color32 = rgb(0xd9d9d9);
/// `custom-button` : lightgrey.
pub const BUTTON: Color32 = rgb(0xd3d3d3);
/// `highlight` (hl-line, mouse-face) : darkseagreen2.
pub const HIGHLIGHT: Color32 = rgb(0xb4eeb4);
/// `region` : lightgoldenrod2.
pub const REGION: Color32 = rgb(0xeedc82);
/// `link` : RoyalBlue3.
pub const LINK: Color32 = rgb(0x3a5fcd);
/// `custom-group-tag` : Blue1.
pub const HEADING: Color32 = rgb(0x0000ff);
/// `minibuffer-prompt` : medium blue.
pub const PROMPT: Color32 = rgb(0x0000cd);
/// `error` : Red1.
pub const ERROR: Color32 = rgb(0xff0000);
/// Les barres : des noms X11 sobres, D1 SteelBlue, D2 sienna.
pub const D1: Color32 = rgb(0x4682b4);
pub const D2: Color32 = rgb(0xa0522d);
pub const SERIES: [Color32; 7] = [
    rgb(0x4682b4), // SteelBlue
    rgb(0xa0522d), // sienna
    rgb(0x228b22), // ForestGreen
    rgb(0x9932cc), // DarkOrchid
    rgb(0x008b8b), // DarkCyan
    rgb(0xb22222), // firebrick
    rgb(0x696969), // DimGray
];

/// La taille du corps : DejaVu Sans Mono 10 pt.
pub const SIZE: f32 = 13.5;

pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn mono() -> FontId {
    FontId::monospace(SIZE)
}

pub fn bold_font() -> FontId {
    FontId::new(SIZE, bold_family())
}

pub fn apply(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "mono".into(),
        FontData::from_static(include_bytes!("../fonts/DejaVuSansMono.ttf")),
    );
    fonts.font_data.insert(
        "mono-bold".into(),
        FontData::from_static(include_bytes!("../fonts/DejaVuSansMono-Bold.ttf")),
    );
    // Tout est à chasse fixe ; les fontes d'egui ne servent qu'en repli.
    for fam in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(fam)
            .or_default()
            .insert(0, "mono".into());
    }
    let mut bold = vec!["mono-bold".to_owned()];
    bold.extend(fonts.families[&FontFamily::Monospace].iter().cloned());
    fonts.families.insert(bold_family(), bold);
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::light();
    v.panel_fill = BG;
    v.window_fill = GREY90;
    v.extreme_bg_color = GREY85;
    v.faint_bg_color = GREY90;
    v.override_text_color = Some(FG);
    v.hyperlink_color = LINK;
    v.window_rounding = 0.0.into();
    v.menu_rounding = 0.0.into();
    v.window_stroke = Stroke::new(1.0_f32, SHADOW);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.selection.bg_fill = REGION;
    v.selection.stroke = Stroke::new(1.0_f32, FG);
    v.text_cursor.stroke = Stroke::new(2.0_f32, FG);
    v.text_cursor.blink = false;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = 0.0.into();
        w.bg_fill = GREY85;
        w.weak_bg_fill = GREY85;
        w.bg_stroke = Stroke::NONE;
        w.fg_stroke = Stroke::new(1.0_f32, FG);
        w.expansion = 0.0;
    }
    v.widgets.hovered.weak_bg_fill = HIGHLIGHT;
    v.widgets.open.weak_bg_fill = HIGHLIGHT;
    v.widgets.active.weak_bg_fill = HIGHLIGHT;
    // Les ascenseurs : le pouce gris sur le fond grey85.
    v.widgets.inactive.bg_fill = MODE_LINE;
    v.widgets.hovered.bg_fill = SHADOW;
    v.widgets.active.bg_fill = GREY20;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(0.0, 2.0);
        s.spacing.button_padding = egui::vec2(6.0, 1.0);
        s.spacing.interact_size.y = 20.0;
        s.spacing.menu_margin = egui::Margin::same(2.0);
        s.spacing.scroll = egui::style::ScrollStyle::solid();
        s.spacing.scroll.bar_width = 12.0;
        s.spacing.scroll.foreground_color = false;
        for (style, font) in [
            (egui::TextStyle::Body, mono()),
            (egui::TextStyle::Button, mono()),
            (egui::TextStyle::Monospace, mono()),
            (egui::TextStyle::Small, mono()),
            (egui::TextStyle::Heading, bold_font()),
        ] {
            s.text_styles.insert(style, font);
        }
    });
}

/// La largeur d'un caractère : toutes les mesures en découlent.
pub fn ch(ui: &Ui) -> f32 {
    ui.fonts(|f| f.glyph_width(&mono(), '0'))
}

/// La hauteur d'une ligne de texte.
pub fn line(ui: &Ui) -> f32 {
    ui.fonts(|f| f.row_height(&mono()))
}

/// La hauteur d'une ligne de tampon : le texte et un peu d'air.
pub fn row_h(ui: &Ui) -> f32 {
    (line(ui) + 6.0).round()
}

/// Une boîte « released-button » (en relief) ou « pressed-button ».
pub fn box3d(p: &Painter, r: Rect, raised: bool, width: f32) {
    let (light, dark) = (rgb(0xf5f5f5), rgb(0x6b6b6b));
    let (tl, br) = if raised { (light, dark) } else { (dark, light) };
    for i in 0..width as i32 {
        let r = r.shrink(i as f32);
        let px = |a: Pos2, b: Pos2, c: Color32| p.rect_filled(Rect::from_min_max(a, b), 0.0, c);
        px(r.min, Pos2::new(r.max.x, r.min.y + 1.0), tl);
        px(r.min, Pos2::new(r.min.x + 1.0, r.max.y), tl);
        px(Pos2::new(r.min.x, r.max.y - 1.0), r.max, br);
        px(Pos2::new(r.max.x - 1.0, r.min.y), r.max, br);
    }
}

fn galley(ui: &Ui, text: &str, font: FontId, color: Color32) -> std::sync::Arc<egui::Galley> {
    ui.painter().layout_no_wrap(text.to_owned(), font, color)
}

/// Un `custom-button` : gris clair, en relief, enfoncé quand on le presse.
pub fn button(ui: &mut Ui, text: &str) -> Response {
    let g = galley(ui, text, mono(), FG);
    let size = Vec2::new(g.size().x + 2.0 * ch(ui), row_h(ui));
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let pressed = resp.is_pointer_button_down_on();
        let enabled = ui.is_enabled();
        let r = rect.shrink(1.0);
        p.rect_filled(
            r,
            0.0,
            if resp.hovered() && enabled {
                GREY90
            } else {
                BUTTON
            },
        );
        box3d(p, r, !pressed, 2.0);
        let color = if enabled { FG } else { SHADOW };
        let shift = if pressed {
            Vec2::splat(1.0)
        } else {
            Vec2::ZERO
        };
        p.galley(r.center() - g.size() / 2.0 + shift, g, color);
        if resp.has_focus() {
            p.rect_stroke(r.shrink(3.0), 0.0, Stroke::new(1.0_f32, GREY20));
        }
    }
    resp
}

/// Un bouton radio d'un widget : `(*) D1` ou `( ) D1`, le survol en
/// `highlight`.
pub fn radio(ui: &mut Ui, text: &str, on: bool) -> Response {
    let said = format!("({}) {text}", if on { '*' } else { ' ' });
    let g = galley(ui, &said, mono(), FG);
    let size = Vec2::new(g.size().x + ch(ui), row_h(ui));
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let tr = Rect::from_min_size(
            Pos2::new(rect.left(), rect.center().y - g.size().y / 2.0),
            g.size(),
        );
        if resp.hovered() {
            p.rect_filled(tr.expand(1.0), 0.0, HIGHLIGHT);
        }
        p.galley(tr.min, g, FG);
    }
    resp
}

/// Un lien (`link`), souligné ; `on`, il est le choix courant.
pub fn link(ui: &mut Ui, text: &str, on: bool) -> Response {
    let g = galley(ui, text, mono(), LINK);
    let size = Vec2::new(g.size().x, row_h(ui));
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let tr = Rect::from_min_size(
            Pos2::new(rect.left(), rect.center().y - g.size().y / 2.0),
            g.size(),
        );
        if resp.hovered() || on {
            p.rect_filled(tr.expand(1.0), 0.0, HIGHLIGHT);
        }
        let y = tr.bottom() - 1.5;
        p.line_segment(
            [Pos2::new(tr.left(), y), Pos2::new(tr.right(), y)],
            Stroke::new(1.0_f32, LINK),
        );
        p.galley(tr.min, g, LINK);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Un champ éditable (`widget-field`) : un fond grey85, sans cadre.
pub fn field(ui: &mut Ui, id: &str, buf: &mut String, width: f32, hint: &str) -> Response {
    let h = row_h(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
    let r = rect.shrink2(Vec2::new(0.0, 1.0));
    ui.painter().rect_filled(r, 0.0, GREY85);
    let inner = r.shrink2(Vec2::new(3.0, 0.0));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add(
        egui::TextEdit::singleline(buf)
            .id(egui::Id::new(id))
            .frame(false)
            .margin(Vec2::ZERO)
            .font(mono())
            .desired_width(inner.width())
            .hint_text(RichText::new(hint).color(SHADOW)),
    )
}

/// Le soulignement ondulé de Flymake : ce champ est obligatoire ou faux.
pub fn squiggle(ui: &Ui, r: Rect) {
    let y = r.bottom() - 3.0;
    let mut pts = Vec::new();
    let mut x = r.left() + 1.0;
    let mut up = true;
    while x <= r.right() - 1.0 {
        pts.push(Pos2::new(x, if up { y - 1.5 } else { y + 0.5 }));
        x += 2.0;
        up = !up;
    }
    ui.painter()
        .add(egui::Shape::line(pts, Stroke::new(1.0_f32, ERROR)));
}

/// Un titre de groupe de Customize, gras, Blue1, et sa précision en
/// `shadow`.
pub fn heading(ui: &mut Ui, text: &str, aside: &str) {
    ui.add_space(4.0);
    let mut job = LayoutJob::default();
    job.append(text, 0.0, TextFormat::simple(bold_font(), HEADING));
    if !aside.is_empty() {
        job.append(aside, ch(ui), TextFormat::simple(mono(), SHADOW));
    }
    ui.label(job);
    ui.add_space(2.0);
}

pub fn shadow(text: impl Into<String>) -> RichText {
    RichText::new(text).color(SHADOW)
}

/// La barre d'onglets (`tab-bar-mode`) : grey85, l'onglet courant en
/// relief, les autres en grey75.
pub fn tab_bar(ui: &mut Ui, current: &mut usize, labels: &[&str]) {
    let h = row_h(ui) + 2.0;
    let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    ui.painter().rect_filled(strip, 0.0, GREY85);
    let c = ch(ui);
    let mut x = strip.left();
    for (i, l) in labels.iter().enumerate() {
        let on = i == *current;
        let g = galley(ui, l, if on { bold_font() } else { mono() }, FG);
        let r = Rect::from_min_size(
            Pos2::new(x, strip.top() + 1.0),
            Vec2::new(g.size().x + 2.0 * c, h - 2.0),
        );
        let resp = ui.interact(r, ui.id().with(("tab", i)), Sense::click());
        let p = ui.painter();
        let fill = if on {
            GREY85
        } else if resp.hovered() {
            HIGHLIGHT
        } else {
            MODE_LINE
        };
        p.rect_filled(r, 0.0, fill);
        box3d(p, r, true, 1.0);
        p.galley(r.center() - g.size() / 2.0, g, FG);
        if resp.clicked() {
            *current = i;
        }
        x = r.right() + 1.0;
    }
}

/// La ligne de mode : grey75, en relief d'un pixel. Les morceaux marqués
/// sont en gras — l'identité du tampon.
pub fn mode_line(ui: &mut Ui, parts: &[(&str, bool)], right: &str) {
    let h = row_h(ui);
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 0.0, MODE_LINE);
    box3d(p, r, true, 1.0);
    let mut job = LayoutJob::default();
    for (t, b) in parts {
        job.append(
            t,
            0.0,
            TextFormat::simple(if *b { bold_font() } else { mono() }, FG),
        );
    }
    let g = ui.fonts(|f| f.layout_job(job));
    p.galley(
        Pos2::new(r.left() + ch(ui), r.center().y - g.size().y / 2.0),
        g,
        FG,
    );
    p.text(
        Pos2::new(r.right() - ch(ui), r.center().y),
        Align2::RIGHT_CENTER,
        right,
        mono(),
        FG,
    );
}

/// Une ligne d'en-tête (`header-line`) : grey90, les noms de colonnes en
/// grey20, chacun à sa largeur.
pub fn header_line(ui: &mut Ui, widths: &[f32], names: &[&str], left: f32) {
    let h = row_h(ui);
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 0.0, GREY90);
    let mut x = r.left() + left;
    for (w, n) in widths.iter().zip(names) {
        p.text(
            Pos2::new(x, r.center().y),
            Align2::LEFT_CENTER,
            *n,
            mono(),
            GREY20,
        );
        x += w;
    }
}
