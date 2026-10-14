//! Files dropped on the home screen or the canvas open a frame later, behind an "Importing to
//! Drafts" card: opening a large `.fig` blocks the UI thread for a while, and the card is on screen
//! before it does (rather than a window that freezes without a word).

use serde_json::Value;

use crate::FigApp;
use crate::i18n::{tr, trf};
use crate::theme::{Tokens, semibold};

/// A dropped file waiting to be opened (`file.open`, a document of its own) or imported
/// (`file.import`, into the current document).
pub struct PendingOpen {
    /// What the card shows for it (the file name).
    pub label: String,
    pub params: Value,
    pub command: &'static str,
}

/// Frames the card is drawn before the files open: egui lays a new card out invisibly on its first
/// frame, so the second one is the first the window shows.
const FRAMES_BEFORE_OPENING: u8 = 2;

/// The card while files wait, then (once it has been on screen) open them. Called before the import
/// report, so a report for these files shows after them.
pub fn pending_opens(app: &mut FigApp, ctx: &egui::Context) {
    if app.ui.pending_opens.is_empty() {
        app.ui.pending_open_frames = 0;
        return;
    }
    ctx.request_repaint();
    if app.ui.pending_open_frames >= FRAMES_BEFORE_OPENING {
        app.ui.pending_open_frames = 0;
        for pending in std::mem::take(&mut app.ui.pending_opens) {
            if app.run(pending.command, pending.params).is_some() && pending.command == "file.open" {
                app.ui.home = false;
            }
        }
        return;
    }
    app.ui.pending_open_frames += 1;
    let title = match app.ui.pending_opens.first().map(|p| p.command) {
        Some("file.import") => {
            let name = app.session.doc().map(|d| d.name.clone()).unwrap_or_default();
            trf("Importing to {name}", &[("name", &name)])
        }
        _ => tr("Importing to Drafts").to_string(),
    };
    let t = Tokens::get(ctx);
    let mut modal = crate::dialogs::modal(egui::Id::new("pending_open"), ctx);
    // Shown at once: the frame after this one may block the window while the file opens.
    modal.area = modal.area.fade_in(false);
    let pending = &app.ui.pending_opens;
    modal.show(ctx, |ui| {
        ui.set_width(480.0);
        ui.label(egui::RichText::new(title).font(semibold(13.0)));
        ui.add_space(12.0);
        for p in pending {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(16.0));
                ui.label(p.label.as_str());
            });
        }
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new().size(12.0));
            ui.label(egui::RichText::new(tr("Opening the file…")).color(t.text_secondary));
        });
    });
}
