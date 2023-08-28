use std::ops::Range;

use swash::{
    shape::{Shaper},
    FontRef, Metrics,
};

use crate::fonts::{Font, Glyph};

//use super::fonts::FontCacheKey;

#[derive(Debug)]
pub struct Run {
    pub font_index: usize,
    pub glyphs: Vec<Glyph>,
    pub coords: Vec<i16>,
    pub size: f32,
    pub metrics: Metrics,
}

#[derive(Default, Debug)]
pub struct Line {
    pub runs: Vec<Run>,
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    pub above: f32,
    pub below: f32,
}

impl Line {
    fn reset(&mut self) {
        self.runs.clear();
    }
}

#[derive(Default)]
pub struct Layout {
    pub lines: Vec<Line>,
}

impl Layout {
    pub fn new() -> Layout {
        Layout::default()
    }

    pub fn reset(&mut self) {
        for line in &mut self.lines {
            line.reset();
        }
    }

    pub fn push_run(
        &mut self,
        line_no: usize,
        font_index: usize,
        doc_byte_range: Range<usize>,
        glyphs: &[Glyph],
    ) {
        println!("RUN: {} {} {:?} {:?}", line_no, font_index, doc_byte_range, glyphs);
        // let mut glyphs = Vec::new();
        // let metrics = shaper.metrics();
        // let coords = shaper.normalized_coords().to_vec();
        // {
        //     let glyphs = &mut glyphs;
        //     shaper.shape_with(move |c| {
        //         for g in c.glyphs {
        //             glyphs.push(*g);
        //         }
        //     });
        // }
        // if glyphs.is_empty() {
        //     return;
        // }
        // while self.lines.len() <= line_no {
        //     self.lines.push(Line::default());
        // }
        // let line = &mut self.lines[line_no];
        // line.runs.push(Run {
        //     font: (),
        //     glyphs,
        //     coords,
        //     size,
        //     metrics,
        // });
    }

    pub fn finish(&mut self) {
        for line in &mut self.lines {
            line.ascent = 0.;
            line.descent = 0.;
            line.leading = 0.;
            for run in &line.runs {
                line.ascent = line.ascent.max(run.metrics.ascent);
                line.descent = line.descent.max(run.metrics.descent);
                line.leading = line.leading.max(run.metrics.leading);
            }
            // eh???
            line.ascent = line.ascent.round();
            line.descent = line.ascent.round();
            line.leading = (line.leading * 0.5).round() * 2.;
            line.below = (line.descent + line.leading * 0.5).round();
            // baseline = y + above
            line.above = (line.ascent + line.leading * 0.5).round();
        }
    }
}
