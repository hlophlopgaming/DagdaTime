use super::{
    model::{self, Range, Track},
    AppIcon, Message,
};
use iced::{
    keyboard, mouse,
    widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text},
    Color, Event, Point, Rectangle, Renderer, Size, Theme,
};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::Instant,
};

fn wheel_zoom_factor(delta: mouse::ScrollDelta) -> f64 {
    let steps = match delta {
        mouse::ScrollDelta::Lines { y, .. } => f64::from(y),
        mouse::ScrollDelta::Pixels { y, .. } => f64::from(y) / 100.0,
    };
    0.9_f64.powf(steps.clamp(-10.0, 10.0))
}

pub const ROW: f32 = 45.0;
pub const RULER: f32 = 32.0;
#[derive(Default)]
pub struct PaintStats {
    pub frames: AtomicUsize,
    pub max_build_us: AtomicU64,
}
#[derive(Default)]
pub struct Interaction {
    drag: Option<Drag>,
    hover: Option<(usize, usize)>,
    shift: bool,
    last_click: Option<(Instant, Point, String)>,
    double: bool,
    tag_drag: Option<(i64, Range, i8)>,
}
struct Drag {
    x: f32,
    old: Option<Range>,
    shift: bool,
    moved: bool,
}
pub struct Timeline<'a> {
    pub tracks: &'a [Track],
    pub filtered: &'a [Vec<usize>],
    pub visible: &'a [usize],
    pub view: Range,
    pub selection: Option<Range>,
    pub highlight: Option<&'a str>,
    pub grid_cache: &'a canvas::Cache,
    pub cache: &'a canvas::Cache,
    pub stats: &'a PaintStats,
    pub icons: &'a HashMap<String, AppIcon>,
}
pub fn should_draw_app_icon(track: usize, width: f32) -> bool {
    track == 2 && width >= 28.0
}
impl Timeline<'_> {
    fn hit(&self, point: Point, width: f32) -> Option<(usize, usize)> {
        if point.y < RULER {
            return None;
        }
        let row = ((point.y - RULER) / ROW) as usize;
        let track = *self.visible.get(row)?;
        let ts = model::pixel_range(self.view, width, point.x, point.x).start;
        self.filtered[track]
            .iter()
            .rev()
            .copied()
            .find(|index| {
                let segment = &self.tracks[track].segments[*index];
                segment.range.start <= ts && segment.range.end > ts
            })
            .map(|index| (track, index))
    }
    fn extended(value: Range, old: Option<Range>) -> Range {
        old.map(|old| Range::new(old.start.min(value.start), old.end.max(value.end)))
            .unwrap_or(value)
    }
}
impl canvas::Program<Message> for Timeline<'_> {
    type State = Interaction;
    fn update(
        &self,
        state: &mut Interaction,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.shift = modifiers.shift();
            return None;
        }
        let inside = cursor.position_in(bounds);
        let point = inside.or_else(|| {
            state
                .drag
                .as_ref()
                .and_then(|_| cursor.position())
                .map(|p| {
                    Point::new(
                        (p.x - bounds.x).clamp(0.0, bounds.width),
                        (p.y - bounds.y).clamp(0.0, bounds.height),
                    )
                })
        });
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let point = inside?;
                let hit = self.hit(point, bounds.width);
                let key = hit
                    .map(|(track, index)| self.tracks[track].segments[index].key.to_string())
                    .unwrap_or_default();
                state.double = state
                    .last_click
                    .as_ref()
                    .is_some_and(|(time, position, last)| {
                        time.elapsed().as_millis() < 350
                            && position.distance(point) < 6.0
                            && *last == key
                    });
                state.last_click = Some((Instant::now(), point, key));
                state.tag_drag = hit
                    .filter(|(track, _)| *track == 0 && !state.shift)
                    .and_then(|(track, index)| {
                        let segment = &self.tracks[track].segments[index];
                        let (id, original_range) = segment.tag_record?;
                        let x = |ts| {
                            (ts - self.view.start) as f32 / self.view.duration() as f32
                                * bounds.width
                        };
                        let edge = if (point.x - x(original_range.start)).abs() < 6.0 {
                            -1
                        } else if (point.x - x(original_range.end)).abs() < 6.0 {
                            1
                        } else {
                            0
                        };
                        Some((id, original_range, edge))
                    });
                state.drag = Some(Drag {
                    x: point.x,
                    old: self.selection,
                    shift: state.shift,
                    moved: false,
                });
                if state.double {
                    if let Some((id, _, _)) = state.tag_drag {
                        return Some(
                            canvas::Action::publish(Message::Edit(super::tagging::Edit::Range(id)))
                                .and_capture(),
                        );
                    }
                    if let Some((track, index)) = hit {
                        return Some(
                            canvas::Action::publish(Message::Select(Some(model::continuous_span(
                                &self.tracks[track].segments[index],
                                &self.tracks[track].segments,
                            ))))
                            .and_capture(),
                        );
                    }
                }
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(drag) = state.drag.as_mut() {
                    let point = point?;
                    if (point.x - drag.x).abs() > 3.0 {
                        drag.moved = true;
                        let range = if let Some((_, range, edge)) = state.tag_drag {
                            let delta = ((point.x - drag.x) / bounds.width
                                * self.view.duration() as f32)
                                as i64;
                            super::tagging::drag_range(range, delta, edge)
                        } else {
                            model::pixel_range(self.view, bounds.width, drag.x, point.x)
                        };
                        return Some(
                            canvas::Action::publish(Message::Select(Some(Self::extended(
                                range,
                                if drag.shift { drag.old } else { None },
                            ))))
                            .and_capture(),
                        );
                    }
                    return Some(canvas::Action::capture());
                }
                let hit = inside.and_then(|point| self.hit(point, bounds.width));
                if hit != state.hover {
                    state.hover = hit;
                    return Some(canvas::Action::publish(Message::Hover(hit)));
                }
                None
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let drag = state.drag.take()?;
                if !state.double && drag.moved {
                    if let (Some((id, range, edge)), Some(point)) = (state.tag_drag.take(), point) {
                        let delta = ((point.x - drag.x) / bounds.width
                            * self.view.duration() as f32)
                            as i64;
                        return Some(
                            canvas::Action::publish(Message::Edit(
                                super::tagging::Edit::Transform(
                                    id,
                                    super::tagging::drag_range(range, delta, edge),
                                ),
                            ))
                            .and_capture(),
                        );
                    }
                }
                state.tag_drag = None;
                if state.double || drag.moved {
                    return Some(canvas::Action::capture());
                }
                let value = point
                    .and_then(|p| self.hit(p, bounds.width))
                    .map(|(track, index)| self.tracks[track].segments[index].range);
                Some(
                    canvas::Action::publish(Message::Select(value.map(|value| {
                        Self::extended(value, if drag.shift { drag.old } else { None })
                    })))
                    .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if inside.is_some() => {
                let (x, y) = match delta {
                    mouse::ScrollDelta::Lines { x, y } | mouse::ScrollDelta::Pixels { x, y } => {
                        (*x, *y)
                    }
                };
                if state.shift || x.abs() > y.abs() {
                    Some(
                        canvas::Action::publish(Message::PanBy(if x + y > 0.0 {
                            -0.2
                        } else {
                            0.2
                        }))
                        .and_capture(),
                    )
                } else {
                    Some(
                        canvas::Action::publish(Message::Zoom(
                            wheel_zoom_factor(*delta),
                            inside.map(|point| {
                                model::pixel_range(self.view, bounds.width, point.x, point.x).start
                            }),
                        ))
                        .and_capture(),
                    )
                }
            }
            _ => None,
        }
    }
    fn draw(
        &self,
        _state: &Interaction,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let started = Instant::now();
        let palette = theme.palette();
        let dark = palette.background.r < 0.5;
        let muted = if dark {
            Color::from_rgb8(146, 165, 179)
        } else {
            Color::from_rgb8(136, 149, 164)
        };
        let line = if dark {
            Color::from_rgb8(42, 57, 69)
        } else {
            Color::from_rgb8(237, 240, 243)
        };
        let x = |ts: i64| {
            (ts - self.view.start) as f64 / self.view.duration().max(1) as f64 * bounds.width as f64
        };
        let grid = self.grid_cache.draw(renderer, bounds.size(), |frame| {
            let span = self.view.duration();
            let tick = if span > 12 * 3_600_000 {
                2 * 3_600_000
            } else if span > 4 * 3_600_000 {
                3_600_000
            } else if span > 3_600_000 {
                30 * 60_000
            } else if span > 15 * 60_000 {
                5 * 60_000
            } else {
                60_000
            };
            let mut ts = self.view.start.div_euclid(tick) * tick;
            while ts < self.view.end {
                if ts >= self.view.start {
                    let left = x(ts) as f32;
                    frame.stroke(
                        &Path::line(Point::new(left, RULER), Point::new(left, bounds.height)),
                        Stroke::default().with_color(line),
                    );
                    frame.fill_text(Text {
                        content: model::time_label(ts),
                        position: Point::new(left + 5.0, 8.0),
                        color: muted,
                        size: 11.0.into(),
                        ..Text::default()
                    });
                }
                ts += tick;
            }
            for (row, _) in self.visible.iter().enumerate() {
                let top = RULER + row as f32 * ROW;
                frame.stroke(
                    &Path::line(
                        Point::new(0.0, top + ROW),
                        Point::new(bounds.width, top + ROW),
                    ),
                    Stroke::default().with_color(line),
                );
            }
        });
        let base = self.cache.draw(renderer, bounds.size(), |frame| {
            for (row, track) in self.visible.iter().enumerate() {
                let top = RULER + row as f32 * ROW;
                let mut previous = -1;
                for index in &self.filtered[*track] {
                    let segment = &self.tracks[*track].segments[*index];
                    let Some(range) = model::clip(segment.range, self.view) else {
                        continue;
                    };
                    let left = x(range.start) as f32;
                    let width = (x(range.end) - x(range.start)) as f32;
                    if width < 1.0 && left.floor() as i32 == previous {
                        continue;
                    }
                    previous = left.floor() as i32;
                    let color = Color {
                        r: segment.color[0],
                        g: segment.color[1],
                        b: segment.color[2],
                        a: if self
                            .highlight
                            .is_some_and(|key| key != segment.key.as_ref())
                        {
                            0.25
                        } else {
                            1.0
                        },
                    };
                    frame.fill_rectangle(
                        Point::new(left, top + 9.0),
                        Size::new(width.max(1.0), ROW - 18.0),
                        color,
                    );
                    if should_draw_app_icon(*track, width) {
                        let bounds = Rectangle::new(
                            Point::new(left + 4.0, top + 12.0),
                            Size::new(21.0, 21.0),
                        );
                        match self.icons.get(segment.key.as_ref()) {
                            Some(AppIcon::Png(handle)) => frame.draw_image(
                                bounds,
                                iced::advanced::image::Image::new(handle.clone()),
                            ),
                            Some(AppIcon::Svg(handle)) => frame
                                .draw_svg(bounds, iced::advanced::svg::Svg::new(handle.clone())),
                            None => {}
                        }
                    }
                    if width > 90.0 {
                        frame.fill_text(Text {
                            content: super::localization::segment_label(segment)
                                .chars()
                                .take(
                                    ((width
                                        - if should_draw_app_icon(*track, width) {
                                            27.0
                                        } else {
                                            0.0
                                        })
                                        / 7.0) as usize
                                        - 2,
                                )
                                .collect(),
                            position: Point::new(
                                left + 6.0
                                    + if should_draw_app_icon(*track, width) {
                                        27.0
                                    } else {
                                        0.0
                                    },
                                top + 16.0,
                            ),
                            color: Color::WHITE,
                            size: 11.0.into(),
                            ..Text::default()
                        });
                    }
                }
            }
        });
        let mut overlay = Frame::new(renderer, bounds.size());
        if let Some(range) = self
            .selection
            .and_then(|range| model::clip(range, self.view))
        {
            let left = x(range.start) as f32;
            let width = (x(range.end) - x(range.start)) as f32;
            overlay.fill_rectangle(
                Point::new(left, RULER),
                Size::new(width, bounds.height - RULER),
                Color::from_rgba(0.15, 0.56, 0.52, 0.18),
            );
            // Filled borders keep every pixel inside tiny-skia's damage bounds.
            let height = (bounds.height - RULER).max(0.0);
            let border = Color::from_rgb8(39, 143, 133);
            let edge = width.min(1.0);
            let cap = height.min(1.0);
            for (position, size) in [
                (Point::new(left, RULER), Size::new(width, cap)),
                (
                    Point::new(left, RULER + height - cap),
                    Size::new(width, cap),
                ),
                (Point::new(left, RULER), Size::new(edge, height)),
                (
                    Point::new(left + width - edge, RULER),
                    Size::new(edge, height),
                ),
            ] {
                overlay.fill_rectangle(position, size, border);
            }
        }
        self.stats.frames.fetch_add(1, Ordering::Relaxed);
        self.stats.max_build_us.fetch_max(
            started.elapsed().as_micros().min(u64::MAX as u128) as u64,
            Ordering::Relaxed,
        );
        vec![grid, base, overlay.into_geometry()]
    }
    fn mouse_interaction(
        &self,
        state: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag.is_some() || cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}
pub fn canvas<'a>(program: Timeline<'a>) -> Canvas<Timeline<'a>, Message> {
    let height = RULER + program.visible.len() as f32 * ROW;
    Canvas::new(program)
        .width(iced::Length::Fill)
        .height(height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::canvas::Program;

    #[test]
    fn selection_pixels_stay_inside_renderer_damage_bounds() {
        use iced::advanced::{graphics::geometry::Renderer as _, renderer::Headless as _};

        let program = Timeline {
            tracks: &[],
            filtered: &[],
            visible: &[],
            view: Range::new(0, 240),
            selection: Some(Range::new(50, 100)),
            highlight: None,
            grid_cache: &canvas::Cache::new(),
            cache: &canvas::Cache::new(),
            stats: &PaintStats::default(),
            icons: &HashMap::new(),
        };
        let mut renderer = Renderer::new(iced::Font::DEFAULT, 16.0.into());
        let overlay = program
            .draw(
                &Interaction::default(),
                &renderer,
                &Theme::Dark,
                Rectangle::with_size(Size::new(240.0, 212.0)),
                mouse::Cursor::Unavailable,
            )
            .pop()
            .unwrap();
        renderer.draw_geometry(overlay);
        let damage: Vec<_> = renderer
            .layers()
            .iter()
            .flat_map(|layer| &layer.primitives)
            .flat_map(|item| item.as_slice())
            .map(|primitive| primitive.visible_bounds())
            .collect();
        let pixels = renderer.screenshot(Size::new(240, 212), 1.0, Color::BLACK);
        for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
            if *pixel != [0, 0, 0, 255] {
                let point = Point::new((index % 240) as f32 + 0.5, (index / 240) as f32 + 0.5);
                assert!(
                    damage.iter().any(|bounds| bounds.contains(point)),
                    "selection pixel {point:?} lies outside repaint bounds"
                );
            }
        }
    }

    #[test]
    fn wheel_zoom_scales_with_scroll_distance() {
        let line = wheel_zoom_factor(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 });
        let pixel = wheel_zoom_factor(mouse::ScrollDelta::Pixels { x: 0.0, y: 1.0 });
        assert!((line - 0.9).abs() < 1e-9);
        assert!(pixel > 0.998 && pixel < 1.0);
        assert_eq!(
            wheel_zoom_factor(mouse::ScrollDelta::Lines { x: 0.0, y: 0.0 }),
            1.0
        );
        let reverse = wheel_zoom_factor(mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 });
        assert!((line * reverse - 1.0).abs() < 1e-9);
    }

    #[test]
    fn application_icons_only_appear_when_the_zoomed_block_can_fit_one() {
        assert!(super::should_draw_app_icon(2, 28.0));
        assert!(!super::should_draw_app_icon(2, 27.0));
        assert!(!super::should_draw_app_icon(1, 100.0));
    }

    #[test]
    fn native_canvas_drag_and_shift_extension_publish_correct_selection() {
        let tracks = model::fixture(Range::new(0, 86_400_000));
        let cache = canvas::Cache::new();
        let stats = PaintStats::default();
        let visible = [0, 1, 2, 3];
        let program = Timeline {
            tracks: &tracks,
            filtered: &tracks
                .iter()
                .map(|t| (0..t.segments.len()).collect())
                .collect::<Vec<_>>(),
            visible: &visible,
            view: Range::new(0, 86_400_000),
            selection: Some(Range::new(8 * 3_600_000, 9 * 3_600_000)),
            highlight: None,
            grid_cache: &canvas::Cache::new(),
            cache: &cache,
            stats: &stats,
            icons: &HashMap::new(),
        };
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(240.0, 212.0));
        let mut state = Interaction::default();
        program.update(
            &mut state,
            &Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::SHIFT,
            )),
            bounds,
            mouse::Cursor::Available(Point::new(100.0, 140.0)),
        );
        program.update(
            &mut state,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(Point::new(100.0, 140.0)),
        );
        let action = program
            .update(
                &mut state,
                &Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(110.0, 140.0),
                }),
                bounds,
                mouse::Cursor::Available(Point::new(110.0, 140.0)),
            )
            .unwrap();
        match action.into_inner().0 {
            Some(Message::Select(Some(range))) => {
                assert_eq!(range, Range::new(8 * 3_600_000, 11 * 3_600_000))
            }
            message => panic!("unexpected action {message:?}"),
        }
    }
}
