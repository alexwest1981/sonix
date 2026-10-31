//! CPU effects on a decoded frame, shared by playback and export.
//!
//! A frame is the RGB24 buffer `video_engine` already produces, three bytes per pixel, top
//! row first — the chain never allocates a second buffer type, so the picture on screen and
//! the picture in the file are filtered by the same bytes and the same code. An empty chain
//! touches nothing, which keeps the no-effect path byte-identical to an unfiltered decode.

use super::video_engine::Frame;

/// One effect. `apply` gets the whole RGB24 frame and its dimensions; effects that do not
/// need the geometry ignore the two extra arguments.
pub trait VideoFx {
    fn name(&self) -> &str;

    fn apply(&mut self, frame: &mut [u8], width: usize, height: usize);

    /// The numbers the UI edits: one for brightness and pixelate, three for tint.
    fn params(&mut self) -> &mut [f32];
}

/// Multiplies every channel by `gain`; 1.0 is the untouched picture.
pub struct Brightness {
    pub gain: f32,
}

impl VideoFx for Brightness {
    fn name(&self) -> &str {
        "Brightness"
    }

    fn apply(&mut self, frame: &mut [u8], width: usize, height: usize) {
        let gain = self.gain.max(0.0);
        let end = (width * height * 3).min(frame.len());
        for level in &mut frame[..end] {
            *level = (*level as f32 * gain).round().clamp(0.0, 255.0) as u8;
        }
    }

    fn params(&mut self) -> &mut [f32] {
        std::slice::from_mut(&mut self.gain)
    }
}

/// Per-channel multipliers; `[1.0, 1.0, 1.0]` is the untouched picture, red is `[2.0, 1.0, 1.0]`.
pub struct Tint {
    pub rgb: [f32; 3],
}

impl VideoFx for Tint {
    fn name(&self) -> &str {
        "Tint"
    }

    fn apply(&mut self, frame: &mut [u8], width: usize, height: usize) {
        let end = (width * height * 3).min(frame.len());
        for pixel in frame[..end].chunks_exact_mut(3) {
            for (level, gain) in pixel.iter_mut().zip(self.rgb) {
                *level = (*level as f32 * gain.max(0.0)).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    fn params(&mut self) -> &mut [f32] {
        &mut self.rgb
    }
}

/// Averages nothing and samples once: every pixel in a `block`×`block` cell takes the colour
/// of the cell's top-left pixel. Nearest-neighbour is what "pixelate" means here, and it keeps
/// the effect free of a second pass over the frame.
pub struct Pixelate {
    pub block: f32,
}

impl VideoFx for Pixelate {
    fn name(&self) -> &str {
        "Pixelate"
    }

    fn apply(&mut self, frame: &mut [u8], width: usize, height: usize) {
        let block = (self.block.round() as usize).max(1);
        if block == 1 || width == 0 || height == 0 {
            return;
        }
        for top in (0..height).step_by(block) {
            for left in (0..width).step_by(block) {
                let source = (top * width + left) * 3;
                // A frame that is shorter than its dimensions says so by yielding nothing;
                // the rest of the block is then left alone instead of panicking.
                let Some(cell) = frame.get(source..source + 3) else {
                    return;
                };
                let colour = [cell[0], cell[1], cell[2]];
                for y in top..(top + block).min(height) {
                    for x in left..(left + block).min(width) {
                        let target = (y * width + x) * 3;
                        if let Some(pixel) = frame.get_mut(target..target + 3) {
                            pixel.copy_from_slice(&colour);
                        }
                    }
                }
            }
        }
    }

    fn params(&mut self) -> &mut [f32] {
        std::slice::from_mut(&mut self.block)
    }
}

/// The effects in the order they run. Empty by default, so a project that never asked for a
/// filter gets the decoder's bytes untouched.
#[derive(Default)]
pub struct VideoFxChain {
    effects: Vec<Box<dyn VideoFx>>,
}

impl VideoFxChain {
    pub fn push(&mut self, fx: Box<dyn VideoFx>) {
        self.effects.push(fx);
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.effects.len() {
            self.effects.remove(index);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn VideoFx>> {
        self.effects.iter_mut()
    }

    pub fn apply(&mut self, frame: &mut [u8], width: usize, height: usize) {
        for fx in &mut self.effects {
            fx.apply(frame, width, height);
        }
    }

    /// Filters one decoded frame. Playback and export both call this, so a frame cannot look
    /// one way on screen and another way in the file.
    pub fn apply_frame(&mut self, frame: &mut Frame) {
        let (width, height) = (frame.width as usize, frame.height as usize);
        self.apply(&mut frame.rgb, width, height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::video_export::prepared_frame;

    /// First eight bytes of SHA-256 over the frame, so a report can name the bytes it compared.
    fn hash(bytes: &[u8]) -> String {
        let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
        digest.as_ref()[..8].iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn grey(width: usize, height: usize, level: u8) -> Frame {
        Frame {
            width: width as u32,
            height: height as u32,
            rgb: vec![level; width * height * 3],
            pts_secs: 0.0,
            produced_at: std::time::Instant::now(),
        }
    }

    /// A frame with a different colour in every pixel, so a filter cannot pass by accident.
    fn gradient(width: usize, height: usize) -> Frame {
        let mut frame = grey(width, height, 0);
        for (index, pixel) in frame.rgb.chunks_exact_mut(3).enumerate() {
            pixel[0] = (index % 251) as u8;
            pixel[1] = (index % 97) as u8;
            pixel[2] = (index / 7 % 61) as u8;
        }
        frame
    }

    /// (a) Gain 1.5 lifts every channel of a mid-grey frame: 128 becomes 192.
    #[test]
    fn brightness_1_5_raises_every_pixel_of_a_mid_grey_frame() {
        let mut frame = grey(4, 3, 128);
        let before = frame.rgb[0];
        let mut chain = VideoFxChain::default();
        chain.push(Box::new(Brightness { gain: 1.5 }));
        chain.apply_frame(&mut frame);

        println!("brightness 1.5 on mid grey: pixel 0 before {before}, after {}", frame.rgb[0]);
        println!("frame after: min {}, max {}", frame.rgb.iter().min().unwrap(), frame.rgb.iter().max().unwrap());
        assert_eq!(before, 128);
        assert_eq!(frame.rgb[0], 192, "128 * 1.5 is 192");
        assert!(frame.rgb.iter().all(|level| *level > 128), "a pixel was not lifted");
    }

    /// (b) No effects means no change at all, down to the byte.
    #[test]
    fn an_empty_chain_leaves_the_frame_byte_identical() {
        let mut frame = gradient(37, 11);
        let original = frame.rgb.clone();
        let mut chain = VideoFxChain::default();
        assert!(chain.is_empty());
        chain.apply_frame(&mut frame);

        println!(
            "empty chain: {} bytes, sha256 before {}, after {}",
            original.len(),
            hash(&original),
            hash(&frame.rgb)
        );
        assert_eq!(original, frame.rgb, "an empty chain changed the picture");
        assert_eq!(hash(&original), hash(&frame.rgb));
    }

    /// (c) Block 8: the 8x8 cell at the top left ends up as one colour.
    #[test]
    fn pixelate_with_block_8_makes_every_pixel_in_a_block_equal() {
        let (width, block) = (32usize, 8usize);
        let mut frame = gradient(width, 16);
        let collect_block = |rgb: &[u8]| -> Vec<u8> {
            let mut cell = Vec::with_capacity(block * block * 3);
            for y in 0..block {
                for x in 0..block {
                    let at = (y * width + x) * 3;
                    cell.extend_from_slice(&rgb[at..at + 3]);
                }
            }
            cell
        };
        let before = collect_block(&frame.rgb);

        let mut chain = VideoFxChain::default();
        chain.push(Box::new(Pixelate { block: 8.0 }));
        chain.apply_frame(&mut frame);

        let after = collect_block(&frame.rgb);
        println!(
            "pixelate block 8: top-left pixel before {:?}, after {:?}",
            &before[..3],
            &after[..3]
        );
        println!("first row of the block, first six pixels, before {:?}, after {:?}", &before[..18], &after[..18]);
        assert_eq!(&after[..3], &before[..3], "the cell must keep its top-left colour");
        let first = &after[..3];
        for pixel in after.chunks_exact(3) {
            assert_eq!(pixel, first, "a pixel inside the block kept its own colour");
        }
    }

    /// (d) The export's frame producer and the playback path filter the same decoded frame
    /// into the same bytes.
    #[test]
    fn export_and_playback_filter_one_frame_to_the_same_bytes() {
        let decoded = gradient(40, 6);
        let mut on_screen = decoded.clone();

        // The playback path: the view calls exactly this on the frame it puts on screen.
        let mut chain = VideoFxChain::default();
        chain.push(Box::new(Brightness { gain: 1.5 }));
        chain.push(Box::new(Pixelate { block: 8.0 }));
        chain.apply_frame(&mut on_screen);

        // The export path: the producer `export_clip` uses for every frame it pushes.
        let mut video = None;
        let exported =
            prepared_frame(&mut video, Some(&decoded), 0, 25.0, &mut chain).expect("a frame");

        println!("playback frame sha256 {}, export frame sha256 {}", hash(&on_screen.rgb), hash(&exported.rgb));
        assert_eq!(hash(&on_screen.rgb), hash(&exported.rgb));
        assert_eq!(on_screen.rgb, exported.rgb);
    }
}
