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
        let fahrenheit = opts().weather_fahrenheit;
        let sig = format!("{style:?}|{s:?}|{area:?}|{time}|{date}|{news_view:?}|{weather_view:?}|{fahrenheit}|{:?}", stops);
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
                    let cw = (aw as f32 * 0.60).round();
                    let ww = aw as f32 - cw - gap;
                    glass(&mut p, (0.0, 0.0, cw, cards_h), radius, &pal, &stops);
                    draw_studio_clock(&mut p, &look, &time, &date, (0.0, 0.0, cw, cards_h));
                    glass(&mut p, (cw + gap, 0.0, ww, cards_h), radius, &pal, &stops);
                    draw_studio_weather(&mut p, &look, &weather_view, data, fahrenheit, (cw + gap, 0.0, ww, cards_h));
                }
                Style::News => {
                    glass(&mut p, (0.0, 0.0, aw as f32, cards_h), radius, &pal, &stops);
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
        let loudest = heights.iter().cloned().fold(0.0f32, f32::max);
        // no music: a slow, gentle wave so the gradient is always visible
        let idle_mix = (1.0 - loudest * 20.0).clamp(0.0, 1.0);
        let secs = (now.timestamp_millis() % 600_000) as f32 / 1000.0;
        for (i, h) in heights.iter().enumerate() {
            let wave = 0.10 + 0.09 * (((i as f32) * 0.33 + secs * 1.4).sin() * 0.5 + 0.5);
            let level = h.clamp(0.0, 1.0).powf(0.8).max(wave * idle_mix);
            let bh = (level * sh as f32).max(4.0);
            let x0 = (i as f32 * (bw + gap_px)).round() as i32;
            let x1 = (i as f32 * (bw + gap_px) + bw).round() as i32;
            let top = (sh as f32 - bh).floor() as i32;
            let r = (bw / 2.0).min(7.0);
            for y in top..(sh as i32) {
                let t = ((sh as f32 - y as f32) / sh as f32).clamp(0.0, 1.0);
                let rgb = gradient_at(&stops, t);
                let dy = (y - top) as f32;
                let inset = if dy < r { (r - (r * r - (r - dy - 0.5).powi(2)).max(0.0).sqrt()).round() as i32 } else { 0 };
                self.bars.span_over(y, x0 + inset, x1 - inset, rgb, 235);
            }
        }
        self.bars.composite_onto(fb, 0, fh - sh);
    }
}

/// A frosted-glass card: a translucent dark fill, a soft color wash from the spectrum colors, a
/// lighter sheen over the upper half and a thin lavender edge.
fn glass(p: &mut Painter, rect: (f32, f32, f32, f32), radius: f32, pal: &Palette, stops: &[Rgb]) {
    let (x, y, w, h) = rect;
    p.layer.fill_rrect_solid((x.round(), y.round(), w.round(), h.round()), radius, pal.fill, pal.alpha);
    let st = stops.to_vec();
    p.layer.fill_rrect((x + 1.0, y + 1.0, w - 2.0, h - 2.0), (radius - 1.0).max(4.0), &move |t| gradient_at(&st, t), 38);
    p.layer.fill_rrect((x + 2.0, y + 2.0, w - 4.0, h * 0.46), (radius - 2.0).max(4.0), &|_| (255, 255, 255), 18);
    p.layer.stroke_rrect((x, y, w, h), radius, 1.6, (0xED, 0xC6, 0xFF), 90);
}

/// Anti-aliased filled disc.
fn disc(p: &mut Painter, cx: f32, cy: f32, r: f32, rgb: Rgb, alpha: u8) {
    for py in ((cy - r - 1.0).floor() as i32)..((cy + r + 1.0).ceil() as i32) {
        for px in ((cx - r - 1.0).floor() as i32)..((cx + r + 1.0).ceil() as i32) {
            let d = (px as f32 + 0.5 - cx).hypot(py as f32 + 0.5 - cy);
            let cov = (r + 0.5 - d).clamp(0.0, 1.0);
            if cov > 0.0 {
                p.layer.over(px, py, rgb, (alpha as f32 * cov).round() as u8);
            }
        }
    }
}

/// Soft glow around a point (fades from `alpha` at the middle to nothing at `r`).
fn glow(p: &mut Painter, cx: f32, cy: f32, r: f32, rgb: Rgb, alpha: u8) {
    for py in ((cy - r).floor() as i32)..((cy + r).ceil() as i32) {
        for px in ((cx - r).floor() as i32)..((cx + r).ceil() as i32) {
            let d = (px as f32 + 0.5 - cx).hypot(py as f32 + 0.5 - cy) / r;
            if d < 1.0 {
                let f = (1.0 - d).powi(2);
                p.layer.over(px, py, rgb, (alpha as f32 * f).round() as u8);
            }
        }
    }
}

fn cloud(p: &mut Painter, cx: f32, cy: f32, s: f32, rgb: Rgb) {
    disc(p, cx - 0.55 * s, cy + 0.12 * s, 0.42 * s, rgb, 255);
    disc(p, cx - 0.05 * s, cy - 0.18 * s, 0.58 * s, rgb, 255);
    disc(p, cx + 0.55 * s, cy + 0.10 * s, 0.44 * s, rgb, 255);
    p.layer.fill_rrect((cx - 0.98 * s, cy + 0.02 * s, 2.0 * s, 0.52 * s), 0.26 * s, &move |_| rgb, 255);
}

fn sun(p: &mut Painter, cx: f32, cy: f32, r: f32) {
    glow(p, cx, cy, r * 2.4, (255, 190, 100), 120);
    disc(p, cx, cy, r, (255, 154, 94), 255);
    disc(p, cx - r * 0.12, cy - r * 0.12, r * 0.82, (255, 193, 94), 255);
    disc(p, cx - r * 0.25, cy - r * 0.25, r * 0.5, (255, 244, 194), 255);
}

/// A modern weather picture (sun, clouds, rain...) centered on (cx, cy), about `size` wide.
fn weather_glyph(p: &mut Painter, icon: weather_icon::WeatherIcon, cx: f32, cy: f32, size: f32) {
    use weather_icon::WeatherIcon as I;
    let s = size / 2.2;
    let cloud_rgb = (0xE4, 0xDB, 0xF2);
    let dark_rgb = (0xB5, 0xAB, 0xCB);
    let drops = |p: &mut Painter, n: usize, rgb: Rgb| {
        for i in 0..n {
            let dx = (i as f32 - (n as f32 - 1.0) / 2.0) * 0.62 * s;
            p.layer.fill_rrect((cx + dx - 0.07 * s, cy + 0.72 * s, 0.14 * s, 0.42 * s), 0.07 * s, &move |_| rgb, 235);
        }
    };
    match icon {
        I::Clear => sun(p, cx, cy, 0.8 * s),
        I::PartlyCloudy => {
            sun(p, cx - 0.45 * s, cy - 0.38 * s, 0.62 * s);
            cloud(p, cx + 0.2 * s, cy + 0.28 * s, 0.95 * s, cloud_rgb);
        }
        I::Cloudy => {
            cloud(p, cx + 0.3 * s, cy - 0.3 * s, 0.7 * s, dark_rgb);
            cloud(p, cx - 0.1 * s, cy + 0.15 * s, 0.95 * s, cloud_rgb);
        }
        I::Fog => {
            cloud(p, cx, cy - 0.25 * s, 0.9 * s, dark_rgb);
            for i in 0..3 {
                p.layer.fill_rrect((cx - 0.9 * s + i as f32 * 0.2 * s, cy + 0.5 * s + i as f32 * 0.3 * s, 1.8 * s - i as f32 * 0.4 * s, 0.12 * s), 0.06 * s, &|_| (0xC8, 0xC0, 0xDA), 220);
            }
        }
        I::Drizzle => {
            cloud(p, cx, cy - 0.25 * s, 0.95 * s, dark_rgb);
            drops(p, 3, (0x00, 0xBF, 0xFF));
        }
        I::Rain => {
            cloud(p, cx, cy - 0.25 * s, 0.95 * s, dark_rgb);
            drops(p, 4, (0x00, 0xBF, 0xFF));
        }
        I::Snow => {
            cloud(p, cx, cy - 0.25 * s, 0.95 * s, cloud_rgb);
            for i in 0..3 {
                disc(p, cx + (i as f32 - 1.0) * 0.6 * s, cy + 0.9 * s, 0.1 * s, (255, 255, 255), 255);
            }
        }
        I::Thunder => {
            cloud(p, cx, cy - 0.25 * s, 0.95 * s, (0x8E, 0x84, 0xA8));
            drops(p, 2, (0xFF, 0xD2, 0x3F));
            p.layer.fill_rrect((cx - 0.08 * s, cy + 0.55 * s, 0.16 * s, 0.7 * s), 0.07 * s, &|_| (0xFF, 0xD2, 0x3F), 255);
        }
    }
}

/// Big thin time with the date under it.
fn draw_studio_clock(p: &mut Painter, look: &Look2, time: &str, date: &str, (x, y, w, h): (f32, f32, f32, f32)) {
    let pad = (h * 0.12).clamp(14.0, 44.0);
    let inner = w - 2.0 * pad;
    let dcap = (h * 0.12).clamp(10.0, 34.0);
    let avail = h - 2.0 * pad - dcap * 1.8;
    let tcap = p.fit(time, avail.min(h * 0.66), inner, W::Light);
    let total = tcap + dcap * 1.8;
    let top = y + (h - total) / 2.0;
    let tw = p.width(time, tcap, W::Light);
    let left = x + (w - tw) / 2.0;
    p.text(left, top + tcap, time, tcap, W::Light, look.text, 255);
    let dc = p.fit(date, dcap, inner, W::Regular);
    let dw = p.width(date, dc, W::Regular);
    p.text(x + (w - dw) / 2.0, top + tcap + dcap * 1.6, date, dc, W::Regular, look.muted, 255);
}

/// The weather card: a picture on the left, the big temperature, the condition and the city.
fn draw_studio_weather(p: &mut Painter, look: &Look2, v: &TileView, data: &WidgetData, fahrenheit: bool, (x, y, w, h): (f32, f32, f32, f32)) {
    let pad = (h * 0.12).clamp(14.0, 44.0);
    let Some(wx) = &data.weather else {
        let dash = "--";
        let cap = h * 0.4;
        let tw = p.width(dash, cap, W::Light);
        p.text(x + (w - tw) / 2.0, y + (h + cap) / 2.0, dash, cap, W::Light, look.muted, 255);
        return;
    };
    let glyph = (h * 0.62).min(w * 0.36);
    weather_glyph(p, wx.icon, x + pad + glyph / 2.0, y + h / 2.0, glyph);
    let tx = x + pad + glyph + pad * 0.6;
    let inner = x + w - pad - tx;
    let temp = format!("{:.0}\u{b0}{}", wx.temp_in(fahrenheit), if fahrenheit { "F" } else { "C" });
    let ccap = (h * 0.12).clamp(10.0, 30.0);
    let cond = nice_case(weather_icon::condition_name(wx.icon));
    let city = v.label.clone();
    let tcap = p.fit(&temp, h * 0.46, inner, W::Light);
    let total = tcap + ccap * 3.2;
    let top = y + (h - total) / 2.0;
    p.text(tx, top + tcap, &temp, tcap, W::Light, look.text, 255);
    p.text(tx, top + tcap + ccap * 1.5, &fit_label(p, &cond, ccap, inner), ccap, W::Medium, look.text, 255);
    p.text(tx, top + tcap + ccap * 2.9, &fit_label(p, &city, ccap * 0.85, inner), ccap * 0.85, W::Regular, look.muted, 255);
}
