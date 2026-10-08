//! UA Clipboard panel (ticket 07's spec, first cut — the "prototype" the
//! user reacts to per ticket 09). Linux builds show the libadwaita window;
//! other hosts print guidance so the crate still builds for development.
//!
//! Single-instance: launching `ua-clipboard-panel` again presents the
//! existing window (GTK application activation) — that's what makes the
//! Super+V path (`ua-clipboard toggle` → daemon spawns this binary) feel
//! like a toggle.
//!
//! Status: list, server-side full-text search (daemon filters, this side
//! only highlights), pick (Select request), Esc-close, Ctrl+F search
//! focus. Pin/delete buttons, section styling, image cards and the rest
//! of the keyboard model land with the panel milestone.

#[cfg(target_os = "linux")]
fn main() {
    use gtk::prelude::*;
    use libadwaita as adw;
    use libadwaita::prelude::*;
    use ua_core::ipc::{Request, Response};

    const APP_ID: &str = "org.ua.Clipboard";

    let app = adw::Application::builder().application_id(APP_ID).build();

    // Single-instance state: second launch re-presents the same window.
    let window_slot = std::rc::Rc::new(std::cell::RefCell::new(None::<adw::ApplicationWindow>));
    let slot = window_slot.clone();

    app.connect_activate(move |app| {
        if let Some(existing) = slot.borrow().as_ref() {
            existing.present();
            return;
        }

        let win = adw::ApplicationWindow::builder()
            .application(app)
            .title("UA Clipboard")
            .default_width(420)
            .default_height(560)
            .build();

        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 8);
        box_.set_margin_top(8);
        box_.set_margin_bottom(8);
        box_.set_margin_start(8);
        box_.set_margin_end(8);

        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search clipboard"));
        box_.append(&search);

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        scroll.set_child(Some(&list));
        box_.append(&scroll);
        win.set_content(Some(&box_));

        // Ids of the rows currently displayed, parallel to row position —
        // the activate handler maps row → id through this, no string
        // smuggling through widget properties.
        let displayed_ids = std::rc::Rc::new(std::cell::RefCell::new(Vec::<u64>::new()));

        let list_r = list.clone();
        let ids_r = displayed_ids.clone();
        let rebuild = move |query: &str| {
            let mut ids = ids_r.borrow_mut();
            ids.clear();
            while let Some(child) = list_r.first_child() {
                list_r.remove(&child);
            }
            let response = ua_ipc::request(&Request::List {
                query: query.to_string(),
                offset: 0,
            });
            if let Ok(Response::Entries { entries }) = response {
                for s in entries {
                    let row = gtk::ListBoxRow::new();
                    let label = gtk::Label::new(None);
                    label.set_markup(&preview_markup(&s.preview, query));
                    label.set_halign(gtk::Align::Start);
                    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    label.set_max_width_chars(44);
                    row.set_child(Some(&label));
                    row.set_tooltip_text(Some(&relative_time(s.copied_at)));
                    ids.push(s.id);
                    list_r.append(&row);
                }
            }
        };

        // Search re-queries the daemon: full-text, case-insensitive
        // (server-side ua_core::search), this side only highlights.
        let rebuild_s = rebuild.clone();
        search.connect_changed(move |s| rebuild_s(s.text().as_str()));

        let ids_a = displayed_ids.clone();
        list.connect_row_activated(move |_l, row| {
            let idx = row.index();
            let id = ids_a.borrow().get(idx.max(0) as usize).copied();
            if let Some(id) = id {
                let _ = ua_ipc::request(&Request::Select { id });
            }
            if let Some(root) = row.root() {
                if let Ok(w) = root.downcast::<gtk::Window>() {
                    w.close();
                }
            }
        });

        // Keyboard model (ticket 07 subset): Esc closes, Ctrl+F focuses
        // the search field.
        let key = gtk::EventControllerKey::new();
        let search_k = search.clone();
        let win_k = win.clone();
        key.connect_key_pressed(move |_, keyval, _, state| {
            let name = keyval.name().unwrap_or_default();
            if name == "Escape" {
                win_k.close();
                return gtk::glib::Propagation::Stop;
            }
            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) && name == "f" {
                search_k.grab_focus();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        win.add_controller(key);

        *slot.borrow_mut() = Some(win.clone());
        rebuild("");
        win.present();
    });

    app.run();
}

/// Pango markup for a preview card: escaped text with `<b>` around
/// case-insensitive occurrences of the query (ua_core::search::highlight).
#[cfg(target_os = "linux")]
fn preview_markup(preview: &str, query: &str) -> String {
    use ua_core::search::highlight;

    let spans = highlight(preview, query);
    let mut out = String::with_capacity(preview.len() + 16);
    for span in spans {
        let chunk = &preview[span.start..span.start + span.byte_len];
        let escaped = escape_markup(chunk);
        if span.matched {
            out.push_str(&format!("<b>{escaped}</b>"));
        } else {
            out.push_str(&escaped);
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn escape_markup(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(target_os = "linux")]
fn relative_time(copied_at: u64) -> String {
    use ua_core::clock::Clock as _;
    let now = ua_core::clock::SystemClock.now_ms();
    let s = now.saturating_sub(copied_at) / 1000;
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86399 => format!("{} h ago", s / 3600),
        _ => format!("{} d ago", s / 86400),
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("ua-clipboard-panel requires Linux (GTK4 + libadwaita).");
    println!("See README.md for the Docker-based development flow.");
}
