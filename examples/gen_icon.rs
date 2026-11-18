use resvg::{tiny_skia, usvg};
use std::fs;
use std::path::Path;

const ICO_SIZES: &[u32] = &[16, 24, 32, 48, 64, 256];

fn render_svg(svg_bytes: &[u8], size: u32) -> tiny_skia::Pixmap {
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_bytes, &opts).expect("valid SVG");
    let svg_size = tree.size();
    let scale = (size as f32 / svg_size.width()).min(size as f32 / svg_size.height());
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("valid pixmap");
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
}

fn encode_dib_frame(pixmap: &tiny_skia::Pixmap) -> Vec<u8> {
    let sz = pixmap.width();
    let pixels = pixmap.pixels();

    // Bottom-up 32bpp BGRA pixel data (unpremultiplied)
    let mut bgra = Vec::with_capacity((sz * sz * 4) as usize);
    for r in (0..sz).rev() {
        for c in 0..sz {
            let p = pixels[(r * sz + c) as usize].demultiply();
            bgra.push(p.blue());
            bgra.push(p.green());
            bgra.push(p.red());
            bgra.push(p.alpha());
        }
    }

    let and_row_bytes = sz.div_ceil(32) * 4;
    let mut and_mask = vec![0u8; (and_row_bytes * sz) as usize];
    for r in 0..sz {
        let src_r = sz - 1 - r;
        for c in 0..sz {
            let a = pixels[(src_r * sz + c) as usize].alpha();
            if a == 0 {
                let idx = (r * and_row_bytes + (c >> 3)) as usize;
                and_mask[idx] |= 0x80 >> (c & 7);
            }
        }
    }

    let mut out = Vec::with_capacity(40 + bgra.len() + and_mask.len());
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(sz as i32).to_le_bytes()); // biWidth
    out.extend_from_slice(&((sz * 2) as i32).to_le_bytes()); // biHeight (XOR + AND)
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    out.extend_from_slice(&((bgra.len() + and_mask.len()) as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // biXPelsPerMeter
    out.extend_from_slice(&0u32.to_le_bytes()); // biYPelsPerMeter
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
    out.extend_from_slice(&bgra);
    out.extend_from_slice(&and_mask);
    out
}

fn main() {
    let svg_bytes = fs::read("assets/icon.svg").expect("read assets/icon.svg");

    let mut frames: Vec<(u32, Vec<u8>)> = Vec::new();
    for &sz in ICO_SIZES {
        let pm = render_svg(&svg_bytes, sz);
        let payload = if sz >= 256 {
            pm.encode_png().expect("encode png")
        } else {
            encode_dib_frame(&pm)
        };
        frames.push((sz, payload));
    }

    let count = frames.len() as u16;
    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&count.to_le_bytes());

    let mut offset = 6u32 + 16u32 * (frames.len() as u32);
    let mut payloads = Vec::new();
    for (sz, data) in frames {
        let dim = if sz >= 256 { 0u8 } else { sz as u8 };
        ico.push(dim);
        ico.push(dim);
        ico.push(0);
        ico.push(0);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(data.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
        payloads.extend_from_slice(&data);
    }
    ico.extend_from_slice(&payloads);

    fs::write("assets/icon.ico", &ico).expect("write assets/icon.ico");
    println!("Wrote assets/icon.ico ({} bytes)", ico.len());

    if let Some(preview_path) = std::env::args().nth(1) {
        let pm256 = render_svg(&svg_bytes, 256);
        let png = pm256.encode_png().expect("encode preview");
        if let Some(parent) = Path::new(&preview_path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&preview_path, png).expect("write preview png");
        println!("Wrote preview {preview_path}");
    }
}
