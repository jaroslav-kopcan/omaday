use chrono::NaiveDate;
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use crate::core::calendar::{self, DayEntry};
use crate::core::debug;
use crate::core::saver::{Action, Saver};
use crate::core::vault::Vault;

/// Delay between the last keystroke and the save.
pub const SAVE_DELAY: Duration = Duration::from_secs(2);

/// Caret callback: the editor, then the caret's top and height in it.
type CaretMoved = dyn Fn(&gtk::Widget, i32, i32);

/// One day in the list: a header button and a fold out editor below it.
pub struct DayRow {
    pub date: NaiveDate,
    root: gtk::Box,
    header: gtk::Button,
    revealer: gtk::Revealer,
    view: gtk::TextView,
    notice: gtk::Label,
    vault: Rc<Vault>,
    saver: RefCell<Saver>,
    modified: Cell<Option<SystemTime>>,
    timer: RefCell<Option<glib::SourceId>>,
    loading: Cell<bool>,
    read_only: Cell<bool>,
    caret_queued: Cell<bool>,
    on_saved: Box<dyn Fn()>,
    on_caret_moved: Box<CaretMoved>,
}

impl DayRow {
    /// `on_toggle` runs when the header is clicked or Escape is pressed in the
    /// editor; the caller decides whether that opens or closes the row.
    /// `on_saved` runs after every successful write or delete.
    /// `on_caret_moved` runs when the caret of the focused editor moved; it
    /// gets the editor and the caret's top and height in its coordinates.
    pub fn new(
        entry: &DayEntry,
        vault: Rc<Vault>,
        on_toggle: impl Fn(&Rc<DayRow>) + 'static,
        on_saved: impl Fn() + 'static,
        on_caret_moved: impl Fn(&gtk::Widget, i32, i32) + 'static,
    ) -> Rc<DayRow> {
        let number = gtk::Label::new(Some(&format!("{:02}.", entry.number)));
        number.add_css_class("number");
        number.add_css_class("omaday-large");
        number.set_valign(gtk::Align::Baseline);
        let weekday = gtk::Label::new(Some(entry.weekday));
        weekday.add_css_class("omaday-body");
        weekday.set_valign(gtk::Align::Baseline);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.append(&number);
        line.append(&weekday);

        let header = gtk::Button::new();
        header.set_child(Some(&line));
        header.set_has_frame(false);
        header.set_halign(gtk::Align::Start);
        header.add_css_class("day");
        if entry.has_note {
            header.add_css_class("has-note");
        }
        if entry.is_today {
            header.add_css_class("today");
        }

        let view = gtk::TextView::new();
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_left_margin(0);
        view.set_right_margin(0);
        view.set_top_margin(8);
        view.set_bottom_margin(16);
        view.add_css_class("editor");

        let notice = gtk::Label::new(None);
        notice.add_css_class("notice");
        notice.add_css_class("omaday-body");
        notice.set_xalign(0.0);
        notice.set_wrap(true);
        notice.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        // Keep the natural width small so a long notice wraps to the column
        // instead of widening it.
        notice.set_max_width_chars(1);
        notice.set_visible(false);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
        body.add_css_class("editor-body");
        body.append(&view);
        body.append(&notice);

        let revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideDown)
            .transition_duration(200)
            .child(&body)
            .build();

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&header);
        root.append(&revealer);

        let row = Rc::new(DayRow {
            date: entry.date,
            root,
            header,
            revealer,
            view,
            notice,
            vault,
            saver: RefCell::new(Saver::new(None, SAVE_DELAY)),
            modified: Cell::new(None),
            timer: RefCell::new(None),
            loading: Cell::new(false),
            read_only: Cell::new(false),
            caret_queued: Cell::new(false),
            on_saved: Box::new(on_saved),
            on_caret_moved: Box::new(on_caret_moved),
        });

        let on_toggle = Rc::new(on_toggle);

        let weak = Rc::downgrade(&row);
        let toggle = on_toggle.clone();
        row.header.connect_clicked(move |_| {
            if let Some(row) = weak.upgrade() {
                toggle(&row);
            }
        });

        let weak = Rc::downgrade(&row);
        row.view.buffer().connect_changed(move |buffer| {
            let Some(row) = weak.upgrade() else { return };
            if row.loading.get() || row.read_only.get() {
                return;
            }
            let (start, end) = buffer.bounds();
            let text = buffer.text(&start, &end, true).to_string();
            let now = Instant::now();
            row.saver.borrow_mut().edited(text, now);
            row.arm_timer(now);
            row.queue_caret();
        });

        let weak = Rc::downgrade(&row);
        row.view.buffer().connect_mark_set(move |buffer, _, mark| {
            if *mark != buffer.get_insert() {
                return;
            }
            if let Some(row) = weak.upgrade() {
                row.queue_caret();
            }
        });

        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&row);
        keys.connect_key_pressed(move |_, key, _, _| {
            if key != gtk::gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            if let Some(row) = weak.upgrade() {
                on_toggle(&row);
            }
            glib::Propagation::Stop
        });
        row.view.add_controller(keys);

        row
    }

    pub fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    pub fn focus_header(&self) {
        self.header.grab_focus();
    }

    pub fn notice_widget(&self) -> gtk::Widget {
        self.notice.clone().upcast()
    }

    /// Run `f` each time the editor has finished unfolding. `f` must hold
    /// only weak references: the revealer owns it.
    pub fn connect_revealed(&self, f: impl Fn() + 'static) {
        self.revealer
            .connect_child_revealed_notify(move |revealer| {
                if revealer.is_child_revealed() {
                    f();
                }
            });
    }

    /// Read the file from disk and unfold the editor.
    pub fn open(self: &Rc<Self>) {
        self.notice.set_visible(false);
        self.read_only.set(false);
        self.view.set_visible(true);
        let buffer = self.view.buffer();
        self.loading.set(true);
        match self.vault.read(self.date) {
            Ok(Some(note)) => {
                self.modified.set(Some(note.modified));
                *self.saver.borrow_mut() = Saver::new(Some(note.text.clone()), SAVE_DELAY);
                set_text_silently(&buffer, &note.text);
            }
            Ok(None) => {
                self.modified.set(None);
                *self.saver.borrow_mut() = Saver::new(None, SAVE_DELAY);
                set_text_silently(&buffer, "");
            }
            Err(e) => {
                self.read_only.set(true);
                set_text_silently(&buffer, "");
                self.view.set_visible(false);
                self.show_notice(&format!("Cannot open this day: {e}"));
            }
        }
        self.loading.set(false);
        self.header.add_css_class("open");
        self.revealer.set_reveal_child(true);
        if !self.read_only.get() {
            let weak = Rc::downgrade(self);
            glib::idle_add_local_once(move || {
                if let Some(row) = weak.upgrade() {
                    row.view.grab_focus();
                }
            });
        }
        debug(format!("opened {}", self.date));
    }

    /// Fold the editor. Callers flush first; a pending timer still fires.
    pub fn close(&self) {
        self.revealer.set_reveal_child(false);
        self.header.remove_css_class("open");
    }

    /// Save now when there is something to save. True when nothing is left unsaved.
    pub fn flush(&self) -> bool {
        self.cancel_timer();
        let action = self.saver.borrow_mut().flush();
        self.perform(action)
    }

    /// Report the caret position once layout is idle. Coalesces the several
    /// signals one keystroke emits into one report.
    fn queue_caret(self: &Rc<Self>) {
        if self.caret_queued.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(row) = weak.upgrade() else { return };
            row.caret_queued.set(false);
            if !row.view.is_focus() {
                return;
            }
            let buffer = row.view.buffer();
            let iter = buffer.iter_at_mark(&buffer.get_insert());
            let location = row.view.iter_location(&iter);
            let (_, y) = row.view.buffer_to_window_coords(
                gtk::TextWindowType::Widget,
                location.x(),
                location.y(),
            );
            (row.on_caret_moved)(row.view.upcast_ref(), y, location.height());
        });
    }

    fn arm_timer(self: &Rc<Self>, now: Instant) {
        self.cancel_timer();
        let wait = self.saver.borrow().remaining(now).unwrap_or(SAVE_DELAY);
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(wait, move || {
            let Some(row) = weak.upgrade() else { return };
            row.timer.borrow_mut().take();
            let now = Instant::now();
            let action = row.saver.borrow_mut().tick(now);
            if action == Action::Nothing && row.saver.borrow().is_dirty() {
                // Fired a little early; wait out the rest of the delay.
                row.arm_timer(now);
                return;
            }
            row.perform(action);
        });
        *self.timer.borrow_mut() = Some(id);
    }

    fn cancel_timer(&self) {
        if let Some(id) = self.timer.borrow_mut().take() {
            id.remove();
        }
    }

    /// Carry out a saver decision. True when the disk now matches the buffer.
    fn perform(&self, action: Action) -> bool {
        match action {
            Action::Nothing => true,
            Action::Write(text) => match self.vault.write(self.date, &text, self.modified.get()) {
                Ok(modified) => {
                    self.modified.set(Some(modified));
                    self.saver.borrow_mut().written(text);
                    self.header.add_css_class("has-note");
                    self.after_change("saved");
                    true
                }
                Err(e) => {
                    self.show_notice(&format!("Could not save: {e}"));
                    false
                }
            },
            Action::Delete => match self.vault.delete(self.date, self.modified.get()) {
                Ok(()) => {
                    self.modified.set(None);
                    self.saver.borrow_mut().deleted();
                    self.header.remove_css_class("has-note");
                    self.after_change("removed empty note for");
                    true
                }
                Err(e) => {
                    self.show_notice(&format!("Could not remove the empty note: {e}"));
                    false
                }
            },
        }
    }

    fn after_change(&self, what: &str) {
        self.notice.set_visible(false);
        if self.date == calendar::today() {
            self.header.add_css_class("today");
        } else {
            self.header.remove_css_class("today");
        }
        debug(format!("{what} {}", self.date));
        (self.on_saved)();
    }

    fn show_notice(&self, text: &str) {
        self.notice.set_text(text);
        self.notice.set_visible(true);
    }

    /// Add a sentence to the notice, after what it already says.
    pub fn append_notice(&self, text: &str) {
        let current = self.notice.text();
        if !self.notice.is_visible() || current.is_empty() {
            self.show_notice(text);
        } else if current.ends_with('.') {
            self.show_notice(&format!("{current} {text}"));
        } else {
            self.show_notice(&format!("{current}. {text}"));
        }
    }
}

/// Set buffer text without adding an undo step.
fn set_text_silently(buffer: &gtk::TextBuffer, text: &str) {
    buffer.begin_irreversible_action();
    buffer.set_text(text);
    buffer.end_irreversible_action();
}
