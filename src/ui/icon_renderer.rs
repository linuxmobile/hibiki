use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::gio::MemoryInputStream;
use gtk4::glib::Bytes;
use ksni::Icon;
use tracing::error;

const LOGO_SVG: &[u8] = include_bytes!("../assets/logo-symbolic.svg");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconTheme {
    Light,
    Dark,
}

pub fn render_tray_icon(theme: IconTheme) -> Vec<Icon> {
    let (fg_color, bg_color) = match theme {
        IconTheme::Dark => ("#ffffff", "#2e3436"),
        IconTheme::Light => ("#2e3436", "#ffffff"),
    };

    let svg_str = String::from_utf8_lossy(LOGO_SVG);
    let svg_with_color = svg_str
        .replace("currentColor", fg_color)
        .replace("currentBackgroundColor", bg_color);

    let bytes = Bytes::from(svg_with_color.as_bytes());
    let stream = MemoryInputStream::from_bytes(&bytes);

    let size = 22;

    match Pixbuf::from_stream_at_scale(&stream, size, size, false, gtk4::gio::Cancellable::NONE) {
        Ok(pixbuf) => {
            let width = pixbuf.width();
            let height = pixbuf.height();
            let channels = pixbuf.n_channels();
            let rowstride = pixbuf.rowstride();

            let pixels = match pixbuf.pixel_bytes() {
                Some(p) => p,
                None => {
                    error!("Failed to get pixel bytes from pixbuf");
                    return Vec::new();
                }
            };

            let pixel_slice: &[u8] = pixels.as_ref();

            let mut argb_data = Vec::with_capacity((width * height * 4) as usize);

            for y in 0..height {
                for x in 0..width {
                    let offset = (y * rowstride + x * channels) as usize;
                    if channels == 4 {
                        let r = pixel_slice[offset];
                        let g = pixel_slice[offset + 1];
                        let b = pixel_slice[offset + 2];
                        let a = pixel_slice[offset + 3];

                        argb_data.push(a);
                        argb_data.push(r);
                        argb_data.push(g);
                        argb_data.push(b);
                    } else if channels == 3 {
                        let r = pixel_slice[offset];
                        let g = pixel_slice[offset + 1];
                        let b = pixel_slice[offset + 2];

                        argb_data.push(255);
                        argb_data.push(r);
                        argb_data.push(g);
                        argb_data.push(b);
                    }
                }
            }

            vec![Icon {
                width,
                height,
                data: argb_data,
            }]
        }
        Err(e) => {
            error!("Failed to render tray icon from SVG: {}", e);
            Vec::new()
        }
    }
}
