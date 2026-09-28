use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk4 as gtk;

/// Widest the content column gets, in pixels.
pub const MAX_WIDTH: i32 = 720;
/// Space kept free on each side of the column, in pixels.
pub const SIDE_PADDING: i32 = 24;

/// A box holding one child, centered, at most `MAX_WIDTH` wide, with
/// `SIDE_PADDING` on both sides. Below that width the child shrinks with
/// the window down to its own minimum width.
///
/// gtk4-rs does not bind `gtk::CustomLayout`, so this is a small
/// `LayoutManager` subclass set on a plain `gtk::Box`.
pub fn clamp(child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrapper.add_css_class("column");
    wrapper.set_halign(gtk::Align::Fill);
    wrapper.set_hexpand(true);
    wrapper.set_layout_manager(Some(glib::Object::new::<ColumnLayout>()));
    wrapper.append(child);
    wrapper
}

glib::wrapper! {
    pub struct ColumnLayout(ObjectSubclass<imp::ColumnLayout>)
        @extends gtk::LayoutManager;
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct ColumnLayout;

    #[glib::object_subclass]
    impl ObjectSubclass for ColumnLayout {
        const NAME: &'static str = "OmadayColumnLayout";
        type Type = super::ColumnLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for ColumnLayout {}

    impl LayoutManagerImpl for ColumnLayout {
        fn request_mode(&self, _widget: &gtk::Widget) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(
            &self,
            widget: &gtk::Widget,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let Some(child) = widget.first_child() else {
                return (0, 0, -1, -1);
            };
            let (child_min_width, _, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
            match orientation {
                gtk::Orientation::Horizontal => {
                    let min = child_min_width + 2 * SIDE_PADDING;
                    let natural = (MAX_WIDTH + 2 * SIDE_PADDING).max(min);
                    (min, natural, -1, -1)
                }
                _ => {
                    let width = if for_size >= 0 {
                        column_width(for_size, child_min_width)
                    } else {
                        MAX_WIDTH.max(child_min_width)
                    };
                    let (min, natural, _, _) = child.measure(gtk::Orientation::Vertical, width);
                    (min, natural, -1, -1)
                }
            }
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, baseline: i32) {
            let Some(child) = widget.first_child() else {
                return;
            };
            let (child_min_width, _, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
            let w = column_width(width, child_min_width);
            let x = (width - w) / 2;
            child.size_allocate(&gtk::Allocation::new(x, 0, w, height), baseline);
        }
    }

    /// Child width for a given outer width: the outer width less padding,
    /// capped at `MAX_WIDTH`, never below the child's minimum.
    fn column_width(outer: i32, child_min_width: i32) -> i32 {
        (outer - 2 * SIDE_PADDING)
            .min(MAX_WIDTH)
            .max(child_min_width)
    }
}
