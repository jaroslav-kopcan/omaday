mod core;
mod ui;

fn main() -> gtk4::glib::ExitCode {
    ui::app::run()
}
