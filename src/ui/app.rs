use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

use crate::core::config::{self, Config};
use crate::core::debug;
use crate::core::theme::{self, Palette};
use crate::core::vault::Vault;
use crate::ui::style::{Style, ThemeWatcher, Theming};
use crate::ui::window::{self, MainWindow};

pub const APP_ID: &str = "dev.rajo.omaday";

pub fn run() -> glib::ExitCode {
    let app = gtk::Application::builder().application_id(APP_ID).build();
    app.connect_activate(activate);
    app.run()
}

fn activate(app: &gtk::Application) {
    // A second launch reaches the running instance over D-Bus and lands here.
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            show_startup_error(app, &Config::fallback(), &e.to_string());
            return;
        }
    };
    let vault = match Vault::open(config.vault.clone()) {
        Ok(vault) => Rc::new(vault),
        Err(e) => {
            let hint = match config::home_dir() {
                Some(home) => format!(
                    " Set vault = \"/path/to/vault\" in {}.",
                    config::config_path(&home).display()
                ),
                None => String::new(),
            };
            show_startup_error(app, &config, &format!("{e}.{hint}"));
            return;
        }
    };
    let theming = install_theming(&config);
    debug(format!("vault {}", vault.root().display()));

    let main = MainWindow::new(app, vault, theming);
    main.present();
}

fn show_startup_error(app: &gtk::Application, config: &Config, message: &str) {
    eprintln!("omaday: {message}");
    // The error screen keeps the colors it started with; no watcher needed.
    let _theming = install_theming(config);
    window::error_window(app, message).present();
}

/// Load the palette, install the stylesheet, and watch for theme switches.
pub fn install_theming(config: &Config) -> Theming {
    let path = theme::palette_path();
    let palette = match path.as_deref().map(Palette::load) {
        Some(Ok(palette)) => palette,
        Some(Err(e)) => {
            eprintln!("omaday: using builtin colors: {e}");
            Palette::builtin()
        }
        None => {
            eprintln!("omaday: HOME is not set, using builtin colors");
            Palette::builtin()
        }
    };
    let style = Rc::new(Style::install(
        &palette.to_css(&config.font, config.font_size),
    ));
    let watcher = path.map(|path| {
        let style = style.clone();
        let font = config.font.clone();
        let size = config.font_size;
        let watched = path.clone();
        ThemeWatcher::start(&watched, move || match Palette::load(&path) {
            Ok(palette) => {
                style.replace(&palette.to_css(&font, size));
                debug("theme reloaded");
            }
            Err(e) => eprintln!("omaday: theme not reloaded: {e}"),
        })
    });
    Theming {
        _style: style,
        _watcher: watcher,
    }
}
