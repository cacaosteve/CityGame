//! Finger → pan / pinch-zoom / tool disambiguation for tablet input.
use glam::Vec2;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchGesture {
    /// One-finger drag delta in backing pixels (same space as mouse pan).
    Pan(Vec2),
    /// Two-finger pinch: zoom factor and midpoint anchor in backing pixels.
    Pinch { factor: f32, anchor: Vec2 },
}

#[derive(Debug, Default)]
pub struct TouchTracker {
    fingers: HashMap<u64, Vec2>,
    last_pinch_distance: Option<f32>,
    last_midpoint: Option<Vec2>,
}

impl TouchTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.fingers.clear();
        self.last_pinch_distance = None;
        self.last_midpoint = None;
    }

    pub fn finger_count(&self) -> usize {
        self.fingers.len()
    }

    pub fn on_down(&mut self, finger_id: u64, screen: Vec2) {
        self.fingers.insert(finger_id, screen);
        if self.fingers.len() >= 2 {
            self.last_pinch_distance = Some(active_distance(&self.fingers));
            self.last_midpoint = Some(active_midpoint(&self.fingers));
        } else {
            self.last_pinch_distance = None;
            self.last_midpoint = None;
        }
    }

    pub fn on_up(&mut self, finger_id: u64) {
        self.fingers.remove(&finger_id);
        if self.fingers.len() >= 2 {
            self.last_pinch_distance = Some(active_distance(&self.fingers));
            self.last_midpoint = Some(active_midpoint(&self.fingers));
        } else {
            self.last_pinch_distance = None;
            self.last_midpoint = None;
        }
    }

    /// Update finger position; returns a gesture when motion is meaningful.
    pub fn on_motion(&mut self, finger_id: u64, screen: Vec2) -> Option<TouchGesture> {
        let prev = self.fingers.insert(finger_id, screen)?;
        let delta = screen - prev;
        if self.fingers.len() >= 2 {
            let dist = active_distance(&self.fingers);
            let mid = active_midpoint(&self.fingers);
            let factor = match self.last_pinch_distance {
                Some(prev_dist) if prev_dist > 1.0 => dist / prev_dist,
                _ => 1.0,
            };
            self.last_pinch_distance = Some(dist);
            let pan = self
                .last_midpoint
                .map(|prev_mid| mid - prev_mid)
                .unwrap_or(Vec2::ZERO);
            self.last_midpoint = Some(mid);
            if (factor - 1.0).abs() >= 0.002 {
                return Some(TouchGesture::Pinch {
                    factor,
                    anchor: mid,
                });
            }
            if pan.length_squared() >= 0.25 {
                return Some(TouchGesture::Pan(pan));
            }
            return None;
        }
        if delta.length_squared() < 0.25 {
            return None;
        }
        Some(TouchGesture::Pan(delta))
    }
}

/// Mutual-exclusion arbiter for map touches: tap / pan / paint / pinch.
///
/// - **Placement tool** (`paint_stroke_ok`): a drag places immediately. A still
///   release is one tool tap. Pan that tool with two fingers.
/// - **Query / no stroke**: a still release is a tap; a drag past slop pans.
/// - **Long-press** while still, for a placement tool, also starts the stroke.
/// - **Two fingers**: pinch zoom / two-finger pan (cancels tool/paint)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapTouchPhase {
    Idle,
    Pending,
    Panning,
    Painting,
    Pinching,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapTouchAction {
    ToolTap { screen: Vec2 },
    BeginPaint { screen: Vec2 },
    PaintMove { screen: Vec2 },
    EndPaint,
    Pan { delta: Vec2 },
    Pinch { factor: f32, anchor: Vec2 },
}

#[derive(Debug)]
pub struct MapTouchArbiter {
    phase: MapTouchPhase,
    start: Vec2,
    last: Vec2,
    started_at: Instant,
    paint_stroke_ok: bool,
    pan_slop: f32,
    long_press: Duration,
}

impl Default for MapTouchArbiter {
    fn default() -> Self {
        Self::new()
    }
}

impl MapTouchArbiter {
    pub fn new() -> Self {
        Self {
            phase: MapTouchPhase::Idle,
            start: Vec2::ZERO,
            last: Vec2::ZERO,
            started_at: Instant::now(),
            paint_stroke_ok: false,
            pan_slop: 20.0,
            long_press: Duration::from_millis(280),
        }
    }

    pub fn phase(&self) -> MapTouchPhase {
        self.phase
    }

    pub fn is_active(&self) -> bool {
        self.phase != MapTouchPhase::Idle
    }

    pub fn clear(&mut self) {
        self.phase = MapTouchPhase::Idle;
        self.paint_stroke_ok = false;
    }

    /// Configure slop from backing-pixel scale (retina-friendly).
    pub fn begin_map_gesture(&mut self, screen: Vec2, paint_stroke_ok: bool, pixel_scale: f32) {
        let scale = pixel_scale.max(1.0);
        self.phase = MapTouchPhase::Pending;
        self.start = screen;
        self.last = screen;
        self.started_at = Instant::now();
        self.paint_stroke_ok = paint_stroke_ok;
        self.pan_slop = (16.0 * scale).clamp(14.0, 48.0);
        self.long_press = Duration::from_millis(280);
    }

    #[cfg(test)]
    pub fn set_long_press_for_test(&mut self, long_press: Duration) {
        self.long_press = long_press;
    }

    pub fn on_second_finger(&mut self) -> Vec<MapTouchAction> {
        let mut out = Vec::new();
        match self.phase {
            MapTouchPhase::Painting => {
                out.push(MapTouchAction::EndPaint);
                self.phase = MapTouchPhase::Pinching;
            }
            MapTouchPhase::Pending | MapTouchPhase::Panning => {
                self.phase = MapTouchPhase::Pinching;
            }
            MapTouchPhase::Pinching | MapTouchPhase::Idle => {}
        }
        out
    }

    pub fn on_motion(&mut self, screen: Vec2, finger_count: usize) -> Vec<MapTouchAction> {
        let mut out = Vec::new();
        if finger_count >= 2 {
            if self.phase == MapTouchPhase::Painting {
                out.push(MapTouchAction::EndPaint);
            }
            if self.phase != MapTouchPhase::Pinching && self.phase != MapTouchPhase::Idle {
                self.phase = MapTouchPhase::Pinching;
            }
            self.last = screen;
            return out;
        }

        let delta = screen - self.last;
        self.last = screen;
        let dist = screen.distance(self.start);
        let held = self.started_at.elapsed() >= self.long_press;

        match self.phase {
            MapTouchPhase::Idle | MapTouchPhase::Pinching => {}
            MapTouchPhase::Pending => {
                if self.paint_stroke_ok && dist >= 2.0 {
                    // Zone, road, and bulldoze drags start at the finger-down tile.
                    // One-finger movement must not become a pan for these tools.
                    self.phase = MapTouchPhase::Painting;
                    out.push(MapTouchAction::BeginPaint { screen: self.start });
                    out.push(MapTouchAction::PaintMove { screen });
                } else if held && self.paint_stroke_ok && dist < self.pan_slop {
                    self.phase = MapTouchPhase::Painting;
                    out.push(MapTouchAction::BeginPaint { screen: self.start });
                    if delta.length_squared() >= 0.25 {
                        out.push(MapTouchAction::PaintMove { screen });
                    }
                } else if !self.paint_stroke_ok && dist >= self.pan_slop {
                    // Commit to pan; camera deltas come from TouchTracker while Panning.
                    self.phase = MapTouchPhase::Panning;
                }
            }
            MapTouchPhase::Panning => {}
            MapTouchPhase::Painting => {
                if delta.length_squared() >= 0.25 || screen.distance(self.start) > 0.5 {
                    out.push(MapTouchAction::PaintMove { screen });
                }
            }
        }
        out
    }

    /// Poll pending long-press while the finger is still (no motion events).
    pub fn on_tick(&mut self) -> Vec<MapTouchAction> {
        let mut out = Vec::new();
        if self.phase != MapTouchPhase::Pending || !self.paint_stroke_ok {
            return out;
        }
        let dist = self.last.distance(self.start);
        if self.started_at.elapsed() >= self.long_press && dist < self.pan_slop {
            self.phase = MapTouchPhase::Painting;
            out.push(MapTouchAction::BeginPaint { screen: self.start });
        }
        out
    }

    pub fn on_up(&mut self) -> Vec<MapTouchAction> {
        let mut out = Vec::new();
        match self.phase {
            MapTouchPhase::Pending => {
                out.push(MapTouchAction::ToolTap { screen: self.start });
            }
            MapTouchPhase::Painting => {
                out.push(MapTouchAction::EndPaint);
            }
            MapTouchPhase::Panning | MapTouchPhase::Pinching | MapTouchPhase::Idle => {}
        }
        self.phase = MapTouchPhase::Idle;
        self.paint_stroke_ok = false;
        out
    }
}

/// SDL finger events report normalized 0..1 coordinates.
pub fn finger_to_screen(norm_x: f32, norm_y: f32, window_size: (u32, u32)) -> Vec2 {
    Vec2::new(
        norm_x.clamp(0.0, 1.0) * window_size.0 as f32,
        norm_y.clamp(0.0, 1.0) * window_size.1 as f32,
    )
}

/// True when a mouse event was synthesized from a finger (SDL_TOUCH_MOUSEID = -1).
///
/// On iOS/Android SDL delivers both `Finger*` and `Mouse*` for the same tap. Handlers that
/// toggle UI (menubar) must ignore the synthetic mouse half or the menu opens then closes.
pub fn is_touch_synthesized_mouse(which: u32) -> bool {
    which == u32::MAX
}

fn active_distance(fingers: &HashMap<u64, Vec2>) -> f32 {
    let mut iter = fingers.values();
    let Some(a) = iter.next() else {
        return 0.0;
    };
    let Some(b) = iter.next() else {
        return 0.0;
    };
    a.distance(*b)
}

fn active_midpoint(fingers: &HashMap<u64, Vec2>) -> Vec2 {
    let mut iter = fingers.values();
    let Some(a) = iter.next() else {
        return Vec2::ZERO;
    };
    let Some(b) = iter.next() else {
        return *a;
    };
    (*a + *b) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_finger_pans() {
        let mut t = TouchTracker::new();
        t.on_down(1, Vec2::new(100.0, 100.0));
        let g = t.on_motion(1, Vec2::new(120.0, 110.0));
        assert_eq!(g, Some(TouchGesture::Pan(Vec2::new(20.0, 10.0))));
    }

    #[test]
    fn two_fingers_pinch_zoom() {
        let mut t = TouchTracker::new();
        t.on_down(1, Vec2::new(100.0, 100.0));
        t.on_down(2, Vec2::new(200.0, 100.0));
        let g = t.on_motion(2, Vec2::new(240.0, 100.0));
        match g {
            Some(TouchGesture::Pinch { factor, anchor }) => {
                assert!(factor > 1.0);
                assert!((anchor.x - 170.0).abs() < 1.0);
            }
            other => panic!("expected pinch, got {other:?}"),
        }
    }

    #[test]
    fn finger_norm_maps_to_pixels() {
        assert_eq!(
            finger_to_screen(0.5, 0.25, (800, 600)),
            Vec2::new(400.0, 150.0)
        );
    }

    #[test]
    fn touch_mouse_id_is_u32_max() {
        assert!(is_touch_synthesized_mouse(u32::MAX));
        assert!(!is_touch_synthesized_mouse(0));
        assert!(!is_touch_synthesized_mouse(1));
    }

    #[test]
    fn tap_emits_tool_without_pan() {
        let mut a = MapTouchArbiter::new();
        a.begin_map_gesture(Vec2::new(50.0, 50.0), false, 2.0);
        assert!(a.on_motion(Vec2::new(52.0, 51.0), 1).is_empty());
        let up = a.on_up();
        assert_eq!(
            up,
            vec![MapTouchAction::ToolTap {
                screen: Vec2::new(50.0, 50.0)
            }]
        );
    }

    #[test]
    fn drag_past_slop_with_paint_tool_places_not_pans() {
        let mut a = MapTouchArbiter::new();
        a.begin_map_gesture(Vec2::new(50.0, 50.0), true, 1.0);
        let moved = a.on_motion(Vec2::new(50.0, 90.0), 1);
        assert_eq!(
            moved.first().copied(),
            Some(MapTouchAction::BeginPaint {
                screen: Vec2::new(50.0, 50.0)
            })
        );
        assert!(matches!(
            moved.get(1),
            Some(MapTouchAction::PaintMove { .. })
        ));
        assert_eq!(a.phase(), MapTouchPhase::Painting);
        assert_eq!(a.on_up(), vec![MapTouchAction::EndPaint]);
    }

    #[test]
    fn drag_past_slop_without_paint_tool_pans() {
        let mut a = MapTouchArbiter::new();
        a.begin_map_gesture(Vec2::new(50.0, 50.0), false, 1.0);
        let moved = a.on_motion(Vec2::new(50.0, 90.0), 1);
        assert!(moved.is_empty());
        assert_eq!(a.phase(), MapTouchPhase::Panning);
        assert!(a.on_up().is_empty());
    }

    #[test]
    fn long_press_then_move_paints() {
        let mut a = MapTouchArbiter::new();
        a.begin_map_gesture(Vec2::new(10.0, 10.0), true, 1.0);
        a.set_long_press_for_test(Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(30));
        let actions = a.on_tick();
        assert_eq!(
            actions,
            vec![MapTouchAction::BeginPaint {
                screen: Vec2::new(10.0, 10.0)
            }]
        );
        let moved = a.on_motion(Vec2::new(40.0, 10.0), 1);
        assert!(matches!(
            moved.first(),
            Some(MapTouchAction::PaintMove { .. })
        ));
        assert_eq!(a.on_up(), vec![MapTouchAction::EndPaint]);
    }

    #[test]
    fn second_finger_cancels_pending_into_pinch() {
        let mut a = MapTouchArbiter::new();
        a.begin_map_gesture(Vec2::new(10.0, 10.0), true, 1.0);
        assert!(a.on_second_finger().is_empty());
        assert_eq!(a.phase(), MapTouchPhase::Pinching);
        assert!(a.on_up().is_empty());
    }
}
