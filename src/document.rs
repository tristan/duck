use std::io;

use swash::{text::{cluster::{Parser, Token, CharCluster, SourceRange}, Script}, shape::cluster::Glyph};

use crate::{layout::Layout, fonts::{Font, ShapeContext}};

pub struct Document {
    rope: ropey::Rope,
    pub layout: Layout,
    is_dirty: bool,
}

impl Document {
    pub fn from_str(text: &str) -> Document {
        Document {
            rope: ropey::Rope::from_str(text),
            layout: Layout::new(),
            is_dirty: true,
        }
    }

    pub fn from_reader<T: io::Read>(mut reader: T) -> io::Result<Document> {
        let rope = ropey::Rope::from_reader(reader)?;
        Ok(Document {
            rope,
            layout: Layout::new(),
            is_dirty: true,
        })
    }

    pub fn parse(
        &mut self,
        fonts: &[&Font],
        size: f32,
    ) {
        if !self.is_dirty {
            // no need to do this again!
            return;
        }
        self.layout.reset();

        let mut shapers = fonts.iter().copied().map(ShapeContext::new).collect::<Vec<_>>();
        let mut cluster = CharCluster::new();
        for (line_no, line) in self.rope.lines().enumerate() {
            // TODO: this should be par_iter()-able, but probably needs thread_local!
            // variables for ALL the things
            for shaper in shapers.iter_mut() {
                shaper.reset();
            }
            //let mut is_ascii = Vec::with_capacity(line.len_chars());
            let mut doc_indices = Vec::with_capacity(line.len_chars());
            let mut parser = Parser::new(
                Script::Latin,
                line.chars().map({
                    let mut offset = 0usize;
                    move |ch| {
                        let len = ch.len_utf8();
                        let current_offset = offset as u32;
                        offset += len;
                        Token {
                            ch,
                            offset: current_offset,
                            len: len as u8,
                            info: ch.into(),
                            data: 0,
                        }
                    }
                }),
            );
            while parser.next(&mut cluster) {
                let SourceRange { start: i, end: j } = cluster.range();
                doc_indices.push((line_no, i as usize, j as usize));
                for shaper in shapers.iter_mut() {
                    shaper.add_cluster(&cluster);
                }
            }
            let shapes = shapers.iter_mut().map(|s| s.shape()).collect::<Vec<_>>();
            let mut prev_font_index = 0;
            let mut glyphs: Vec<Glyph> = Vec::with_capacity(1);
            let mut prev_range_start = 0;
            let mut prev_range_end = 0;
            for (i, idx) in doc_indices.iter().enumerate() {
                println!("cluster: {:?} ", line.get_byte_slice(idx.1..idx.2));
                let mut best = None;
                for (font_index, shape) in shapes.iter().enumerate() {
                    let cluster = shape.get(i).unwrap();
                    let num_complete = cluster.iter().filter(|g| g.id != 0).count();
                    println!("    {} num_complete={} len={}", font_index, num_complete, cluster.len());
                    let ratio = num_complete as f32 / cluster.len() as f32;
                    let len = cluster.len();
                    // if num_complete == cluster.len() {
                    //     best = Some((font_index, cluster, num_complete));
                    //     break;
                    // } else
                    if let &Some((_, _, prev_ratio, prev_len)) = &best {
                        if prev_ratio < ratio || (prev_ratio == ratio && prev_len > len) {
                            best = Some((font_index, cluster, ratio, len));
                        }
                    } else {
                        best = Some((font_index, cluster, ratio, len));
                    }
                }
                println!("    BEST = {:?}", best);
                let Some((font_index, cluster, _, _)) = best else { panic!("should be imposible if we have fonts") };
                if font_index != prev_font_index {
                    if !glyphs.is_empty() {
                        self.layout.push_run(line_no, prev_font_index, prev_range_start..prev_range_end, glyphs, size, fonts[prev_font_index].metrics);
                        glyphs = Vec::with_capacity(1);
                    }
                    prev_font_index = font_index;
                    prev_range_start = idx.1;
                }

                prev_range_end = idx.2;
                glyphs.extend(cluster.iter().cloned());
            }

            if !glyphs.is_empty() {
                self.layout.push_run(line_no, prev_font_index, prev_range_start..prev_range_end, glyphs, size, fonts[prev_font_index].metrics);
            }
        }
        self.is_dirty = false;
    }
}
