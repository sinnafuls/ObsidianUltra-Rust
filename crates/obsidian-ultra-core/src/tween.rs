//! Time-based tweens replacing Roblox `TweenService`.

/// Easing curves used by the library (Roblox `EasingStyle`/`EasingDirection` pairs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Ease {
    #[default]
    Linear,
    QuadOut,
    QuadIn,
    QuintOut,
    QuartOut,
    SineInOut,
    BackOut,
}

/// Evaluate an easing curve at `t ∈ [0, 1]`.
pub fn ease(kind: Ease, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match kind {
        Ease::Linear => t,
        Ease::QuadOut => 1.0 - (1.0 - t) * (1.0 - t),
        Ease::QuadIn => t * t,
        Ease::QuintOut => 1.0 - (1.0 - t).powi(5),
        Ease::QuartOut => 1.0 - (1.0 - t).powi(4),
        Ease::SineInOut => -((std::f32::consts::PI * t).cos() - 1.0) / 2.0,
        Ease::BackOut => {
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
        }
    }
}

/// A scalar that animates towards a target. Cheap and `Copy`; stored inline in nodes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anim {
    from: f32,
    to: f32,
    start: f64,
    dur: f32,
    ease: Ease,
}

impl Default for Anim {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl Anim {
    /// A settled animation at `v`.
    pub const fn new(v: f32) -> Self {
        Self { from: v, to: v, start: 0.0, dur: 0.0, ease: Ease::Linear }
    }

    /// Current value at `time`.
    pub fn value(&self, time: f64) -> f32 {
        if self.dur <= 0.0 {
            return self.to;
        }
        let t = ((time - self.start) as f32 / self.dur).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * ease(self.ease, t)
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    /// Is the animation still moving at `time`?
    pub fn active(&self, time: f64) -> bool {
        self.dur > 0.0 && (time - self.start) as f32 <= self.dur && self.from != self.to
    }

    /// Start (or retarget) an animation from the current value to `to`.
    /// No-op when the target is unchanged.
    pub fn set(&mut self, to: f32, time: f64, dur: f32, ease: Ease) {
        if self.to == to {
            return;
        }
        let cur = self.value(time);
        self.from = cur;
        self.to = to;
        self.start = time;
        self.dur = dur;
        self.ease = ease;
    }

    /// Jump immediately to `v`.
    pub fn snap(&mut self, v: f32) {
        self.from = v;
        self.to = v;
        self.dur = 0.0;
    }

    /// Restart an animation from `from` to `to` regardless of the current state.
    pub fn play(&mut self, from: f32, to: f32, time: f64, dur: f32, ease: Ease) {
        self.from = from;
        self.to = to;
        self.start = time;
        self.dur = dur;
        self.ease = ease;
    }
}

/// Library-wide tween durations (seconds) and curves.
pub mod info {
    use super::Ease;

    pub const HOVER: (f32, Ease) = (0.10, Ease::QuadOut);
    pub const TAB: (f32, Ease) = (0.22, Ease::QuadOut);
    pub const WINDOW: (f32, Ease) = (0.35, Ease::QuadOut);
    pub const DROPDOWN: (f32, Ease) = (0.18, Ease::QuadOut);
    pub const KEYPICKER: (f32, Ease) = (0.15, Ease::QuadOut);
    pub const GROUPBOX: (f32, Ease) = (0.20, Ease::QuadOut);
    pub const CHEVRON: (f32, Ease) = (0.30, Ease::BackOut);
    pub const NOTIFY: (f32, Ease) = (0.25, Ease::QuadOut);
    pub const SWITCH_BALL: (f32, Ease) = (0.18, Ease::QuartOut);
    pub const SLIDER_BALL: (f32, Ease) = (0.16, Ease::SineInOut);
    pub const TOOLTIP_SHOW: (f32, Ease) = (0.22, Ease::QuintOut);
    pub const TOOLTIP_HIDE: (f32, Ease) = (0.12, Ease::QuadOut);
    pub const SUBTAB_SLIDE: (f32, Ease) = (0.25, Ease::QuintOut);
    pub const SUBTAB_HOVER: (f32, Ease) = (0.15, Ease::BackOut);
    pub const HISTORY_OPEN: (f32, Ease) = (0.24, Ease::QuintOut);
    pub const HISTORY_CLOSE: (f32, Ease) = (0.17, Ease::QuadIn);
    pub const BADGE_POP: (f32, Ease) = (0.30, Ease::BackOut);
    pub const DIALOG: (f32, Ease) = (0.10, Ease::QuadOut);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_endpoints() {
        for k in [Ease::Linear, Ease::QuadOut, Ease::QuadIn, Ease::QuintOut, Ease::QuartOut, Ease::SineInOut, Ease::BackOut] {
            assert!((ease(k, 0.0)).abs() < 1e-5, "{k:?} start");
            assert!((ease(k, 1.0) - 1.0).abs() < 1e-5, "{k:?} end");
        }
        assert!(ease(Ease::BackOut, 0.7) > 1.0, "back-out overshoots");
    }

    #[test]
    fn anim_moves_and_settles() {
        let mut a = Anim::new(0.0);
        a.set(1.0, 10.0, 1.0, Ease::Linear);
        assert!((a.value(10.5) - 0.5).abs() < 1e-5);
        assert!(a.active(10.5));
        assert_eq!(a.value(12.0), 1.0);
        assert!(!a.active(12.0));
        a.set(1.0, 12.0, 1.0, Ease::Linear);
        assert_eq!(a.value(12.0), 1.0);
        a.set(0.0, 12.0, 2.0, Ease::Linear);
        assert!((a.value(13.0) - 0.5).abs() < 1e-5);
    }
}
