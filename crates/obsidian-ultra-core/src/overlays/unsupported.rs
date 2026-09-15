//! Unsupported-executor screen.

use epaint::Vec2;

use crate::handles::info::{DropdownInfo, GroupboxInfo, LabelInfo, TabInfo, UnsupportedInfo, WindowInfo};
use crate::handles::{Container, Label, UnsupportedScreen};
use crate::types::{DropdownDefault, DropdownValue, Side};
use crate::ui::Ui;

/// One localisation of the gate's own copy (verbatim from `reference/Library.lua:20192-20263`).
struct Lang {
    name: &'static str,
    /// `{}` = executor name.
    heading: &'static str,
    subtitle: &'static str,
    language: &'static str,
    info: [&'static str; 2],
}

const LANGUAGES: [Lang; 7] = [
    Lang {
        name: "English",
        heading: "{} is not supported",
        subtitle: "This script does not support your current executor.",
        language: "Language",
        info: [
            "Your executor lacks the proper environment for support.",
            "Use another executor such as Potassium, Volt, Real, Opiumware, Delta, etc.",
        ],
    },
    Lang {
        name: "Filipino",
        heading: "Ang {} ay hindi suportado",
        subtitle: "Hindi sinusuportahan ng script na ito ang iyong kasalukuyang executor.",
        language: "Wika",
        info: [
            "Ang iyong executor ay walang wastong kapaligiran para sa suporta.",
            "Gumamit ng ibang executor tulad ng Potassium, Volt, Real, Opiumware, Delta, atbp.",
        ],
    },
    Lang {
        name: "Tiếng Việt",
        heading: "{} không được hỗ trợ",
        subtitle: "Tập lệnh này không hỗ trợ trình thực thi hiện tại của bạn.",
        language: "Ngôn ngữ",
        info: [
            "Trình thực thi của bạn thiếu môi trường phù hợp để được hỗ trợ.",
            "Hãy dùng trình thực thi khác như Potassium, Volt, Real, Opiumware, Delta, v.v.",
        ],
    },
    Lang {
        name: "Bahasa Indonesia",
        heading: "{} tidak didukung",
        subtitle: "Skrip ini tidak mendukung executor Anda saat ini.",
        language: "Bahasa",
        info: [
            "Executor Anda tidak memiliki lingkungan yang tepat untuk didukung.",
            "Gunakan executor lain seperti Potassium, Volt, Real, Opiumware, Delta, dll.",
        ],
    },
    Lang {
        name: "Русский",
        heading: "{} не поддерживается",
        subtitle: "Этот скрипт не поддерживает ваш текущий исполнитель.",
        language: "Язык",
        info: [
            "В вашем исполнителе отсутствует нужная среда для поддержки.",
            "Используйте другой исполнитель, например Potassium, Volt, Real, Opiumware, Delta и т. д.",
        ],
    },
    Lang {
        name: "ไทย",
        heading: "ไม่รองรับ {}",
        subtitle: "สคริปต์นี้ไม่รองรับ executor ปัจจุบันของคุณ",
        language: "ภาษา",
        info: [
            "executor ของคุณไม่มีสภาพแวดล้อมที่เหมาะสมสำหรับการรองรับ",
            "ใช้ executor อื่น เช่น Potassium, Volt, Real, Opiumware, Delta ฯลฯ",
        ],
    },
    Lang {
        name: "Deutsch",
        heading: "{} wird nicht unterstützt",
        subtitle: "Dieses Skript unterstützt deinen aktuellen Executor nicht.",
        language: "Sprache",
        info: [
            "Deinem Executor fehlt die passende Umgebung für Unterstützung.",
            "Verwende einen anderen Executor wie Potassium, Volt, Real, Opiumware, Delta usw.",
        ],
    },
];

fn lang_by_name(name: &str) -> &'static Lang {
    LANGUAGES.iter().find(|l| l.name == name).unwrap_or(&LANGUAGES[0])
}

impl Ui {
    /// Returns `None` when the executor is supported (or a window already exists).
    pub fn create_unsupported_screen(&self, info: UnsupportedInfo) -> Option<UnsupportedScreen> {
        let exec = info.executor.to_lowercase();
        let matches = |list: &Vec<String>| list.iter().any(|s| exec.contains(&s.to_lowercase()));
        let unsupported = match (&info.supported, &info.unsupported) {
            (Some(s), _) => !matches(s),
            (None, Some(u)) => matches(u),
            (None, None) => true,
        };
        if !unsupported || self.with(|m| m.window.is_some()) {
            return None;
        }
        let executor = info.executor;

        // Locked-down window.
        let mut winfo = WindowInfo {
            title: info.title,
            icon: info.icon,
            footer: info.footer.unwrap_or_default(),
            size: Vec2::new(660.0, 320.0),
            center: true,
            auto_show: true,
            resizable: false,
            enable_sidebar_resize: false,
            minimizable: false,
            disable_search: true,
            disable_notification_bell: true,
            show_custom_cursor: false,
            always_on_top: info.always_on_top,
            ..Default::default()
        };
        if let Some(f) = info.font {
            winfo.font = f;
        }
        if let Some(r) = info.corner_radius {
            winfo.corner_radius = r;
        }
        let window = self.create_window(winfo);
        let tab = window.add_tab(TabInfo { name: "Unsupported".to_owned(), icon: Some("shield-alert".into()), ..Default::default() });

        // Executor: heading, subtitle, language picker.
        let heading = tab.add_groupbox(GroupboxInfo { side: Side::Left, name: "Executor".to_owned(), icon: Some("shield-alert".into()), ..Default::default() });
        let heading_label = heading.add_label(LabelInfo::new("").wrap());
        let subtitle_label = heading.add_label(LabelInfo::new("").wrap());
        let dropdown = heading.add_dropdown(
            "UnsupportedLanguage",
            DropdownInfo { text: Some("Language".to_owned()), values: LANGUAGES.iter().map(|l| l.name.into()).collect(), default: DropdownDefault::Index(0), ..Default::default() },
        );

        // Information: custom numbered points (verbatim) or two localised labels.
        let info_box = tab.add_groupbox(GroupboxInfo { side: Side::Right, name: "Information".to_owned(), icon: Some("info".into()), ..Default::default() });
        let mut default_labels: Vec<Label> = Vec::new();
        match &info.information {
            Some(points) => {
                for (i, p) in points.iter().enumerate() {
                    let n = i + 1;
                    match p.title.as_deref().filter(|t| !t.is_empty()) {
                        Some(t) => {
                            info_box.add_label(LabelInfo::new(format!("{n}. {t}")).wrap());
                            info_box.add_label(LabelInfo::new(p.text.clone()).wrap());
                        }
                        None => {
                            info_box.add_label(LabelInfo::new(format!("{n}. {}", p.text)).wrap());
                        }
                    }
                }
            }
            None => {
                for _ in 0..2 {
                    default_labels.push(info_box.add_label(LabelInfo::new("").wrap()));
                }
            }
        }

        let apply = {
            let (heading_label, subtitle_label, dropdown, executor) = (heading_label, subtitle_label, dropdown.clone(), executor.clone());
            move |name: &str| {
                let lang = lang_by_name(name);
                heading_label.set_text(&lang.heading.replace("{}", &executor));
                subtitle_label.set_text(lang.subtitle);
                dropdown.set_text(Some(lang.language));
                for (i, l) in default_labels.iter().enumerate() {
                    l.set_text(&format!("{}. {}", i + 1, lang.info[i]));
                }
            }
        };
        apply("English");
        dropdown.on_changed(move |v| {
            if let DropdownValue::Single(Some(name)) = v {
                apply(name);
            }
        });

        Some(UnsupportedScreen { ui: self.clone(), executor, window })
    }
}
