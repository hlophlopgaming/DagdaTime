#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Preference {
    System,
    Light,
    Dark,
}

impl Preference {
    pub(super) fn is_dark(self, system_mode: iced::theme::Mode) -> bool {
        match self {
            Self::System => system_mode == iced::theme::Mode::Dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }

    pub(super) fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

#[cfg(target_os = "linux")]
pub(super) fn system_mode() -> Option<iced::theme::Mode> {
    use zbus::blocking::{Connection, Proxy};

    let connection = Connection::session().ok()?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    )
    .ok()?;
    let scheme: zbus::zvariant::OwnedValue = proxy
        .call("ReadOne", &("org.freedesktop.appearance", "color-scheme"))
        .ok()?;
    let scheme = u32::try_from(scheme).ok()?;
    Some(portal_color_scheme(scheme))
}

#[cfg(target_os = "linux")]
fn portal_color_scheme(scheme: u32) -> iced::theme::Mode {
    match scheme {
        1 => iced::theme::Mode::Dark,
        2 => iced::theme::Mode::Light,
        _ => iced::theme::Mode::None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn system_preference_follows_native_theme_with_light_fallback() {
        use iced::theme::Mode;

        assert!(!super::Preference::System.is_dark(Mode::Light));
        assert!(super::Preference::System.is_dark(Mode::Dark));
        assert!(!super::Preference::System.is_dark(Mode::None));
    }

    #[test]
    fn manual_light_and_dark_override_the_system_theme() {
        use iced::theme::Mode;

        assert!(!super::Preference::Light.is_dark(Mode::Dark));
        assert!(super::Preference::Dark.is_dark(Mode::Light));
    }

    #[test]
    fn theme_button_cycles_system_light_dark() {
        assert_eq!(super::Preference::System.next(), super::Preference::Light);
        assert_eq!(super::Preference::Light.next(), super::Preference::Dark);
        assert_eq!(super::Preference::Dark.next(), super::Preference::System);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn portal_color_scheme_maps_dark_and_light_preferences() {
        use iced::theme::Mode;

        assert_eq!(super::portal_color_scheme(1), Mode::Dark);
        assert_eq!(super::portal_color_scheme(2), Mode::Light);
        assert_eq!(super::portal_color_scheme(0), Mode::None);
        assert_eq!(super::portal_color_scheme(99), Mode::None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires a desktop session portal with an explicit color preference"]
    fn reads_system_theme_from_the_desktop_portal() {
        assert_ne!(super::system_mode(), Some(iced::theme::Mode::None));
    }
}
