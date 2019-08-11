use rusttype::{Font, point, Scale};
use crate::gpu::{SCREEN_WIDTH};

pub struct RegisterOutput<'a> {
    font: Font<'a>,
}

impl<'a> RegisterOutput<'a> {
    pub fn new() -> RegisterOutput<'a> {
        let font_data = include_bytes!("../fonts/consola.ttf");
        let font = Font::from_bytes(font_data as &[u8]).expect("Error constructing font");
        RegisterOutput{ font }
    }

    pub fn output(&self, text: &str) -> Vec<u32> {
        let scale = Scale::uniform(12.0);
        let v_metrics = self.font.v_metrics(scale);
        let glyphs: Vec<_> = self.font
            .layout(text, scale, point(2.0, 2.0 + v_metrics.ascent))
            .collect();

        let mut buffer = vec!();

        for _ in 0..(SCREEN_WIDTH * 16) {
            buffer.push(std::u32::MAX);
        }

        for g in glyphs {
            if let Some(bb) = g.pixel_bounding_box() {
                g.draw(|x, y, v| {
                    let bb_x = x + bb.min.x as u32;// + y * SCREEN_WIDTH as u32;
                    let bb_y = y + bb.min.y as u32;// + y * SCREEN_WIDTH as u32;
                    let i = bb_x + bb_y * SCREEN_WIDTH as u32;
                    let val = ((1.0 - v) * 255.0) as u32;
                    buffer[i as usize] =
                        val << 24 |
                        val << 16 |
                        val << 8 |
                        val;
                })
            }
        }
        buffer
    }
}
