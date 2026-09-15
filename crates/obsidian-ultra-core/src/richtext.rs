//! Minimal Roblox-RichText-compatible markup: `<b>`, `<i>`, `<u>`, `<s>`,
//! `<font color="#rrggbb">`, `<br/>`. Unknown tags are stripped.

use epaint::Color32;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub color: Option<Color32>,
}

#[derive(Clone, Copy, Default)]
struct Style {
    bold: u8,
    italic: u8,
    underline: u8,
    strike: u8,
}

/// Parse markup into styled spans. Text without any `<` returns a single span.
pub fn parse(src: &str) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    if !src.contains('<') && !src.contains('&') {
        spans.push(Span { text: src.to_owned(), ..Default::default() });
        return spans;
    }
    let mut style = Style::default();
    let mut colors: Vec<Color32> = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, spans: &mut Vec<Span>, style: Style, colors: &[Color32]| {
        if cur.is_empty() {
            return;
        }
        spans.push(Span {
            text: std::mem::take(cur),
            bold: style.bold > 0,
            italic: style.italic > 0,
            underline: style.underline > 0,
            strike: style.strike > 0,
            color: colors.last().copied(),
        });
    };
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'<' {
            let Some(end) = src[i..].find('>') else {
                cur.push('<');
                i += 1;
                continue;
            };
            let tag = &src[i + 1..i + end];
            i += end + 1;
            let (closing, body) = match tag.strip_prefix('/') {
                Some(b) => (true, b.trim()),
                None => (false, tag.trim()),
            };
            let name = body.split_whitespace().next().unwrap_or("").trim_end_matches('/').to_ascii_lowercase();
            let adj = |v: &mut u8, closing: bool| {
                if closing {
                    *v = v.saturating_sub(1);
                } else {
                    *v += 1;
                }
            };
            match name.as_str() {
                "b" => {
                    flush(&mut cur, &mut spans, style, &colors);
                    adj(&mut style.bold, closing);
                }
                "i" => {
                    flush(&mut cur, &mut spans, style, &colors);
                    adj(&mut style.italic, closing);
                }
                "u" => {
                    flush(&mut cur, &mut spans, style, &colors);
                    adj(&mut style.underline, closing);
                }
                "s" => {
                    flush(&mut cur, &mut spans, style, &colors);
                    adj(&mut style.strike, closing);
                }
                "br" => cur.push('\n'),
                "font" => {
                    flush(&mut cur, &mut spans, style, &colors);
                    if closing {
                        colors.pop();
                    } else {
                        let color = body
                            .split_whitespace()
                            .filter_map(|attr| attr.split_once('='))
                            .find(|(k, _)| k.eq_ignore_ascii_case("color"))
                            .and_then(|(_, v)| crate::color::parse_hex(v.trim_matches(|c| c == '"' || c == '\'')));
                        colors.push(color.unwrap_or(colors.last().copied().unwrap_or(Color32::PLACEHOLDER)));
                    }
                }
                _ => {}
            }
            continue;
        }
        if c == b'&' {
            let rest = &src[i..];
            let ent = [("&lt;", '<'), ("&gt;", '>'), ("&amp;", '&'), ("&quot;", '"'), ("&apos;", '\'')]
                .iter()
                .find(|(e, _)| rest.starts_with(e));
            if let Some((e, ch)) = ent {
                cur.push(*ch);
                i += e.len();
                continue;
            }
        }
        let ch_len = utf8_len(c);
        cur.push_str(&src[i..i + ch_len]);
        i += ch_len;
    }
    flush(&mut cur, &mut spans, style, &colors);
    spans
}

/// Plain text with all markup removed (used for search and measurements).
pub fn strip(src: &str) -> String {
    if !src.contains('<') && !src.contains('&') {
        return src.to_owned();
    }
    let mut out = String::with_capacity(src.len());
    for span in parse(src) {
        out.push_str(&span.text);
    }
    out
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_tags_and_entities() {
        let spans = parse("Hi <b>bold <i>both</i></b> &lt;x&gt; <font color=\"#ff0000\">red</font> <unknown>plain");
        assert_eq!(spans[0].text, "Hi ");
        assert!(spans[1].bold && !spans[1].italic && spans[1].text == "bold ");
        assert!(spans[2].bold && spans[2].italic && spans[2].text == "both");
        assert_eq!(spans[3].text, " <x> ");
        assert_eq!(spans[4].color, Some(Color32::from_rgb(255, 0, 0)));
        assert_eq!(spans[5].text, " plain");
        assert_eq!(strip("<b>Tab</b> name"), "Tab name");
        assert_eq!(strip("no markup"), "no markup");
    }
}
