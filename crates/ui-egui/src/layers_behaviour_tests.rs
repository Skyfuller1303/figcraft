//! Layers panel behaviour suite (the gestures), written from the public documentation of design
//! tools and driven through egui headless: clicking, Shift ranges and Cmd/Ctrl toggles, expanding
//! and collapsing, sweeping across the eyes and padlocks, renaming several layers, and the pages
//! list (dragging pages, dividers). Rows the documentation is silent on say so and pin today's
//! behaviour. What the gestures do to the document is in the engine's `panels_behaviour_tests`.

use egui::{Event, Modifiers, Pos2};
use figcraft_doc::NodeId;
use figcraft_engine::Session;
use serde_json::json;

use crate::layers_drag_tests::Ui;
use crate::{FigApp, NoServices};

fn id(v: serde_json::Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

const SHIFT: Modifiers = Modifiers { alt: false, ctrl: false, shift: true, mac_cmd: false, command: false };
const CMD: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
const ALT: Modifiers = Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: false };

/// A click at `p` with `m` held, then a pause (so the next click is no double click).
fn click(u: &mut Ui, p: Pos2, m: Modifiers) {
    let button = |pressed| Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: m };
    u.frame_with(vec![Event::PointerMoved(p)], m);
    u.frame_with(vec![button(true)], m);
    u.frame_with(vec![button(false)], m);
    for _ in 0..30 {
        u.frame(vec![]);
    }
}

fn rect(s: &mut Session, name: &str, x: f64, parent: Option<NodeId>) -> NodeId {
    let mut p = json!({"x": x, "y": 0, "width": 20, "height": 20, "props": {"name": name}});
    if let Some(parent) = parent {
        p["parent"] = json!(parent);
    }
    id(s.execute("shape.rectangle", &p).unwrap())
}

/// Four rectangles `a`…`d` on the page (`d` on top: the panel's first row).
fn four() -> (Ui, [NodeId; 4]) {
    let mut s = Session::new();
    let ids = ["a", "b", "c", "d"].map(|n| rect(&mut s, n, 0.0, None));
    s.execute("select.none", &json!({})).unwrap();
    (Ui::new(s), ids)
}

fn sorted(mut v: Vec<NodeId>) -> Vec<NodeId> {
    v.sort();
    v
}

#[test]
fn clicking_rows_selects_by_the_documented_rules() {
    let (mut u, [a, b, c, d]) = four();
    let rows = [a, b, c, d].map(|i| u.row_at(i));
    let [ra, rb, rc, rd] = rows;
    // (row, modifiers, selection after, derivation)
    let steps: [(Pos2, Modifiers, Vec<NodeId>, &str); 5] = [
        (rc, Modifiers::NONE, vec![c], "a click on a layer's row selects it (on the canvas too)"),
        (ra, SHIFT, vec![a, b, c], "Shift-click selects every row between the first and the last"),
        (rd, CMD, vec![a, b, c, d], "Cmd/Ctrl-click adds a row that isn't next to the others"),
        (rb, CMD, vec![a, c, d], "silent: Cmd/Ctrl-click on a selected row takes it out (pinned)"),
        (rb, Modifiers::NONE, vec![b], "a plain click selects only that row"),
    ];
    for (p, m, after, why) in steps {
        click(&mut u, p, m);
        assert_eq!(sorted(u.app.session.selection()), sorted(after), "{why}");
    }
}

#[test]
fn expanding_collapsing_and_revealing() {
    let mut s = Session::new();
    let f = id(s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 200, "height": 200, "name": "F"})).unwrap());
    let inner = rect(&mut s, "inner", 10.0, Some(f));
    let g = id(s.execute("object.group", &json!({"ids": [inner]})).unwrap());
    let h = id(s.execute("frame.create", &json!({"x": 300, "y": 0, "width": 100, "height": 100, "name": "H"})).unwrap());
    rect(&mut s, "h1", 10.0, Some(h));
    s.execute("select.none", &json!({})).unwrap();
    let mut u = Ui::new(s);
    u.app.ui.expanded.clear();
    u.frame(vec![]);
    // Alt-click on a disclosure arrow opens everything inside (silent, pinned); again closes it.
    let row = u.row_at(f);
    // The arrow sits at the row's left, after the panel's inset.
    let arrow = Pos2::new(74.0, row.y);
    click(&mut u, arrow, ALT);
    assert!(u.app.ui.expanded.contains(&f) && u.app.ui.expanded.contains(&g), "silent: Alt-click on an arrow expands every layer inside (pinned)");
    click(&mut u, arrow, ALT);
    assert!(!u.app.ui.expanded.contains(&f) && !u.app.ui.expanded.contains(&g), "silent: Alt-click again collapses them all (pinned)");
    // Selecting on the canvas opens the rows down to the layer (silent, pinned).
    u.app.run("select.set", json!({"ids": [inner]}));
    u.frame(vec![]);
    assert!(
        u.app.ui.expanded.contains(&f) && u.app.ui.expanded.contains(&g),
        "silent: a layer selected on the canvas is revealed in the panel (pinned)"
    );
    // Collapse layers: everything closes except the way to the selection.
    u.app.ui.expanded.insert(h);
    u.app.run("layers.collapseAll", json!({}));
    u.frame(vec![]);
    assert!(!u.app.ui.expanded.contains(&h), "Collapse layers closes the expanded layers");
    assert!(u.app.ui.expanded.contains(&f) && u.app.ui.expanded.contains(&g), "the selected layer's rows stay open");
    u.app.run("select.none", json!({}));
    u.app.run("layers.collapseAll", json!({}));
    u.frame(vec![]);
    assert!(u.app.ui.expanded.is_empty(), "with nothing selected every layer collapses");
}

#[test]
fn sweeping_across_eyes_and_padlocks_sets_several_layers() {
    let (mut u, [a, b, c, d]) = four();
    // The eye and padlock are at the row's right end (the left panel spans 56…296).
    let (eye, padlock) = (272.0, 250.0);
    let (rd, rb) = (u.row_at(d), u.row_at(b));
    let steps = u.app.session.undo_depth().0;
    let from = Pos2::new(eye, rd.y);
    u.frame(vec![Event::PointerMoved(from)]);
    u.drag_to(from, Pos2::new(eye, rb.y));
    u.button(Pos2::new(eye, rb.y), false);
    let doc = u.app.session.doc().unwrap();
    let hidden: Vec<bool> = [a, b, c, d].iter().map(|i| !doc.node(*i).unwrap().visible).collect();
    assert_eq!(hidden, [false, true, true, true], "dragging across the eyes hides every layer passed");
    assert_eq!(u.app.session.undo_depth().0, steps + 1, "silent: one undo step for the sweep (pinned)");
    for _ in 0..30 {
        u.frame(vec![]);
    }
    let from = Pos2::new(padlock, rd.y);
    u.frame(vec![Event::PointerMoved(from)]);
    u.drag_to(from, Pos2::new(padlock, rb.y));
    u.button(Pos2::new(padlock, rb.y), false);
    let doc = u.app.session.doc().unwrap();
    let locked: Vec<bool> = [a, b, c, d].iter().map(|i| doc.node(*i).unwrap().locked).collect();
    assert_eq!(locked, [false, true, true, true], "dragging across the padlocks locks every layer passed");
    // A plain click on an eye still toggles one layer.
    for _ in 0..30 {
        u.frame(vec![]);
    }
    let ra = u.row_at(a);
    click(&mut u, Pos2::new(eye, ra.y), Modifiers::NONE);
    assert!(!u.app.session.doc().unwrap().node(a).unwrap().visible, "a click on the eye hides that layer");
}

#[test]
fn renaming_several_layers_opens_the_rename_dialog() {
    let (mut u, [a, b, c, _d]) = four();
    // Cmd/Ctrl+R (F2, the context menu's Rename) with several layers selected.
    u.app.run("select.set", json!({"ids": [c, a, b]}));
    u.app.run("object.rename", json!({}));
    u.frame(vec![]);
    let dialog = u.app.ui.batch_rename.clone().expect("the Rename layers dialog opens for several layers");
    assert_eq!(dialog.ids, [a, b, c], "silent: the layers are numbered in stacking order, bottom first (pinned)");
    assert_eq!(u.app.ui.rename, None, "no single row is renamed in place");
    // Match empty: Rename to renames the whole name; Number up counts up.
    let mut d = dialog.clone();
    d.to = "Icon $n".into();
    u.app.ui.batch_rename = Some(d.clone());
    u.frame(vec![]);
    u.app.run("object.batchRename", crate::dialogs::batch_rename_params(&d));
    let doc = u.app.session.doc().unwrap();
    let names: Vec<&str> = [a, b, c].iter().map(|i| doc.node(*i).unwrap().name.as_str()).collect();
    assert_eq!(names, ["Icon 1", "Icon 2", "Icon 3"], "Rename to with Number up");
    // With a match, only the matched part changes.
    d.find = "Icon".into();
    d.to = "Logo".into();
    u.app.run("object.batchRename", crate::dialogs::batch_rename_params(&d));
    assert_eq!(u.app.session.doc().unwrap().node(a).unwrap().name, "Logo 1", "a match replaces only that part");
    // Escape closes the dialog; one layer still renames in its row.
    u.frame(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    assert!(u.app.ui.batch_rename.is_none(), "Escape closes the dialog");
    u.app.run("select.set", json!({"ids": [a]}));
    u.app.run("object.rename", json!({}));
    u.frame(vec![]);
    assert_eq!(u.app.ui.rename.map(|r| r.id), Some(a), "a single layer renames in its row");
    assert!(u.app.ui.batch_rename.is_none());
}

/// The middle of page `page`'s row in the pages list, found by clicking down the list from page
/// `back` (each click opens the page under it); `back` is open again afterwards.
fn page_row(u: &mut Ui, page: NodeId, back: NodeId) -> Pos2 {
    u.app.session.execute("page.select", &json!({"id": back})).unwrap();
    u.frame(vec![]);
    for y in (90..400).step_by(4) {
        let p = Pos2::new(150.0, y as f32);
        u.button(p, true);
        u.button(p, false);
        u.app.ui.rename_page = None;
        if u.app.session.page().unwrap() == page {
            u.app.session.execute("page.select", &json!({"id": back})).unwrap();
            for _ in 0..30 {
                u.frame(vec![]);
            }
            // Rows are 32 high: aim at the middle.
            return p + egui::vec2(0.0, 12.0);
        }
    }
    panic!("no row for page {page:?}");
}

#[test]
fn pages_drag_to_reorder_and_dividers_open_nothing() {
    let mut s = Session::new();
    let p1 = s.page().unwrap();
    let p2 = id(s.execute("page.add", &json!({})).unwrap());
    let p3 = id(s.execute("page.add", &json!({})).unwrap());
    s.execute("page.select", &json!({"id": p1})).unwrap();
    let mut u = Ui::new(s);
    let (r1, r3) = (page_row(&mut u, p1, p2), page_row(&mut u, p3, p2));
    // Keep the release point inside the actual scroll viewport; drops above it are ignored.
    u.drag_to(r3, r1 - egui::vec2(0.0, 4.0));
    u.button(r1 - egui::vec2(0.0, 4.0), false);
    assert_eq!(u.app.session.doc().unwrap().pages, [p3, p1, p2], "a page dragged up the list moves there");
    // A divider between two pages: a line that opens nothing.
    let div = id(u.app.session.execute("page.add", &json!({"name": "-"})).unwrap());
    u.app.session.execute("page.move", &json!({"id": div, "index": 1})).unwrap();
    u.app.session.execute("page.select", &json!({"id": p1})).unwrap();
    for _ in 0..30 {
        u.frame(vec![]);
    }
    let (top, below) = (page_row(&mut u, p3, p2), page_row(&mut u, p1, p2));
    u.app.session.execute("page.select", &json!({"id": p1})).unwrap();
    u.frame(vec![]);
    let mid = Pos2::new(top.x, (top.y + below.y) / 2.0);
    click(&mut u, mid, Modifiers::NONE);
    assert_eq!(u.app.session.page().unwrap(), p1, "clicking a divider opens nothing");
    assert!(u.app.ui.last_error.is_none(), "{:?}", u.app.ui.last_error);
}

#[test]
fn many_pages_scroll_without_clipping_the_layers_tree() {
    let mut s = Session::new();
    let first = s.page().unwrap();
    let layer = rect(&mut s, "visible-layer", 0.0, None);
    let last = (1..29).fold(first, |_, i| id(s.execute("page.add", &json!({"name": format!("Page {i:02}")})).unwrap()));
    s.execute("page.select", &json!({"id": first})).unwrap();
    s.execute("select.none", &json!({})).unwrap();
    let mut u = Ui::new(s);

    let layer_row = u.row_at(layer);
    u.button(layer_row, true);
    u.button(layer_row, false);
    assert_eq!(u.app.session.selection(), [layer], "the layers tree remains visible below the bounded pages list");
    let splitter_id = egui::Id::new("pages_list_splitter");
    let before = u.ctx.read_response(splitter_id).expect("the page/layers divider is visible").rect.center();
    u.drag_to(before, before + egui::vec2(0.0, 64.0));
    u.button(before + egui::vec2(0.0, 64.0), false);
    let after = u.ctx.read_response(splitter_id).expect("the divider remains visible after dragging").rect.center();
    assert!(after.y - before.y > 40.0, "dragging the divider increases the pages pane: {before:?} → {after:?}");

    u.app.ui.pages_open = false;
    u.frame(vec![]);
    assert!(u.row_at(layer).y > 0.0, "collapsing Pages leaves the layers tree available");
    u.app.ui.pages_open = true;
    u.frame(vec![]);

    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -240.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    };
    for _ in 0..8 {
        u.frame(vec![Event::PointerMoved(Pos2::new(150.0, 180.0)), wheel.clone()]);
    }
    let mut opened_last = false;
    for y in (110..450).step_by(4) {
        let p = Pos2::new(150.0, y as f32);
        u.button(p, true);
        u.button(p, false);
        if u.app.session.page().unwrap() == last {
            opened_last = true;
            break;
        }
    }
    assert!(opened_last, "the bottom page remains reachable by scrolling the pages list");
}

#[test]
fn dropping_page_drag_over_layers_does_not_reorder_hidden_pages() {
    let mut s = Session::new();
    let first = s.page().unwrap();
    let layer = rect(&mut s, "drop-target", 0.0, None);
    let second = id(s.execute("page.add", &json!({"name": "Page 2"})).unwrap());
    for i in 3..30 {
        s.execute("page.add", &json!({"name": format!("Page {i}")})).unwrap();
    }
    s.execute("page.select", &json!({"id": first})).unwrap();
    s.execute("select.none", &json!({})).unwrap();
    let mut u = Ui::new(s);

    // Locate the first page's row through the existing page click path, then restore it as current.
    let _ = page_row(&mut u, second, first);
    let first_row = page_row(&mut u, first, second);
    u.app.session.execute("page.select", &json!({"id": first})).unwrap();
    u.frame(vec![]);
    let layers_row = u.row_at(layer);
    let order = u.app.session.doc().unwrap().pages.clone();

    u.drag_to(first_row, layers_row);
    u.button(layers_row, false);

    assert_eq!(u.app.session.doc().unwrap().pages, order, "a drop outside the visible Pages viewport must not reorder hidden rows");
}

#[test]
fn page_layers_splitter_keeps_both_panes_and_clamps_in_small_windows() {
    let mut s = Session::new();
    let layer = rect(&mut s, "visible-layer", 0.0, None);
    let mut u = Ui::new(s);
    let splitter_id = egui::Id::new("pages_list_splitter");
    let initial = u.ctx.read_response(splitter_id).expect("the divider is visible").rect.center();

    u.drag_to(initial, initial - egui::vec2(0.0, 1_000.0));
    u.button(initial - egui::vec2(0.0, 1_000.0), false);
    let lower = u.ctx.read_response(splitter_id).expect("the divider remains visible at its lower limit").rect.center();
    assert!((initial.y - lower.y - 144.0).abs() < 1.0, "one-page list retains a 96pt minimum: {initial:?} → {lower:?}");
    assert!(u.row_at(layer).y > 0.0, "the Layers tree remains visible at the Pages minimum");

    u.drag_to(lower, lower + egui::vec2(0.0, 1_000.0));
    u.button(lower + egui::vec2(0.0, 1_000.0), false);
    let upper = u.ctx.read_response(splitter_id).expect("the divider remains visible at its upper limit").rect.center();
    assert!(upper.y > lower.y + 100.0, "the Pages pane expands while retaining the reserved Layers area");

    u.drag_to(upper, upper + egui::vec2(0.0, 1_000.0));
    u.button(upper + egui::vec2(0.0, 1_000.0), false);
    let clamped = u.ctx.read_response(splitter_id).expect("the divider stays visible after an overdrag").rect.center();
    assert!((clamped.y - upper.y).abs() < 1.0, "overdrag cannot consume the Layers reserve: {upper:?} → {clamped:?}");

    let small_window = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1_440.0, 300.0));
    let mut input = egui::RawInput { screen_rect: Some(small_window), ..Default::default() };
    input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().inner_rect = Some(small_window);
    let ctx = egui::Context::default();
    let mut small_session = Session::new();
    let small_layer = rect(&mut small_session, "small-window-layer", 0.0, None);
    let mut app = FigApp::new(small_session, Box::new(NoServices));
    for _ in 0..4 {
        let mut output = ctx.run_ui(input.clone(), |ui| app.ui(ui));
        output.textures_delta.clear();
    }
    let small = ctx.read_response(splitter_id).expect("a small window keeps the divider").rect;
    assert!(small.is_positive() && small.max.y <= 300.0, "small-window pane bounds stay nonnegative and within the window: {small:?}");
    assert!(small.center().y < 220.0, "the Pages viewport shrinks below egui's default 64pt floor: {small:?}");
    let mut visible_layer = false;
    for y in (small.max.y as i32..300).step_by(2) {
        let mut frame_input =
            egui::RawInput { screen_rect: Some(small_window), events: vec![Event::PointerMoved(Pos2::new(150.0, y as f32))], ..Default::default() };
        frame_input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().inner_rect = Some(small_window);
        let mut output = ctx.run_ui(frame_input, |ui| app.ui(ui));
        output.textures_delta.clear();
        if app.session.ds().unwrap().hover == Some(small_layer) {
            visible_layer = true;
            break;
        }
    }
    assert!(visible_layer, "the Layers tree retains real row space at the scaled 250pt minimum");
}
