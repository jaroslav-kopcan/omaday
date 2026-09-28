use gtk::prelude::*;
use gtk::{gio, glib};
use gtk4 as gtk;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

/// The one application stylesheet, swappable at runtime.
pub struct Style {
    display: gtk::gdk::Display,
    provider: RefCell<gtk::CssProvider>,
}

impl Style {
    /// Load `css` for the default display. Call once GTK is initialized.
    pub fn install(css: &str) -> Style {
        let display =
            gtk::gdk::Display::default().expect("GTK is initialized and a display is open");
        let provider = gtk::CssProvider::new();
        provider.load_from_string(css);
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        Style {
            display,
            provider: RefCell::new(provider),
        }
    }

    /// Replace the stylesheet in place. Widgets restyle at once.
    pub fn replace(&self, css: &str) {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(css);
        gtk::style_context_add_provider_for_display(
            &self.display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let old = self.provider.replace(provider);
        gtk::style_context_remove_provider_for_display(&self.display, &old);
    }
}

/// Watches the palette file and its folder, calls `on_change` 200 ms after
/// the last event. Dropping it stops the watch.
pub struct ThemeWatcher {
    _monitors: Vec<gio::FileMonitor>,
}

impl ThemeWatcher {
    pub fn start(path: &Path, on_change: impl Fn() + 'static) -> ThemeWatcher {
        let on_change: Rc<dyn Fn()> = Rc::new(on_change);
        let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let folder = path.parent().unwrap_or(Path::new("/"));
        let file = gio::File::for_path(path);
        let dir = gio::File::for_path(folder);
        let results = [
            file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE),
            dir.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE),
        ];
        let mut monitors = Vec::new();
        for result in results {
            match result {
                Ok(monitor) => {
                    let on_change = on_change.clone();
                    let pending = pending.clone();
                    monitor.connect_changed(move |_, _, _, _| {
                        if let Some(id) = pending.borrow_mut().take() {
                            id.remove();
                        }
                        let on_change = on_change.clone();
                        let slot = pending.clone();
                        let id =
                            glib::timeout_add_local_once(Duration::from_millis(200), move || {
                                slot.borrow_mut().take();
                                on_change();
                            });
                        *pending.borrow_mut() = Some(id);
                    });
                    monitors.push(monitor);
                }
                Err(e) => eprintln!("omaday: cannot watch {}: {e}", path.display()),
            }
        }
        ThemeWatcher {
            _monitors: monitors,
        }
    }
}

/// Everything theme related that must stay alive for the app's lifetime.
pub struct Theming {
    pub _style: Rc<Style>,
    pub _watcher: Option<ThemeWatcher>,
}
