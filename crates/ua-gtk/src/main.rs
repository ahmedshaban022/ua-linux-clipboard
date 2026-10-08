//! UA Clipboard panel (ticket 07's spec, first cut — the "prototype" the
//! user reacts to per ticket 09). Linux builds show the libadwaita window;
//! other hosts print guidance so the crate still builds for development.
//!
//! Status: list + search + pick (Select request) + Esc-close are wired to
//! the daemon over IPC. Pin/delete buttons, section styling, image cards
//! and the full keyboard model land with the panel milestone.

#[cfg(target_os = "linux")]
fn main() {
    use gtk::prelude::*;
    use libadwaita as adw;
    use libadwaita::prelude::*;
    use ua_core::ipc::{EntrySummary, Request, Response};

    const APP_ID: &str = "org.ua.Clipboard";

    let app = adw::Application::builder().application_id(APP_ID).build();

    app.connect_activate(|app| {
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

        let summaries: std::rc::Rc<std::cell::RefCell<Vec<EntrySummary>>> =
            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));

        let list_r = list.clone();
        let summaries_r = summaries.clone();
        let rebuild = move |query: String| {
            let q = query.trim().to_lowercase();
            let all = summaries_r.borrow().clone();
            while let Some(child) = list_r.first_child() {
                list_r.remove(&child);
            }
            for s in all
                .iter()
                .filter(|s| q.is_empty() || s.preview.to_lowercase().contains(&q))
            {
                let row = gtk::ListBoxRow::new();
                let shown = if s.pinned {
                    format!("📌 {}", s.preview)
                } else {
                    s.preview.clone()
                };
                let label = gtk::Label::new(Some(&shown));
                label.set_halign(gtk::Align::Start);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.set_max_width_chars(44);
                row.set_child(Some(&label));
                row.set_tooltip_text(Some(&relative_time(s.copied_at)));
                row.set_property("name", s.id.to_string());
                list_r.append(&row);
            }
        };

        let rebuild_s = rebuild.clone();
        search.connect_changed(move |s| rebuild_s(s.text().to_string()));

        list.connect_row_activated(move |_l, row| {
            let name: String = row.property("name");
            if let Ok(id) = name.parse::<u64>() {
                let _ = ipc_request(&Request::Select { id });
            }
            if let Some(root) = row.root() {
                if let Ok(w) = root.downcast::<gtk::Window>() {
                    w.close();
                }
            }
        });

        // Initial load: pull the list, then render.
        match ipc_request(&Request::List {
            query: String::new(),
        }) {
            Ok(Response::Entries { items }) => *summaries.borrow_mut() = items,
            Ok(Response::Error(e)) => eprintln!("daemon error: {e}"),
            Ok(_) => {}
            Err(e) => eprintln!("{e}"),
        }
        rebuild(String::new());
        win.present();
    });

    app.run();
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

#[cfg(target_os = "linux")]
fn ipc_request(req: &ua_core::ipc::Request) -> Result<ua_core::ipc::Response, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let path = std::env::var("UA_CLIPBOARD_SOCKET").unwrap_or_else(|_| {
        let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        format!("{runtime}/ua-clipboard.sock")
    });
    let mut stream = UnixStream::connect(&path)
        .map_err(|e| format!("connect {path}: {e} — is ua-clipboard-daemon running?"))?;
    let json = serde_json::to_string(req).map_err(|e| e.to_string())?;
    writeln!(stream, "{json}").map_err(|e| e.to_string())?;
    stream.flush().ok();
    let mut line = String::new();
    let mut reader = BufReader::new(stream);
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    serde_json::from_str(line.trim()).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("ua-clipboard-panel requires Linux (GTK4 + libadwaita).");
    println!("See README.md for the Docker-based development flow.");
}
