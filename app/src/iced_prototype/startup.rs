use auto_launch::{AutoLaunch, AutoLaunchBuilder};

pub struct LaunchOptions {
    pub record_on_launch: bool,
    pub hidden: bool,
}

impl LaunchOptions {
    pub fn parse(args: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut options = Self {
            record_on_launch: false,
            hidden: false,
        };
        for arg in args {
            match arg.as_ref() {
                "--autostart" => options.record_on_launch = true,
                "--minimized" => options.hidden = true,
                _ => {}
            }
        }
        options
    }
}

pub trait StartupProvider {
    fn is_enabled(&self) -> Result<bool, String>;
    fn set_enabled(&mut self, enabled: bool) -> Result<(), String>;
}

pub struct SystemStartup(AutoLaunch);

impl SystemStartup {
    pub fn new() -> Result<Self, String> {
        let path = std::env::current_exe().map_err(|error| error.to_string())?;
        let path = path.to_string_lossy();
        let mut builder = AutoLaunchBuilder::new();
        builder
            .set_app_name("DagdaTime")
            .set_app_path(&path)
            .set_args(&["--autostart", "--minimized"])
            .set_bundle_identifiers(&["org.dagdatime.app"]);
        #[cfg(target_os = "linux")]
        builder.set_linux_launch_mode(auto_launch::LinuxLaunchMode::XdgAutostart);
        #[cfg(target_os = "macos")]
        builder.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
        #[cfg(target_os = "windows")]
        builder.set_windows_enable_mode(auto_launch::WindowsEnableMode::CurrentUser);
        builder.build().map(Self).map_err(|error| error.to_string())
    }
}

impl StartupProvider for SystemStartup {
    fn is_enabled(&self) -> Result<bool, String> {
        self.0.is_enabled().map_err(|error| error.to_string())
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
        if enabled {
            self.0.enable()
        } else {
            self.0.disable()
        }
        .map_err(|error| error.to_string())
    }
}

pub fn set_enabled(provider: &mut impl StartupProvider, enabled: bool) -> Result<bool, String> {
    provider.set_enabled(enabled)?;
    provider.is_enabled()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockStartup {
        enabled: bool,
    }

    impl StartupProvider for MockStartup {
        fn is_enabled(&self) -> Result<bool, String> {
            Ok(self.enabled)
        }

        fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
            self.enabled = enabled;
            Ok(())
        }
    }

    #[test]
    fn login_setting_updates_and_reports_provider_state() {
        let mut startup = MockStartup::default();
        assert!(!startup.is_enabled().unwrap());
        assert!(set_enabled(&mut startup, true).unwrap());
        assert!(!set_enabled(&mut startup, false).unwrap());
    }

    #[test]
    fn autostart_launches_hidden_and_starts_recording() {
        let options = LaunchOptions::parse(["--autostart", "--minimized"]);
        assert!(options.record_on_launch);
        assert!(options.hidden);

        let manual = LaunchOptions::parse(std::iter::empty::<&str>());
        assert!(!manual.record_on_launch);
        assert!(!manual.hidden);
    }
}
