use chrono::Datelike;
use gtk::prelude::*;
use gtk::{gio, glib, graphene};
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::calendar::{self, MONTH_ABBR, MonthView};
use crate::core::debug;
use crate::core::vault::Vault;
use crate::ui::column;
use crate::ui::day_row::DayRow;
use crate::ui::style::Theming;

/// Height of the fades over the list's top and bottom edges, in pixels.
const FADE: f64 = 48.0;

/// Run `f` once the layout queued so far has been done. Tick callbacks run
/// before the layout of their frame, so `f` waits for the second tick. `f`
/// is owned by `widget` and must hold only weak references to its owners.
fn after_layout(widget: &gtk::Widget, f: impl FnOnce(&gtk::Widget) + 'static) {
    let f = Cell::new(Some(f));
    let ticks = Cell::new(0);
    widget.add_tick_callback(move |widget, _| {
        ticks.set(ticks.get() + 1);
        if ticks.get() < 2 {
            return glib::ControlFlow::Continue;
        }
        if let Some(f) = f.take() {
            f(widget);
        }
        glib::ControlFlow::Break
    });
}

/// A window with one centered line of text. Used for startup problems.
pub fn error_window(app: &gtk::Application, message: &str) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("omaday")
        .default_width(900)
        .default_height(700)
        .build();
    window.add_css_class("omaday");
    let label = gtk::Label::new(Some(message));
    label.add_css_class("startup-error");
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label.set_halign(gtk::Align::Center);
    label.set_valign(gtk::Align::Center);
    label.set_margin_start(48);
    label.set_margin_end(48);
    window.set_child(Some(&label));
    window
}

/// Header of months, scrolling day list, footer of years.
pub struct MainWindow {
    pub window: gtk::ApplicationWindow,
    vault: Rc<Vault>,
    _theming: Theming,
    year: Cell<i32>,
    month: Cell<u32>,
    month_buttons: Vec<gtk::Button>,
    footer: gtk::Box,
    list: gtk::Box,
    viewport: gtk::Viewport,
    rows: RefCell<Vec<Rc<DayRow>>>,
    open_row: RefCell<Option<Rc<DayRow>>>,
    quit_armed: Cell<bool>,
}

impl MainWindow {
    pub fn new(app: &gtk::Application, vault: Rc<Vault>, theming: Theming) -> Rc<MainWindow> {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("omaday")
            .default_width(900)
            .default_height(700)
            .build();
        window.add_css_class("omaday");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        header.set_halign(gtk::Align::Center);
        header.set_margin_top(24);
        header.set_margin_bottom(8);
        let month_buttons: Vec<gtk::Button> = MONTH_ABBR
            .iter()
            .map(|name| {
                let button = gtk::Button::with_label(name);
                button.set_has_frame(false);
                button.add_css_class("month");
                button.add_css_class("omaday-large");
                header.append(&button);
                button
            })
            .collect();

        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        list.set_halign(gtk::Align::Fill);
        list.set_margin_top(48);
        list.set_margin_bottom(48);
        let viewport = gtk::Viewport::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        viewport.set_child(Some(&column::clamp(&list)));
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::External, gtk::PolicyType::Automatic);
        scroller.set_vexpand(true);
        scroller.set_child(Some(&viewport));
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&scroller));
        for (class, align) in [
            ("fade-top", gtk::Align::Start),
            ("fade-bottom", gtk::Align::End),
        ] {
            let fade = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            fade.add_css_class(class);
            fade.set_valign(align);
            fade.set_height_request(48);
            fade.set_can_target(false);
            overlay.add_overlay(&fade);
        }

        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        footer.set_halign(gtk::Align::Center);
        footer.set_margin_top(8);
        footer.set_margin_bottom(24);

        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.append(&header);
        column.append(&overlay);
        column.append(&footer);
        window.set_child(Some(&column));

        let today = calendar::today();
        let main = Rc::new(MainWindow {
            window,
            vault,
            _theming: theming,
            year: Cell::new(today.year()),
            month: Cell::new(today.month()),
            month_buttons,
            footer,
            list,
            viewport,
            rows: RefCell::new(Vec::new()),
            open_row: RefCell::new(None),
            quit_armed: Cell::new(false),
        });

        for (index, button) in main.month_buttons.iter().enumerate() {
            let weak = Rc::downgrade(&main);
            button.connect_clicked(move |_| {
                if let Some(main) = weak.upgrade() {
                    main.set_month(index as u32 + 1);
                }
            });
        }
        main.install_shortcuts();
        main.install_focus_handling();
        main.rebuild(true);
        main
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn set_month(self: &Rc<Self>, month: u32) {
        self.switch_to(self.year.get(), month, false);
    }

    pub fn set_year(self: &Rc<Self>, year: i32) {
        self.switch_to(year, self.month.get(), false);
    }

    pub fn step_month(self: &Rc<Self>, forward: bool) {
        let (year, month) = if forward {
            calendar::next_month(self.year.get(), self.month.get())
        } else {
            calendar::prev_month(self.year.get(), self.month.get())
        };
        self.switch_to(year, month, false);
    }

    pub fn step_year(self: &Rc<Self>, delta: i32) {
        self.switch_to(self.year.get() + delta, self.month.get(), false);
    }

    pub fn go_today(self: &Rc<Self>) {
        let today = calendar::today();
        self.switch_to(today.year(), today.month(), true);
    }

    /// Window actions with shortcuts, handled in the bubble phase so the
    /// focused widget sees the keys first. Inside the editor Ctrl+Left,
    /// Right, Up and Down keep their text navigation meaning; on a row all
    /// six shortcuts work; Ctrl+T and Ctrl+Q work everywhere.
    ///
    /// Two controllers carry the same shortcuts. The scrolled window binds
    /// Ctrl+arrows to scrolling and would take them from a focused row
    /// before they bubble up to the window, so one controller sits on the
    /// list, below it. The other is global, for the header, the footer and
    /// a window with no focus.
    fn install_shortcuts(self: &Rc<Self>) {
        type Binding = (&'static str, &'static str, fn(&Rc<MainWindow>));
        let bindings: [Binding; 6] = [
            ("prev-month", "<Control>Left", |m| m.step_month(false)),
            ("next-month", "<Control>Right", |m| m.step_month(true)),
            ("prev-year", "<Control>Down", |m| m.step_year(-1)),
            ("next-year", "<Control>Up", |m| m.step_year(1)),
            ("today", "<Control>t", |m| m.go_today()),
            ("quit", "<Control>q", |m| m.window.close()),
        ];
        let on_list = gtk::ShortcutController::new();
        on_list.set_scope(gtk::ShortcutScope::Local);
        let global = gtk::ShortcutController::new();
        global.set_scope(gtk::ShortcutScope::Global);
        for (name, accel, handler) in bindings {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(main) = weak.upgrade() {
                    handler(&main);
                }
            });
            self.window.add_action(&action);
            for controller in [&on_list, &global] {
                controller.add_shortcut(gtk::Shortcut::new(
                    gtk::ShortcutTrigger::parse_string(accel),
                    Some(gtk::NamedAction::new(&format!("win.{name}"))),
                ));
            }
        }
        for controller in [&on_list, &global] {
            controller.set_propagation_phase(gtk::PropagationPhase::Bubble);
        }
        self.list.add_controller(on_list);
        self.window.add_controller(global);
    }

    /// Show another month. Saves the open row first; when that save fails
    /// the view stays as it is so the typed text is not lost. The year is
    /// kept within 1 to 9999, well inside what the date type can hold.
    pub(crate) fn switch_to(self: &Rc<Self>, year: i32, month: u32, focus_today: bool) {
        if !self.flush_open_row() {
            return;
        }
        self.year.set(year.clamp(1, 9999));
        self.month.set(month);
        self.rebuild(focus_today);
    }

    /// Rebuild the day list for the selected month and year. Closes the open
    /// row first; callers flush it before. Scrolls to today when the month
    /// has it, else to the top; `focus_today` also moves keyboard focus there.
    fn rebuild(self: &Rc<Self>, focus_today: bool) {
        self.close_open_row();
        let (year, month) = (self.year.get(), self.month.get());
        let today = calendar::today();
        let notes = self.vault.days_with_notes(year, month).unwrap_or_else(|e| {
            eprintln!("omaday: cannot list notes: {e}");
            Vec::new()
        });
        let view = MonthView::build(year, month, today, &notes);

        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let mut rows = Vec::with_capacity(view.days.len());
        for entry in &view.days {
            let weak = Rc::downgrade(self);
            let on_toggle = move |row: &Rc<DayRow>| {
                if let Some(main) = weak.upgrade() {
                    main.toggle_row(row);
                }
            };
            let weak = Rc::downgrade(self);
            let on_saved = move || {
                if let Some(main) = weak.upgrade() {
                    // Nothing is left unsaved, so the next close is a
                    // normal one again.
                    main.quit_armed.set(false);
                    main.refresh_footer();
                }
            };
            let weak = Rc::downgrade(self);
            let on_caret_moved = move |view: &gtk::Widget, y: i32, height: i32| {
                if let Some(main) = weak.upgrade() {
                    main.reveal_caret(view, y, height);
                }
            };
            let row = DayRow::new(
                entry,
                self.vault.clone(),
                on_toggle,
                on_saved,
                on_caret_moved,
            );
            let (weak_main, weak_row) = (Rc::downgrade(self), Rc::downgrade(&row));
            row.connect_revealed(move || {
                if let (Some(main), Some(row)) = (weak_main.upgrade(), weak_row.upgrade()) {
                    main.reveal_later(&row.widget());
                }
            });
            self.list.append(&row.widget());
            rows.push(row);
        }
        *self.rows.borrow_mut() = rows;

        for (index, button) in self.month_buttons.iter().enumerate() {
            if index as u32 + 1 == month {
                button.add_css_class("selected");
            } else {
                button.remove_css_class("selected");
            }
        }
        self.refresh_footer();

        let today_row = self
            .rows
            .borrow()
            .iter()
            .find(|row| row.date == today)
            .cloned();
        match today_row {
            Some(row) => {
                let (main, weak_row) = (Rc::downgrade(self), Rc::downgrade(&row));
                after_layout(&row.widget(), move |widget| {
                    let Some(main) = main.upgrade() else { return };
                    // Focus first: GTK's own scroll to the focused widget
                    // reads the allocation, which lags behind a new value.
                    if let Some(row) = weak_row.upgrade().filter(|_| focus_today) {
                        row.focus_header();
                    }
                    main.reveal(widget);
                });
            }
            None => {
                if let Some(adjustment) = self.viewport.vadjustment() {
                    adjustment.set_value(0.0);
                }
            }
        }
        debug(format!("showing {year}-{month:02}"));
    }

    /// Open the row, or close it when it is the open one. Only one row is
    /// open at a time. When the open row cannot be saved it stays open.
    fn toggle_row(self: &Rc<Self>, row: &Rc<DayRow>) {
        let open = self.open_row.borrow_mut().take();
        if let Some(open) = open {
            if !open.flush() {
                self.reveal_later(&open.notice_widget());
                *self.open_row.borrow_mut() = Some(open);
                return;
            }
            open.close();
            if Rc::ptr_eq(&open, row) {
                row.focus_header();
                return;
            }
        }
        row.open();
        *self.open_row.borrow_mut() = Some(row.clone());
    }

    /// Flush and fold the open row. True when nothing is left unsaved.
    fn close_open_row(&self) -> bool {
        let open = self.open_row.borrow_mut().take();
        match open {
            Some(row) => {
                let saved = row.flush();
                row.close();
                saved
            }
            None => true,
        }
    }

    /// Save the open row without folding it. True when nothing is left
    /// unsaved; otherwise its notice is scrolled into view.
    fn flush_open_row(self: &Rc<Self>) -> bool {
        let open = self.open_row.borrow().clone();
        let Some(row) = open else { return true };
        if row.flush() {
            return true;
        }
        self.reveal_later(&row.notice_widget());
        false
    }

    /// `reveal` once pending layout is done, for widgets that just appeared
    /// or changed size.
    fn reveal_later(self: &Rc<Self>, widget: &gtk::Widget) {
        let main = Rc::downgrade(self);
        after_layout(widget, move |widget| {
            if let Some(main) = main.upgrade() {
                main.reveal(widget);
            }
        });
    }

    /// Scroll the list the least amount that shows `widget` with 48 px to
    /// spare above and below, clear of the fades. A widget taller than the
    /// list shows its top.
    fn reveal(&self, widget: &gtk::Widget) {
        let Some(content) = self.content_of(widget) else {
            return;
        };
        let Some(bounds) = widget.compute_bounds(&content) else {
            return;
        };
        self.reveal_rect(bounds.y() as f64, bounds.height() as f64);
    }

    /// `reveal` for the caret, given in the editor's coordinates.
    fn reveal_caret(&self, view: &gtk::Widget, y: i32, height: i32) {
        let Some(content) = self.content_of(view) else {
            return;
        };
        let point = graphene::Point::new(0.0, y as f32);
        let Some(point) = view.compute_point(&content, &point) else {
            return;
        };
        self.reveal_rect(point.y() as f64, height as f64);
    }

    /// The scrolled content, when `widget` is inside it. An idle queued
    /// before a rebuild may bring a row that is no longer in the list.
    fn content_of(&self, widget: &gtk::Widget) -> Option<gtk::Widget> {
        let content = self.viewport.child()?;
        widget.is_ancestor(&content).then_some(content)
    }

    /// Scroll so the band from `y` to `y + height`, in the coordinates of
    /// the scrolled content, is visible with 48 px margins. Content
    /// coordinates do not depend on the scroll position, so a value set
    /// since the last layout does not skew them.
    fn reveal_rect(&self, y: f64, height: f64) {
        let Some(adjustment) = self.viewport.vadjustment() else {
            return;
        };
        let value = adjustment.value();
        let page = adjustment.page_size();
        let top = y - FADE;
        let bottom = y + height + FADE;
        let target = if bottom - top > page {
            top
        } else if bottom > value + page {
            bottom - page
        } else if top < value {
            top
        } else {
            return;
        };
        let lower = adjustment.lower();
        adjustment.set_value(target.clamp(lower, (adjustment.upper() - page).max(lower)));
    }

    /// Years with notes, the current year and the selected year, ascending.
    fn refresh_footer(self: &Rc<Self>) {
        let mut years = self.vault.years_with_notes().unwrap_or_else(|e| {
            eprintln!("omaday: cannot list years: {e}");
            Vec::new()
        });
        years.push(calendar::today().year());
        years.push(self.year.get());
        years.sort_unstable();
        years.dedup();
        while let Some(child) = self.footer.first_child() {
            self.footer.remove(&child);
        }
        for year in years {
            let button = gtk::Button::with_label(&year.to_string());
            button.set_has_frame(false);
            button.add_css_class("year");
            button.add_css_class("omaday-large");
            if year == self.year.get() {
                button.add_css_class("selected");
            }
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(main) = weak.upgrade() {
                    main.set_year(year);
                }
            });
            self.footer.append(&button);
        }
    }

    /// Save when the window loses focus; guard quitting behind a successful save.
    fn install_focus_handling(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.window.connect_is_active_notify(move |window| {
            if window.is_active() {
                return;
            }
            if let Some(main) = weak.upgrade() {
                main.flush_open_row();
            }
        });

        // Deliberate strong reference: it keeps MainWindow alive exactly as
        // long as the GTK window exists. Everything else holds weak refs.
        let main = self.clone();
        self.window.connect_close_request(move |_| {
            if main.flush_open_row() {
                main.quit_armed.set(false);
                return glib::Propagation::Proceed;
            }
            if main.quit_armed.get() {
                return glib::Propagation::Proceed;
            }
            main.quit_armed.set(true);
            eprintln!("omaday: a note could not be saved; close again to quit anyway");
            // The failed flush above already queued a reveal of this notice;
            // it runs after layout, so it covers the added sentence too.
            let open = main.open_row.borrow().clone();
            if let Some(row) = open {
                row.append_notice("Close again to quit without saving.");
            }
            glib::Propagation::Stop
        });
    }
}
