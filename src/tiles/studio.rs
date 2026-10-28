//! The `studio` and `studio-news` screens: the look designed in Trofeo Studio.
//!
//! * `layout = studio` - a glass clock card (left), a big glass weather card (right) and a
//!   gradient spectrum that sits flush with the bottom edge of the panel (it ignores the
//!   margins on purpose).
//! * `layout = studio-news` - one big glass card with one headline of `ticker_source` at a
//!   time, a dot per card, changing every `tiles_news_seconds` (default 10), over the same
//!   spectrum.
//!
//! Colors and opacity come from the `tiles_*` keys (`tiles_color`, `tiles_opacity`,
//! `tiles_text_color`, `tiles_radius`, `tiles_gap`), the spectrum colors from
//! `spectrum_gradient`, the background from the global `background` (a video or a picture).
//! Both screens keep running while music plays, so the bars move.

use super::*;

/// Which of the two screens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Main,
    News,
}

impl Style {
    pub fn from_layout(name: &str) -> Option<Style> {
        match name {
            "studio" => Some(Style::Main),
            "studio-news" => Some(Style::News),
            _ => None,
        }
    }
}

pub struct StudioScreen {
    layer: Layer,
    bars: Layer,
    sig: String,
    bg: TilesScreen,
}

impl StudioScreen {
    pub fn new() -> Self {
        StudioScreen { layer: Layer::new(1, 1), bars: Layer::new(1, 1), sig: String::new(), bg: TilesScreen::new() }
    }

    /// The spectrum moves, so the screen wants a steady rate.
    pub fn wanted_fps(active_fps: f32) -> f32 {
        active_fps.clamp(15.0, 30.0)
    }

    /// Paints the cards (re-drawn only when something shown changes) and the spectrum (every
    /// frame) over `fb`. `area` is the part of `fb` inside the margins; the spectrum uses the
    /// whole width and the bottom edge of `fb`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        fb: &mut Framebuffer,
        area: Rect,
        style: Style,
        data: &WidgetData,
        heights: &[f32],
        s: &Settings,
        gradient: &[Rgb],
        env: &Env,
    ) {
        // an optional background of its own (`tiles_bg`), otherwise the global one is kept
        let bg = Settings { bg: s.bg, bg_colors: s.bg_colors.clone(), bg_image: s.bg_image.clone(), bg_dim: s.bg_dim, ..Settings::default() };
        self.bg.paint_background(fb, &bg, env);

        let (ax, ay, aw, ah) = area;
        if aw < 200 || ah < 120 {
            return;
        }
        let strip_h = ((ah as f32 * 0.26).round() as u32).clamp(40, 200);
        let now = Local::now();
        let pal = s.palette();
        let stops: Vec<Rgb> = if gradient.len() >= 2 { gradient.to_vec() } else { vec![(0x00, 0xBF, 0xFF), (0xA8, 0x55, 0xF7), (0xFF, 0x40, 0x81)] };

        // ---- the cards
        let spec = |kind| TileSpec { kind, look: Look::Bar, tall: false };
        let news_view = view(spec(Kind::News), data, s, &now);
        let weather_view = view(spec(Kind::Weather), data, s, &now);
        let time = now.format("%H:%M:%S").to_string();
        let date = nice_case(&now.format("%A, %-d %B %Y").to_string());
        let sig = format!("{style:?}|{s:?}|{area:?}|{time}|{date}|{news_view:?}|{weather_view:?}|{:?}", stops);
        if sig != self.sig {
            if self.layer.w != aw as usize || self.layer.h != ah as usize {
                self.layer = Layer::new(aw, ah);
            } else {
                self.layer.clear();
            }
            let gap = s.gap.max(8) as f32;
            let cards_h = (ah as f32 - strip_h as f32 - gap).max(60.0);
            let radius = s.radius.map_or((cards_h * 0.16).clamp(14.0, 44.0), |r| r as f32);
            let look = Look2 { text: pal.text, muted: lerp_rgb(pal.text, pal.fill, 0.4), track_alpha: 40 };
            let mut p = Painter { layer: &mut self.layer, font: FontStyle::Smooth };
            match style {
                Style::Main => {
                    let cw = (aw as f32 * 0.56).round();
                    let ww = aw as f32 - cw - gap;
                    glass(&mut p, (0.0, 0.0, cw, cards_h), radius, &pal);
                    draw_studio_clock(&mut p, &look, &time, &date, (0.0, 0.0, cw, cards_h));
                    glass(&mut p, (cw + gap, 0.0, ww, cards_h), radius, &pal);
                    draw_weather(&mut p, &look, &weather_view, (cw + gap, 0.0, ww, cards_h));
                }
                Style::News => {
                    glass(&mut p, (0.0, 0.0, aw as f32, cards_h), radius, &pal);
                    draw_news(&mut p, &look, &news_view, (0.0, 0.0, aw as f32, cards_h));
                }
            }
            self.sig = sig;
        }
        self.layer.composite_onto(fb, ax, ay);

        // ---- the spectrum, flush with the bottom edge of the panel
        let (fw, fh) = (fb.width(), fb.height());
        let sh = strip_h.min(fh);
        if self.bars.w != fw as usize || self.bars.h != sh as usize {
            self.bars = Layer::new(fw, sh);
        } else {
            self.bars.clear();
        }
        let n = heights.len().max(1);
        let gap_px = 6.0f32;
        let bw = ((fw as f32 - gap_px * (n as f32 - 1.0)) / n as f32).max(2.0);
        for (i, h) in heights.iter().enumerate() {
            let frac = h.clamp(0.0, 1.0).powf(0.8);
            let bh = (frac * sh as f32).max(3.0);
            let x0 = (i as f32 * (bw + gap_px)).round() as i32;
            let x1 = (i as f32 * (bw + gap_px) + bw).round() as i32;
            let top = sh as f32 - bh;
            for y in (top.floor() as i32)..(sh as i32) {
                let t = ((sh as f32 - y as f32) / sh as f32).clamp(0.0, 1.0);
                let rgb = gradient_at(&stops, t);
                // soft top edge
                let a = if (y as f32) < top + 1.0 { 150 } else { 235 };
                self.bars.span_over(y, x0, x1, rgb, a);
            }
        }
        self.bars.composite_onto(fb, 0, fh - sh);
    }
}

/// A frosted-glass card: translucent fill, a lighter sheen over the upper half, a thin edge.
fn glass(p: &mut Painter, rect: (f32, f32, f32, f32), radius: f32, pal: &Palette) {
    let (x, y, w, h) = rect;
    p.layer.fill_rrect_solid((x.round(), y.round(), w.round(), h.round()), radius, pal.fill, pal.alpha);
    p.layer.fill_rrect((x + 2.0, y + 2.0, w - 4.0, h * 0.46), (radius - 2.0).max(4.0), &|_| (255, 255, 255), 16);
    p.layer.stroke_rrect((x, y, w, h), radius, 1.5, (255, 255, 255), 56);
}

/// Big thin time with the date under it, left aligned like the Studio preview.
fn draw_studio_clock(p: &mut Painter, look: &Look2, time: &str, date: &str, (x, y, w, h): (f32, f32, f32, f32)) {
    let pad = (h * 0.12).clamp(14.0, 40.0);
    let inner = w - 2.0 * pad;
    let dcap = (h * 0.13).clamp(10.0, 34.0);
    let avail = h - 2.0 * pad - dcap * 1.6;
    let tcap = p.fit(time, avail.min(h * 0.62), inner, W::Light);
    let total = tcap + dcap * 1.6;
    let top = y + (h - total) / 2.0;
    p.text(x + pad, top + tcap, time, tcap, W::Light, look.text, 255);
    let dc = p.fit(date, dcap, inner, W::Regular);
    p.text(x + pad, top + tcap + dcap * 1.5, date, dc, W::Regular, look.muted, 255);
}
