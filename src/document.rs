use std::io;

use crate::layout::Layout;
use swash::{
    shape::ShapeContext,
    text::{
        cluster::{CharCluster, Parser, Token},
        Script,
    },
    FontRef,
};

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
        mono_fontref: FontRef<'_>,
        emoji_fontref: FontRef<'_>,
        size: f32,
        shape_context: &mut ShapeContext,
    ) {
        if !self.is_dirty {
            // no need to do this again!
            return;
        }
        self.layout.reset();

        let mut cluster = CharCluster::new();
        let mono_charmap = mono_fontref.charmap();
        let emoji_charmap = emoji_fontref.charmap();

        for (line_no, line) in self.rope.lines().enumerate() {
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
            let mut current_fontref = &mono_fontref;
            let mut shaper = shape_context
                .builder(mono_fontref)
                .script(Script::Latin)
                .size(size)
                .build();
            while parser.next(&mut cluster) {
                // find the font that best matches this cluster

                // This is an attempt to make sure characters that can be rendered as emoji
                // (e.g. 0..9, *, #, etc) are not done so unless they have the fe0f switch
                // after them, and to make sure characters that are emoji, but can be rendered
                // using the non-emoji font are tried with the emoji font first.
                let try_emoji_first = cluster.chars().iter().any(|ch| !ch.ch.is_ascii());
                let emoji_result;
                let mono_result;
                if try_emoji_first {
                    emoji_result = cluster.map(|ch| emoji_charmap.map(ch));
                    if matches!(emoji_result, swash::text::cluster::Status::Complete) {
                        mono_result = swash::text::cluster::Status::Discard;
                    } else {
                        mono_result = cluster.map(|ch| mono_charmap.map(ch));
                    }
                } else {
                    mono_result = cluster.map(|ch| mono_charmap.map(ch));
                    if matches!(mono_result, swash::text::cluster::Status::Complete) {
                        emoji_result = swash::text::cluster::Status::Discard;
                    } else {
                        emoji_result = cluster.map(|ch| emoji_charmap.map(ch));
                    }
                }
                // println!(
                //     "{:?}: {}, {:?}|{:?}",
                //     cluster.chars().iter().map(|ch| (ch.ch, emoji_charmap.map(ch.ch), mono_charmap.map(ch.ch))).collect::<Vec<_>>(),
                //     try_emoji_first,
                //     emoji_result,
                //     mono_result,
                // );
                let is_emoji_fontref = std::ptr::eq(current_fontref, &emoji_fontref);
                use swash::text::cluster::Status::*;
                match (emoji_result, mono_result, try_emoji_first) {
                    (Discard, Keep | Complete, _)
                    | (Keep, Complete, _)
                    | (Complete, Complete, false)
                    | (Keep, Keep, false) => {
                        // monospace preferred
                        if is_emoji_fontref {
                            self.layout.push_run(line_no, shaper, current_fontref, size);
                            current_fontref = &mono_fontref;
                            shaper = shape_context
                                .builder(*current_fontref)
                                .script(Script::Latin)
                                .size(size)
                                .build();
                        }
                        shaper.add_cluster(&cluster);
                    }
                    (Keep | Complete, Discard, _)
                    | (Complete, Keep, _)
                    | (Complete, Complete, true)
                    | (Keep, Keep, true) => {
                        // emoji preferred
                        if !is_emoji_fontref {
                            self.layout.push_run(line_no, shaper, current_fontref, size);
                            current_fontref = &emoji_fontref;
                            shaper = shape_context
                                .builder(*current_fontref)
                                .script(Script::Latin)
                                .size(size)
                                .build();
                        }
                        shaper.add_cluster(&cluster);
                    }
                    (Discard, Discard, _) => {
                        // TODO!
                    }
                }
            }
            self.layout.push_run(line_no, shaper, current_fontref, size);
        }
        self.is_dirty = false;
    }
}
