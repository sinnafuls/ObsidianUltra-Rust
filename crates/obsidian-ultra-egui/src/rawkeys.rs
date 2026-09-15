//! Windows raw key state via `GetAsyncKeyState` (distinguishes left/right modifiers,
//! delivers keys while a text field is not focused). No `windows` crate dependency.

use obsidian_ultra_core::input::Key;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(vkey: i32) -> i16;
}

pub struct WindowsRawKeys;

impl super::RawKeys for WindowsRawKeys {
    fn is_down(&mut self, key: Key) -> bool {
        let Some(vk) = vk(key) else { return false };
        // SAFETY: plain Win32 call with an integer argument.
        (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0
    }
}

fn vk(key: Key) -> Option<i32> {
    use Key::*;
    Some(match key {
        A => 0x41, B => 0x42, C => 0x43, D => 0x44, E => 0x45, F => 0x46, G => 0x47, H => 0x48, I => 0x49,
        J => 0x4A, K => 0x4B, L => 0x4C, M => 0x4D, N => 0x4E, O => 0x4F, P => 0x50, Q => 0x51, R => 0x52,
        S => 0x53, T => 0x54, U => 0x55, V => 0x56, W => 0x57, X => 0x58, Y => 0x59, Z => 0x5A,
        Zero => 0x30, One => 0x31, Two => 0x32, Three => 0x33, Four => 0x34, Five => 0x35, Six => 0x36,
        Seven => 0x37, Eight => 0x38, Nine => 0x39,
        F1 => 0x70, F2 => 0x71, F3 => 0x72, F4 => 0x73, F5 => 0x74, F6 => 0x75, F7 => 0x76, F8 => 0x77,
        F9 => 0x78, F10 => 0x79, F11 => 0x7A, F12 => 0x7B, F13 => 0x7C, F14 => 0x7D, F15 => 0x7E, F16 => 0x7F,
        F17 => 0x80, F18 => 0x81, F19 => 0x82, F20 => 0x83, F21 => 0x84, F22 => 0x85, F23 => 0x86, F24 => 0x87,
        Left => 0x25, Right => 0x27, Up => 0x26, Down => 0x28,
        Escape => 0x1B, Return => 0x0D, Space => 0x20, Tab => 0x09, Backspace => 0x08, Delete => 0x2E,
        Home => 0x24, End => 0x23, PageUp => 0x21, PageDown => 0x22, Insert => 0x2D, CapsLock => 0x14,
        LeftShift => 0xA0, RightShift => 0xA1, LeftControl => 0xA2, RightControl => 0xA3, LeftAlt => 0xA4, RightAlt => 0xA5,
        LeftSuper => 0x5B, RightSuper => 0x5C,
        Minus => 0xBD, Equals => 0xBB, LeftBracket => 0xDB, RightBracket => 0xDD, Backslash => 0xDC, Semicolon => 0xBA,
        Quote => 0xDE, Comma => 0xBC, Period => 0xBE, Slash => 0xBF, Backquote => 0xC0,
        KeypadZero => 0x60, KeypadOne => 0x61, KeypadTwo => 0x62, KeypadThree => 0x63, KeypadFour => 0x64,
        KeypadFive => 0x65, KeypadSix => 0x66, KeypadSeven => 0x67, KeypadEight => 0x68, KeypadNine => 0x69,
        KeypadPlus => 0x6B, KeypadMinus => 0x6D, KeypadMultiply => 0x6A, KeypadDivide => 0x6F, KeypadPeriod => 0x6E,
        KeypadEnter => return None,
        Unknown => return None,
    })
}
