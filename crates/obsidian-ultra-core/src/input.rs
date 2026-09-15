//! Backend-independent input snapshot for one frame.

use std::collections::HashSet;

use epaint::{Pos2, Vec2};

/// Keyboard keys (Roblox `Enum.KeyCode` subset that a desktop backend can deliver).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[allow(missing_docs)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Zero, One, Two, Three, Four, Five, Six, Seven, Eight, Nine,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
    Left, Right, Up, Down,
    Escape, Return, Space, Tab, Backspace, Delete, Home, End, PageUp, PageDown, Insert, CapsLock,
    LeftShift, RightShift, LeftControl, RightControl, LeftAlt, RightAlt, LeftSuper, RightSuper,
    Minus, Equals, LeftBracket, RightBracket, Backslash, Semicolon, Quote, Comma, Period, Slash, Backquote,
    KeypadZero, KeypadOne, KeypadTwo, KeypadThree, KeypadFour, KeypadFive, KeypadSix, KeypadSeven, KeypadEight, KeypadNine,
    KeypadPlus, KeypadMinus, KeypadMultiply, KeypadDivide, KeypadPeriod, KeypadEnter,
    Unknown,
}

impl Key {
    /// A single printable character for ASCII keys, else the variant name.
    pub fn display_name(self) -> String {
        if let Some(c) = self.ascii() {
            return c.to_string();
        }
        self.name().to_owned()
    }

    /// The Roblox-style variant name (`"LeftControl"`, `"F5"`, ...).
    pub fn name(self) -> &'static str {
        macro_rules! names { ($($k:ident),*) => { match self { $(Key::$k => stringify!($k)),* } } }
        names!(
            A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
            Zero, One, Two, Three, Four, Five, Six, Seven, Eight, Nine,
            F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
            F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
            Left, Right, Up, Down,
            Escape, Return, Space, Tab, Backspace, Delete, Home, End, PageUp, PageDown, Insert, CapsLock,
            LeftShift, RightShift, LeftControl, RightControl, LeftAlt, RightAlt, LeftSuper, RightSuper,
            Minus, Equals, LeftBracket, RightBracket, Backslash, Semicolon, Quote, Comma, Period, Slash, Backquote,
            KeypadZero, KeypadOne, KeypadTwo, KeypadThree, KeypadFour, KeypadFive, KeypadSix, KeypadSeven, KeypadEight, KeypadNine,
            KeypadPlus, KeypadMinus, KeypadMultiply, KeypadDivide, KeypadPeriod, KeypadEnter,
            Unknown
        )
    }

    /// Parse a variant name (case-sensitive, as produced by [`Key::name`]).
    pub fn from_name(name: &str) -> Option<Key> {
        ALL_KEYS.iter().copied().find(|k| k.name() == name)
    }

    /// Printable ASCII (Roblox KeyCode values 34..126), uppercase letters.
    pub fn ascii(self) -> Option<char> {
        use Key::*;
        Some(match self {
            A => 'A', B => 'B', C => 'C', D => 'D', E => 'E', F => 'F', G => 'G', H => 'H', I => 'I',
            J => 'J', K => 'K', L => 'L', M => 'M', N => 'N', O => 'O', P => 'P', Q => 'Q', R => 'R',
            S => 'S', T => 'T', U => 'U', V => 'V', W => 'W', X => 'X', Y => 'Y', Z => 'Z',
            Zero => '0', One => '1', Two => '2', Three => '3', Four => '4', Five => '5', Six => '6',
            Seven => '7', Eight => '8', Nine => '9',
            Minus => '-', Equals => '=', LeftBracket => '[', RightBracket => ']', Backslash => '\\',
            Semicolon => ';', Quote => '\'', Comma => ',', Period => '.', Slash => '/', Backquote => '`',
            _ => return None,
        })
    }

    /// Is this one of the eight key-picker modifier keys?
    pub fn modifier(self) -> Option<Modifier> {
        Some(match self {
            Key::LeftAlt => Modifier::LAlt,
            Key::RightAlt => Modifier::RAlt,
            Key::LeftControl => Modifier::LCtrl,
            Key::RightControl => Modifier::RCtrl,
            Key::LeftShift => Modifier::LShift,
            Key::RightShift => Modifier::RShift,
            Key::Tab => Modifier::Tab,
            Key::CapsLock => Modifier::CapsLock,
            _ => return None,
        })
    }
}

/// Every key, for name lookups.
pub const ALL_KEYS: &[Key] = &[
    Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G, Key::H, Key::I, Key::J, Key::K, Key::L, Key::M,
    Key::N, Key::O, Key::P, Key::Q, Key::R, Key::S, Key::T, Key::U, Key::V, Key::W, Key::X, Key::Y, Key::Z,
    Key::Zero, Key::One, Key::Two, Key::Three, Key::Four, Key::Five, Key::Six, Key::Seven, Key::Eight, Key::Nine,
    Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12,
    Key::F13, Key::F14, Key::F15, Key::F16, Key::F17, Key::F18, Key::F19, Key::F20, Key::F21, Key::F22, Key::F23, Key::F24,
    Key::Left, Key::Right, Key::Up, Key::Down,
    Key::Escape, Key::Return, Key::Space, Key::Tab, Key::Backspace, Key::Delete, Key::Home, Key::End,
    Key::PageUp, Key::PageDown, Key::Insert, Key::CapsLock,
    Key::LeftShift, Key::RightShift, Key::LeftControl, Key::RightControl, Key::LeftAlt, Key::RightAlt,
    Key::LeftSuper, Key::RightSuper,
    Key::Minus, Key::Equals, Key::LeftBracket, Key::RightBracket, Key::Backslash, Key::Semicolon, Key::Quote,
    Key::Comma, Key::Period, Key::Slash, Key::Backquote,
    Key::KeypadZero, Key::KeypadOne, Key::KeypadTwo, Key::KeypadThree, Key::KeypadFour, Key::KeypadFive,
    Key::KeypadSix, Key::KeypadSeven, Key::KeypadEight, Key::KeypadNine,
    Key::KeypadPlus, Key::KeypadMinus, Key::KeypadMultiply, Key::KeypadDivide, Key::KeypadPeriod, Key::KeypadEnter,
    Key::Unknown,
];

/// Key-picker modifier keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Modifier {
    LAlt,
    RAlt,
    LCtrl,
    RCtrl,
    LShift,
    RShift,
    Tab,
    CapsLock,
}

impl Modifier {
    pub fn short_name(self) -> &'static str {
        match self {
            Modifier::LAlt => "LAlt",
            Modifier::RAlt => "RAlt",
            Modifier::LCtrl => "LCtrl",
            Modifier::RCtrl => "RCtrl",
            Modifier::LShift => "LShift",
            Modifier::RShift => "RShift",
            Modifier::Tab => "Tab",
            Modifier::CapsLock => "CapsLock",
        }
    }

    pub fn key(self) -> Key {
        match self {
            Modifier::LAlt => Key::LeftAlt,
            Modifier::RAlt => Key::RightAlt,
            Modifier::LCtrl => Key::LeftControl,
            Modifier::RCtrl => Key::RightControl,
            Modifier::LShift => Key::LeftShift,
            Modifier::RShift => Key::RightShift,
            Modifier::Tab => Key::Tab,
            Modifier::CapsLock => Key::CapsLock,
        }
    }
}

/// Mouse buttons (`MB1`/`MB2`/`MB3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub const fn index(self) -> usize {
        match self {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            MouseButton::Left => "MB1",
            MouseButton::Right => "MB2",
            MouseButton::Middle => "MB3",
        }
    }

    pub const ALL: [MouseButton; 3] = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];
}

/// One frame of input. Built by the backend; consumed by [`crate::Ui::frame`].
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub pointer: Option<Pos2>,
    pub pointer_delta: Vec2,
    pub down: [bool; 3],
    pub pressed: [bool; 3],
    pub released: [bool; 3],
    pub double_clicked: [bool; 3],
    pub keys_down: HashSet<Key>,
    pub keys_pressed: Vec<Key>,
    pub keys_released: Vec<Key>,
    /// Scroll delta in points (positive `y` = scroll up).
    pub scroll: Vec2,
    /// Roblox `IsRobloxFocused`: gates clicks and drags.
    pub window_focused: bool,
    /// A backend-hosted text field currently has keyboard focus.
    pub text_edit_focused: bool,
}

impl Input {
    pub fn is_down(&self, b: MouseButton) -> bool {
        self.down[b.index()]
    }
    pub fn pressed(&self, b: MouseButton) -> bool {
        self.pressed[b.index()] && self.window_focused
    }
    pub fn released(&self, b: MouseButton) -> bool {
        self.released[b.index()]
    }
    pub fn key_down(&self, k: Key) -> bool {
        self.keys_down.contains(&k)
    }
    pub fn key_pressed(&self, k: Key) -> bool {
        self.keys_pressed.contains(&k)
    }
    pub fn ctrl_down(&self) -> bool {
        self.key_down(Key::LeftControl) || self.key_down(Key::RightControl)
    }
}
