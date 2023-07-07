mod document;
mod fonts;
mod gfx;
mod layout;

use document::Document;
use fonts::FontSource;
use gfx::{
    color::{self, Color},
    compositor::Compositor,
    image_cache::ImageCache,
    types::Rect,
};
use swash::shape::ShapeContext;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

use crate::{
    fonts::FontFamily,
    gfx::{glyph_cache::GlyphCache, wgpu_context::WgpuContext},
};

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_resizable(true)
        .with_title("DUCK")
        .build(&event_loop)
        .expect("failed to create window");

    let mut wgpu = WgpuContext::new(&window);

    let mut fontsource = FontSource::new();
    let monospace_font = fontsource
        .load(&[FontFamily::Title("Jetbrains Mono".to_string())])
        .expect("Failed to load monospace font");
    let emoji_font = fontsource
        .load(&[FontFamily::Title("Noto Color Emoji".to_string())])
        .expect("failed to load emoji font family");
    let mut compositor = Compositor::new();
    let mut shape_context = ShapeContext::new();
    let mut image_cache = ImageCache::new(wgpu.device.limits().max_texture_dimension_2d);
    let mut glyph_cache = GlyphCache::new();

    //let document = Document::from_reader(std::fs::File::open("../../v0/emoji-zwj-sequences.txt").unwrap()).unwrap();
    let mut document = Document::from_str("Simple String!");
    // TODO: fix rendering extra empty character after 0 and * (and similar) emojis
    //let mut document = Document::from_str("y̆es 0️*1(*️)2*3 🧙🏻‍♂️⭐😶‍🌫️ *️*️*️ + 🦆&🙂😶");
    //let mut document = Document::from_str("🦆🦆🦆🦆🦆😶‍🌫️");

    let scale = window.scale_factor() as f32;
    document.parse(
        fontsource
            .get_fontref(monospace_font)
            .expect("Font should be loaded!"),
        fontsource
            .get_fontref(emoji_font)
            .expect("Font should be loaded!"),
        32. * scale, // TODO: if the scale changes we need to update things!
        &mut shape_context,
    );
    document.layout.finish();
    compositor.begin();
    let subpx_bias = (0.125, 0.);
    let screen_size = window.inner_size();
    let margin = 12.;
    let buffer_window = Rect::new(
        margin,
        margin,
        screen_size.width as f32 - margin,
        screen_size.height as f32 - margin,
    );
    for line in &document.layout.lines {
        let baseline = line.above;
        let mut px = buffer_window.x;
        for run in &line.runs {
            let font = fontsource
                .get_fontref(run.font)
                .expect("Font should be loaded!");
            let mut session =
                glyph_cache.session(&wgpu, &mut image_cache, font, run.size, &run.coords);
            let py = baseline + buffer_window.y;
            for g in &run.glyphs {
                let gx = px + g.x;
                let gy = py - g.y;
                px += g.advance;
                if let Some(entry) = session.get(g.id, gx, gy) {
                    if let Some(tex_loc) = session.get_texture_location(entry.image_id) {
                        let ix = (gx + subpx_bias.0).floor() + entry.left as f32;
                        let iy = (gy + subpx_bias.1).floor() - entry.top as f32;
                        if entry.is_bitmap {
                            compositor.add_image_rect(
                                [ix, iy, entry.width as f32, entry.height as f32],
                                0.01,
                                color::BLACK,
                                tex_loc,
                            );
                        } else {
                            compositor.add_subpixel_rect(
                                [ix, iy, entry.width as f32, entry.height as f32],
                                0.01,
                                color::BLACK,
                                tex_loc,
                            );
                        }
                    }
                }
            }
        }
    }

    // compositor.draw_rect([0.0f32, 0.0, 200.0, 200.0], 0.1, Color::new(255, 0, 0, 128));
    // compositor.draw_rect(
    //     [100.0f32, 100.0, 200.0, 200.0],
    //     0.1,
    //     Color::new(0, 255, 0, 64),
    // );
    // compositor.draw_rect(
    //     [200.0f32, 200.0, 200.0, 200.0],
    //     0.6,
    //     Color::new(0, 0, 255, 255),
    // );
    compositor.draw_rect([300.0f32, 300.0, 200.0, 200.0], 0.4, color::YELLOW);
    compositor.draw_rect([700.0f32, 500.0, 100.0, 100.0], 0.5, color::AQUA);
    let display_list = compositor.build_display_list();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    control_flow.set_exit();
                }
                WindowEvent::Resized(ref new_size)
                | WindowEvent::ScaleFactorChanged {
                    new_inner_size: &mut ref new_size,
                    ..
                } => {
                    let scale_factor = window.scale_factor() as f32;
                    if new_size.width > 0 && new_size.height > 0 {
                        println!("{}x{} @ {}", new_size.width, new_size.height, scale_factor);
                        wgpu.resize(new_size.width, new_size.height, scale_factor);
                    }
                }
                _ => {}
            },

            Event::RedrawRequested(window_id) if window_id == window.id() => {
                if wgpu.render(&mut image_cache, &display_list).is_err() {
                    control_flow.set_exit_with_code(1);
                }
            }
            Event::MainEventsCleared => {
                // window.request_redraw();
            }
            _ => {}
        }
    })
}
