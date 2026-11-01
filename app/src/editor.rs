//! Stan edytora: dokument, skład, zaznaczenie (tekst albo obiekty), cofanie i ponawianie oraz
//! wszystkie operacje na treści, formatowaniu, obiektach i węzłach kształtów.

use std::path::PathBuf;
use std::time::Instant;

use opendtp_core::edit::{self, At};
use opendtp_core::geom::{self, Rect};
use opendtp_core::layout::{Engine, Layout};
use opendtp_core::model::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Typing,
    Delete,
    Other,
}

struct Snap {
    doc: Document,
    story: StoryRef,
    anchor: At,
    head: At,
    obj: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Move {
    Left,
    Right,
    WordLeft,
    WordRight,
    Up,
    Down,
    LineStart,
    LineEnd,
    DocStart,
    DocEnd,
    PageUp,
    PageDown,
    ParaUp,
    ParaDown,
}

/// Ile kroków cofania pamiętamy.
const UNDO_LIMIT: usize = 300;

pub struct Editor {
    pub doc: Document,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub engine: Engine,
    pub layout: Layout,
    pub story: StoryRef,
    pub anchor: At,
    pub head: At,
    pub obj: Option<u64>,
    /// Edytowana komórka tabeli: (obiekt, wiersz, kolumna).
    pub cell: Option<(u64, usize, usize)>,
    pub multi: Vec<u64>,
    pub node_mode: bool,
    pub sel_node: Option<(usize, usize)>,
    pub typing: Option<CharFmt>,
    pub goal_x: Option<f32>,
    pub version: u64,
    pub painter: Option<(CharFmt, Option<(String, ParaFmt)>)>,
    pub find: Vec<(usize, usize, usize)>,
    pub find_i: usize,
    undo: std::collections::VecDeque<Snap>,
    redo: Vec<Snap>,
    last_kind: Kind,
    last_time: Instant,
    pub recovered: bool,
}

impl Editor {
    pub fn new(doc: Document) -> Editor {
        let mut engine = Engine::new();
        let layout = engine.layout(&doc);
        Editor {
            doc,
            path: None,
            dirty: false,
            engine,
            layout,
            story: StoryRef::Body,
            anchor: At::default(),
            head: At::default(),
            obj: None,
            cell: None,
            multi: Vec::new(),
            node_mode: false,
            sel_node: None,
            typing: None,
            goal_x: None,
            version: 1,
            painter: None,
            find: Vec::new(),
            find_i: 0,
            undo: std::collections::VecDeque::new(),
            redo: Vec::new(),
            last_kind: Kind::Other,
            last_time: Instant::now(),
            recovered: false,
        }
    }

    // ------------------------------------------------------------ podstawy

    pub fn relayout(&mut self) {
        // Obiekty zakotwiczone w tekście zależą od składu – kilka przebiegów, aż położenia się ustalą.
        for _ in 0..6 {
            self.layout = self.engine.layout(&self.doc);
            if !opendtp_core::layout::apply_anchors(&mut self.doc, &self.layout) {
                break;
            }
        }
        self.version += 1;
    }

    /// Czy obiekt jest zakotwiczony w tekście.
    pub fn anchored(&self, id: u64) -> bool {
        self.doc.body.paras.iter().any(|p| p.runs.iter().any(|r| r.field == Some(Field::Anchor(id))))
    }

    /// Kotwica obiektu na początku akapitu (treść główna) – obiekt odtąd płynie z tekstem.
    pub fn anchor_at(&mut self, id: u64, para: usize) {
        let para = para.min(self.doc.body.paras.len().saturating_sub(1));
        edit::insert_field(&mut self.doc.body, At::new(para, 0), Field::Anchor(id), &CharFmt::default());
        if self.story == StoryRef::Body && self.head.para == para {
            let n = FIELD_CHAR.len_utf8();
            self.head.off += n;
            self.anchor.off += n;
        }
    }

    pub fn unanchor(&mut self, id: u64) {
        for p in self.doc.body.paras.iter_mut() {
            p.runs.retain(|r| r.field != Some(Field::Anchor(id)));
            p.normalize();
        }
        self.fix_selection();
    }

    /// Po przeciągnięciu obiektu zakotwiczonego: nowy odstęp od punktu zakotwiczenia.
    pub fn update_anchor_offset(&mut self, id: u64) {
        let Some((page, refy)) = opendtp_core::layout::anchor_ref(&self.doc, &self.layout, id) else { return };
        if let Some(o) = self.doc.object_mut(id) {
            o.anchor_dy = if o.page == page { o.y - refy } else { 4.0 };
        }
    }

    /// Zapamiętuje stan przed zmianą (pisanie i usuwanie łączą się w jedną operację cofania).
    pub fn begin(&mut self, kind: Kind) {
        let now = Instant::now();
        let merge = kind != Kind::Other && kind == self.last_kind && now.duration_since(self.last_time).as_millis() < 1500;
        if !merge {
            let snap = Snap { doc: self.doc.clone(), story: self.story, anchor: self.anchor, head: self.head, obj: self.obj };
            self.push_undo(snap);
        }
        self.redo.clear();
        self.last_kind = kind;
        self.last_time = now;
    }

    /// Po każdej zmianie treści.
    pub fn changed(&mut self) {
        self.dirty = true;
        for o in self.doc.objects.iter_mut() {
            if let ObjKind::Table { table } = &o.kind {
                o.h = opendtp_core::paint::table_height(table, o.w);
            }
        }
        self.fix_selection();
        self.relayout();
        if !self.find.is_empty() {
            self.find.clear();
        }
    }

    /// Najstarszy krok wypada z przodu kolejki w stałym czasie (wcześniej `remove(0)` przesuwało całą listę).
    fn push_undo(&mut self, snap: Snap) {
        self.undo.push_back(snap);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.pop_front();
        }
    }

    pub fn break_group(&mut self) {
        self.last_kind = Kind::Other;
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop_back() else { return false };
        self.redo.push(Snap { doc: std::mem::replace(&mut self.doc, s.doc), story: self.story, anchor: self.anchor, head: self.head, obj: self.obj });
        self.story = s.story;
        self.anchor = s.anchor;
        self.head = s.head;
        self.obj = s.obj;
        self.last_kind = Kind::Other;
        self.changed();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        let snap = Snap { doc: std::mem::replace(&mut self.doc, s.doc), story: self.story, anchor: self.anchor, head: self.head, obj: self.obj };
        self.push_undo(snap);
        self.story = s.story;
        self.anchor = s.anchor;
        self.head = s.head;
        self.obj = s.obj;
        self.last_kind = Kind::Other;
        self.changed();
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Bieżący tekst (gdy ramka zniknęła – wracamy do treści głównej).
    pub fn story_ref(&self) -> &Story {
        self.doc.story(self.story).unwrap_or(&self.doc.body)
    }

    fn story_mut(&mut self) -> &mut Story {
        if self.doc.story(self.story).is_none() {
            self.story = StoryRef::Body;
        }
        let r = self.story;
        self.doc.story_mut(r).unwrap()
    }

    pub fn fix_selection(&mut self) {
        if self.doc.story(self.story).is_none() {
            self.story = StoryRef::Body;
        }
        if let StoryRef::Obj(id) = self.story {
            let head = self.doc.chain_head(id);
            self.story = StoryRef::Obj(head);
        }
        let (a, h) = {
            let s = self.story_ref();
            (edit::clamp(s, self.anchor), edit::clamp(s, self.head))
        };
        self.anchor = a;
        self.head = h;
        if let Some(o) = self.obj {
            if self.doc.object(o).is_none() {
                self.obj = None;
                self.node_mode = false;
            }
        }
        self.multi.retain(|m| self.doc.object(*m).is_some());
    }

    pub fn range(&self) -> (At, At) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    pub fn has_sel(&self) -> bool {
        self.anchor != self.head
    }

    pub fn set_caret(&mut self, story: StoryRef, at: At, extend: bool) {
        if story != self.story {
            self.story = story;
            self.anchor = at;
        } else if !extend {
            self.anchor = at;
        }
        self.head = at;
        self.typing = None;
        self.fix_selection();
        self.break_group();
    }

    pub fn para_text(&self, p: usize) -> String {
        self.story_ref().paras.get(p).map(|p| p.text()).unwrap_or_default()
    }

    /// Formatowanie znaków w miejscu kursora (z oczekującym formatowaniem pisania).
    pub fn caret_fmt(&self) -> CharFmt {
        let (a, _) = self.range();
        let s = self.story_ref();
        let p = &s.paras[a.para.min(s.paras.len() - 1)];
        let off = if self.has_sel() { a.off + 1 } else { a.off };
        let mut f = p.fmt_at(off.min(p.len()));
        if let Some(t) = &self.typing {
            f.merge(t);
        }
        f
    }

    /// Pełne formatowanie w miejscu kursora: znaki, akapit, styl.
    pub fn caret_props(&self) -> (CharProps, ParaProps, String) {
        let (a, _) = self.range();
        let s = self.story_ref();
        let p = &s.paras[a.para.min(s.paras.len() - 1)];
        let (pp, mut cp) = self.doc.resolve_para(p);
        cp.apply(&self.caret_fmt());
        (cp, pp, p.style.clone())
    }

    // ------------------------------------------------------------ pisanie

    /// Oznaczenie śledzonej zmiany bieżącego użytkownika.
    pub fn rev_mark(kind: RevKind) -> Rev {
        let author = std::env::var("USERNAME").unwrap_or_default();
        Rev { kind, author, date: opendtp_core::util::iso_now() }
    }

    /// Usuwa tekst `a..b`. Przy śledzeniu zmian tekst zostaje, oznaczony jako usunięty (własne,
    /// niezaakceptowane wstawienia znikają od razu). Zwraca miejsce, w którym kończy się zakres.
    fn remove(&mut self, a: At, b: At) -> At {
        if !self.doc.track {
            edit::delete(self.story_mut(), a, b);
            return a;
        }
        let mark = Editor::rev_mark(RevKind::Delete);
        const DROP: &str = "\u{1}";
        edit::format_chars(self.story_mut(), a, b, &|f| match f.rev.as_ref().map(|r| r.kind) {
            Some(RevKind::Insert) => f.rev = Some(Rev { kind: RevKind::Insert, author: DROP.into(), date: String::new() }),
            Some(RevKind::Delete) => {}
            None => f.rev = Some(mark.clone()),
        });
        let is_drop = |r: &Run| r.fmt.rev.as_ref().is_some_and(|x| x.author == DROP);
        let s = self.story_mut();
        let mut shift = 0;
        let last = b.para.min(s.paras.len().saturating_sub(1));
        for pi in a.para..=last {
            let p = &mut s.paras[pi];
            if pi == b.para {
                let mut off = 0;
                for r in &p.runs {
                    let l = r.text.len();
                    if off + l <= b.off && is_drop(r) {
                        shift += l;
                    }
                    off += l;
                }
            }
            p.runs.retain(|r| !is_drop(r));
            if p.runs.is_empty() {
                p.runs.push(Run::default());
            }
        }
        At::new(b.para, b.off.saturating_sub(shift))
    }

    pub fn insert(&mut self, text: &str) {
        self.begin(if text.chars().all(|c| !c.is_whitespace()) || text == " " { Kind::Typing } else { Kind::Other });
        if self.has_sel() {
            let (a, b) = self.range();
            self.head = self.remove(a, b);
        }
        let track = self.doc.track;
        let fmt = {
            let s = self.story_ref();
            let mut f = s.paras[self.head.para.min(s.paras.len() - 1)].fmt_at(self.head.off);
            if let Some(t) = &self.typing {
                f.merge(t);
            }
            f.link = None.or(f.link);
            // Nowy tekst nie dziedziczy oznaczenia zmiany po sąsiedzie.
            f.rev = if track { Some(Editor::rev_mark(RevKind::Insert)) } else { None };
            f
        };
        let at = self.head;
        let end = edit::insert_text(self.story_mut(), at, text, &fmt);
        self.anchor = end;
        self.head = end;
        self.goal_x = None;
        self.changed();
    }

    /// Zastępuje `len` bajtów przed kursorem tekstem `with` (autokorekta).
    pub fn replace_before(&mut self, len: usize, with: &str) {
        let h = self.head;
        if h.off < len {
            return;
        }
        let a = At::new(h.para, h.off - len);
        let mut fmt = self.story_ref().paras[h.para].fmt_at(a.off + 1);
        fmt.rev = if self.doc.track { Some(Editor::rev_mark(RevKind::Insert)) } else { None };
        let at = self.remove(a, h);
        let end = edit::insert_text(self.story_mut(), at, with, &fmt);
        self.anchor = end;
        self.head = end;
        self.changed();
    }

    pub fn backspace(&mut self, word: bool) {
        if self.has_sel() {
            self.delete_sel();
            return;
        }
        let h = self.head;
        let text = self.para_text(h.para);
        // Pusty akapit listy: najpierw zdejmujemy listę.
        if h.off == 0 {
            let (pp, _) = {
                let p = &self.story_ref().paras[h.para];
                self.doc.resolve_para(p)
            };
            if pp.list.is_some() {
                self.begin(Kind::Other);
                let p = h.para;
                edit::format_paras(self.story_mut(), p, p, &|pa| pa.fmt.list = Some(None));
                self.changed();
                return;
            }
            if h.para == 0 {
                return;
            }
            self.begin(Kind::Delete);
            let prev_len = self.story_ref().paras[h.para - 1].len();
            let a = At::new(h.para - 1, prev_len);
            // Przy śledzeniu zmian akapity się nie łączą – kursor przechodzi tylko na koniec poprzedniego.
            if !self.doc.track {
                edit::delete(self.story_mut(), a, h);
            }
            self.anchor = a;
            self.head = a;
            self.changed();
            return;
        }
        self.begin(Kind::Delete);
        let from = if word { edit::prev_word(&text, h.off) } else { edit::prev_char(&text, h.off) };
        let a = At::new(h.para, from);
        self.remove(a, h);
        self.anchor = a;
        self.head = a;
        self.changed();
    }

    pub fn delete_forward(&mut self, word: bool) {
        if self.has_sel() {
            self.delete_sel();
            return;
        }
        let h = self.head;
        let text = self.para_text(h.para);
        self.begin(Kind::Delete);
        let b = if h.off >= text.len() {
            if h.para + 1 >= self.story_ref().paras.len() {
                return;
            }
            At::new(h.para + 1, 0)
        } else if word {
            At::new(h.para, edit::next_word(&text, h.off))
        } else {
            At::new(h.para, edit::next_char(&text, h.off))
        };
        if self.doc.track && b.para != h.para {
            // Śledzenie zmian: koniec akapitu zostaje, kursor przechodzi dalej.
            self.anchor = b;
            self.head = b;
        } else {
            let end = self.remove(h, b);
            self.anchor = end;
            self.head = end;
        }
        self.changed();
    }

    pub fn delete_sel(&mut self) {
        let (a, b) = self.range();
        self.begin(Kind::Other);
        self.remove(a, b);
        self.anchor = a;
        self.head = a;
        self.changed();
    }

    /// Enter: nowy akapit (styl następny, gdy kursor na końcu nagłówka); w pustym akapicie listy – koniec listy.
    pub fn enter(&mut self) {
        if self.has_sel() {
            self.delete_sel();
        }
        let h = self.head;
        let (pp, _) = {
            let p = &self.story_ref().paras[h.para];
            self.doc.resolve_para(p)
        };
        let empty = self.story_ref().paras[h.para].is_empty();
        if empty && pp.list.is_some() {
            self.begin(Kind::Other);
            let p = h.para;
            edit::format_paras(self.story_mut(), p, p, &|pa| {
                pa.fmt.list = Some(None);
                if pa.style == "list" {
                    pa.style = "normal".into();
                }
            });
            self.changed();
            return;
        }
        self.begin(Kind::Other);
        let at_end = h.off >= self.story_ref().paras[h.para].len();
        let style = self.story_ref().paras[h.para].style.clone();
        let next = self.doc.style(&style).and_then(|s| s.next.clone());
        let end = edit::split(self.story_mut(), h);
        if at_end {
            if let Some(n) = next {
                if n != style {
                    let p = end.para;
                    edit::format_paras(self.story_mut(), p, p, &|pa| {
                        pa.style = n.clone();
                        pa.fmt.list = if n == "list" { pa.fmt.list } else { None };
                        pa.fmt.drop_cap = None;
                        for r in pa.runs.iter_mut() {
                            r.fmt = CharFmt::default();
                        }
                    });
                }
            }
        }
        let p = end.para;
        edit::format_paras(self.story_mut(), p, p, &|pa| pa.fmt.drop_cap = None);
        self.anchor = end;
        self.head = end;
        self.changed();
    }

    pub fn page_break(&mut self, column: bool) {
        self.enter();
        let p = self.head.para;
        self.begin(Kind::Other);
        edit::format_paras(self.story_mut(), p, p, &|pa| pa.fmt.break_before = Some(if column { Break::Column } else { Break::Page }));
        self.changed();
    }

    // ------------------------------------------------------------ ruch kursora

    pub fn move_caret(&mut self, m: Move, extend: bool) {
        if self.has_sel() && !extend && matches!(m, Move::Left | Move::Right) {
            let (a, b) = self.range();
            let t = if m == Move::Left { a } else { b };
            self.anchor = t;
            self.head = t;
            self.goal_x = None;
            return;
        }
        let h = self.head;
        let n = self.story_ref().paras.len();
        let text = self.para_text(h.para);
        let next = match m {
            Move::Left => {
                if h.off > 0 {
                    At::new(h.para, edit::prev_char(&text, h.off))
                } else if h.para > 0 {
                    At::new(h.para - 1, self.story_ref().paras[h.para - 1].len())
                } else {
                    h
                }
            }
            Move::Right => {
                if h.off < text.len() {
                    At::new(h.para, edit::next_char(&text, h.off))
                } else if h.para + 1 < n {
                    At::new(h.para + 1, 0)
                } else {
                    h
                }
            }
            Move::WordLeft => {
                if h.off == 0 && h.para > 0 {
                    At::new(h.para - 1, self.story_ref().paras[h.para - 1].len())
                } else {
                    At::new(h.para, edit::prev_word(&text, h.off))
                }
            }
            Move::WordRight => {
                if h.off >= text.len() && h.para + 1 < n {
                    At::new(h.para + 1, 0)
                } else {
                    At::new(h.para, edit::next_word(&text, h.off))
                }
            }
            Move::ParaUp => At::new(if h.off == 0 { h.para.saturating_sub(1) } else { h.para }, 0),
            Move::ParaDown => At::new((h.para + 1).min(n - 1), if h.para + 1 >= n { text.len() } else { 0 }),
            Move::DocStart => At::new(0, 0),
            Move::DocEnd => At::new(n - 1, self.story_ref().paras[n - 1].len()),
            Move::LineStart | Move::LineEnd => match self.layout.line_for(self.story, h.para, h.off) {
                Some(li) => {
                    let l = &self.layout.lines[li];
                    if m == Move::LineStart {
                        At::new(h.para, l.start)
                    } else if l.last {
                        At::new(h.para, text.len())
                    } else {
                        let off = self.layout.offset_in_line(li, f32::MAX);
                        At::new(h.para, off)
                    }
                }
                None => h,
            },
            Move::Up | Move::Down | Move::PageUp | Move::PageDown => {
                let Some(c) = self.layout.caret(self.story, h.para, h.off) else { return };
                let x = *self.goal_x.get_or_insert(c.x);
                let lines = self.layout.story_lines(self.story);
                let Some(pos) = lines.iter().position(|&i| i == c.line) else { return };
                let step: isize = match m {
                    Move::Up => -1,
                    Move::Down => 1,
                    Move::PageUp => -30,
                    _ => 30,
                };
                let target = (pos as isize + step).clamp(0, lines.len() as isize - 1) as usize;
                if target == pos {
                    if step < 0 {
                        At::new(0, 0)
                    } else {
                        At::new(n - 1, self.story_ref().paras[n - 1].len())
                    }
                } else {
                    let li = lines[target];
                    let l = &self.layout.lines[li];
                    let off = self.layout.offset_in_line(li, x);
                    let keep = self.goal_x;
                    let at = At::new(l.para, off);
                    if !extend {
                        self.anchor = at;
                    }
                    self.head = at;
                    self.goal_x = keep;
                    self.typing = None;
                    return;
                }
            }
        };
        if !matches!(m, Move::Up | Move::Down | Move::PageUp | Move::PageDown) {
            self.goal_x = None;
        }
        if !extend {
            self.anchor = next;
        }
        self.head = next;
        self.typing = None;
        self.break_group();
    }

    pub fn select_all(&mut self) {
        let end = {
            let s = self.story_ref();
            let n = s.paras.len();
            At::new(n - 1, s.paras[n - 1].len())
        };
        self.anchor = At::new(0, 0);
        self.head = end;
        self.obj = None;
    }

    pub fn select_word(&mut self, at: At) {
        let t = self.para_text(at.para);
        let (a, b) = edit::word_at(&t, at.off);
        self.anchor = At::new(at.para, a);
        self.head = At::new(at.para, b);
    }

    pub fn select_para(&mut self, p: usize) {
        let len = self.story_ref().paras.get(p).map(|p| p.len()).unwrap_or(0);
        self.anchor = At::new(p, 0);
        self.head = At::new(p, len);
    }

    pub fn select_sentence(&mut self, at: At) {
        let t = self.para_text(at.para);
        let (a, b) = edit::sentence_at(&t, at.off);
        self.anchor = At::new(at.para, a);
        self.head = At::new(at.para, b);
    }

    // ------------------------------------------------------------ formatowanie

    /// Formatowanie znaków zaznaczenia; bez zaznaczenia – dla wpisywanego tekstu albo słowa pod kursorem.
    pub fn format(&mut self, f: &dyn Fn(&mut CharFmt)) {
        if let (Some(o), false) = (self.obj, self.in_obj_text()) {
            // Zaznaczony obiekt z tekstem: formatujemy cały jego tekst.
            if self.doc.object(o).and_then(|ob| ob.story()).is_some() {
                self.begin(Kind::Other);
                let id = self.doc.chain_head(o);
                if let Some(s) = self.doc.story_mut(StoryRef::Obj(id)) {
                    let n = s.paras.len();
                    let end = At::new(n - 1, s.paras[n - 1].len());
                    edit::format_chars(s, At::new(0, 0), end, f);
                }
                self.changed();
                return;
            }
        }
        if !self.has_sel() {
            let t = self.para_text(self.head.para);
            let (a, b) = edit::word_at(&t, self.head.off);
            let inside = a < self.head.off && self.head.off < b && t[a..b].chars().all(|c| c.is_alphanumeric());
            if !inside {
                let mut tf = self.typing.clone().unwrap_or_else(|| self.caret_fmt());
                f(&mut tf);
                self.typing = Some(tf);
                return;
            }
            self.begin(Kind::Other);
            let p = self.head.para;
            edit::format_chars(self.story_mut(), At::new(p, a), At::new(p, b), f);
            self.changed();
            return;
        }
        self.begin(Kind::Other);
        let (a, b) = self.range();
        edit::format_chars(self.story_mut(), a, b, f);
        self.changed();
    }

    fn in_obj_text(&self) -> bool {
        matches!(self.story, StoryRef::Obj(_)) && self.obj.is_some_and(|o| self.doc.chain_head(o) == match self.story {
            StoryRef::Obj(h) => h,
            _ => 0,
        })
    }

    /// Akapity zaznaczenia (albo cały tekst zaznaczonego obiektu).
    pub fn format_paras(&mut self, f: &dyn Fn(&mut Paragraph)) {
        if let (Some(o), false) = (self.obj, self.in_obj_text()) {
            if self.doc.object(o).and_then(|ob| ob.story()).is_some() {
                self.begin(Kind::Other);
                let id = self.doc.chain_head(o);
                if let Some(s) = self.doc.story_mut(StoryRef::Obj(id)) {
                    let n = s.paras.len();
                    edit::format_paras(s, 0, n - 1, f);
                }
                self.changed();
                return;
            }
        }
        self.begin(Kind::Other);
        let (a, b) = self.range();
        edit::format_paras(self.story_mut(), a.para, b.para, f);
        self.changed();
    }

    pub fn toggle(&mut self, get: fn(&CharProps) -> bool, set: fn(&mut CharFmt, bool)) {
        let (cp, _, _) = self.caret_props();
        let v = !get(&cp);
        self.format(&move |f| set(f, v));
    }

    pub fn clear_format(&mut self) {
        if !self.has_sel() {
            self.typing = Some(CharFmt::default());
            return;
        }
        self.format(&|f| *f = CharFmt { link: f.link.clone(), ..CharFmt::default() });
    }

    pub fn set_style(&mut self, id: &str) {
        let id = id.to_string();
        self.format_paras(&move |p| {
            p.style = id.clone();
            p.fmt = ParaFmt { list: if id == "list" { p.fmt.list } else { None }, ..ParaFmt::default() };
            // Styl nagłówka zdejmuje bezpośrednie formatowanie znaków (jak w Wordzie przy całym akapicie).
            for r in p.runs.iter_mut() {
                r.fmt.font = None;
                r.fmt.size = None;
                r.fmt.color = None;
            }
        });
    }

    pub fn set_list(&mut self, kind: Option<ListKind>) {
        self.format_paras(&move |p| {
            p.fmt.list = Some(kind.map(|k| ListFmt { kind: k, level: p.fmt.list.flatten().map(|l| l.level).unwrap_or(0), start: None }));
            if kind.is_some() && p.style == "normal" {
                p.style = "list".into();
            } else if kind.is_none() && p.style == "list" {
                p.style = "normal".into();
            }
        });
    }

    /// Wstawia stronę tytułową na początek treści głównej; dalszy tekst zaczyna się od nowej strony.
    pub fn insert_cover(&mut self, paras: Vec<Paragraph>) {
        let n = paras.len();
        if n == 0 {
            return;
        }
        self.begin(Kind::Other);
        let body = &mut self.doc.body;
        if let Some(first) = body.paras.first_mut() {
            first.fmt.break_before = Some(Break::Page);
        }
        body.paras.splice(0..0, paras);
        if self.story == StoryRef::Body {
            self.anchor = At::new(0, 0);
            self.head = At::new(0, 0);
        }
        self.changed();
    }

    /// Sortuje zaznaczone akapity alfabetycznie (polska kolejność liter: a < ą < b).
    /// Usuwa zaznaczone akapity (w całości) i zwraca ich tekst – do zamiany tekstu w tabelę.
    pub fn take_selected_paras(&mut self) -> Vec<String> {
        let (a, b) = self.range();
        self.begin(Kind::Other);
        let s = self.story_mut();
        let end = b.para.min(s.paras.len().saturating_sub(1));
        if a.para > end {
            return Vec::new();
        }
        let lines: Vec<String> = s.paras[a.para..=end].iter().map(|p| p.text()).collect();
        s.paras.drain(a.para..=end);
        // Pusty akapit na miejscu tekstu – w nim stanie kotwica tabeli.
        s.paras.insert(a.para, Paragraph::default());
        self.anchor = At::new(a.para, 0);
        self.head = self.anchor;
        self.changed();
        lines
    }

    pub fn sort_paras(&mut self, desc: bool) {
        let (a, b) = self.range();
        if a.para >= b.para {
            return;
        }
        fn key(s: &str) -> Vec<(char, u8)> {
            s.to_lowercase().chars().map(|c| match c {
                'ą' => ('a', 1),
                'ć' => ('c', 1),
                'ę' => ('e', 1),
                'ł' => ('l', 1),
                'ń' => ('n', 1),
                'ó' => ('o', 1),
                'ś' => ('s', 1),
                'ź' => ('z', 1),
                'ż' => ('z', 2),
                c => (c, 0),
            }).collect()
        }
        self.begin(Kind::Other);
        let s = self.story_mut();
        let mut v: Vec<Paragraph> = s.paras[a.para..=b.para].to_vec();
        v.sort_by_cached_key(|p| key(&p.text()));
        if desc {
            v.reverse();
        }
        let end = v.last().map(|p| p.len()).unwrap_or(0);
        s.paras.splice(a.para..=b.para, v);
        self.anchor = At::new(a.para, 0);
        self.head = At::new(b.para, end);
        self.changed();
    }

    pub fn restart_numbering(&mut self) {
        let doc = &self.doc;
        let (a, _) = self.range();
        let p = &self.story_ref().paras[a.para];
        let Some(l) = doc.resolve_para(p).0.list else { return };
        self.format_paras(&move |pa| pa.fmt.list = Some(Some(ListFmt { start: Some(1), ..l })));
    }

    pub fn indent(&mut self, delta: i32) {
        let (a, _) = self.range();
        let p = &self.story_ref().paras[a.para];
        let (pp, _) = self.doc.resolve_para(p);
        if pp.list.is_some() {
            self.format_paras(&move |pa| {
                let base = pa.fmt.list.flatten();
                if let Some(mut l) = base {
                    l.level = (l.level as i32 + delta).clamp(0, 8) as u8;
                    pa.fmt.list = Some(Some(l));
                }
            });
            // Lista ze stylu (bez bezpośredniego ustawienia) – ustawiamy poziom jawnie.
            let l = pp.list.unwrap();
            self.format_paras(&move |pa| {
                if pa.fmt.list.flatten().is_none() {
                    pa.fmt.list = Some(Some(ListFmt { level: (l.level as i32 + delta).clamp(0, 8) as u8, ..l }));
                }
            });
            return;
        }
        let step = 18.0 * delta as f32;
        let cur = pp.indent_left;
        self.format_paras(&move |pa| pa.fmt.indent_left = Some((cur + step).max(0.0)));
    }

    // ------------------------------------------------------------ schowek

    pub fn fragment(&self) -> Vec<Paragraph> {
        let (a, b) = self.range();
        // Kotwice obiektów nie kopiują się razem z tekstem.
        edit::fragment(self.story_ref(), a, b)
            .into_iter()
            .map(|mut p| {
                p.runs.retain(|r| !matches!(r.field, Some(Field::Anchor(_))));
                p.normalize();
                p
            })
            .collect()
    }

    pub fn paste(&mut self, frag: &[Paragraph]) {
        self.begin(Kind::Other);
        if self.has_sel() {
            let (a, b) = self.range();
            edit::delete(self.story_mut(), a, b);
            self.head = a;
        }
        let at = self.head;
        let end = edit::insert_fragment(self.story_mut(), at, frag);
        self.anchor = end;
        self.head = end;
        self.changed();
    }

    /// Wklejanie z innego programu: w pustym akapicie pierwszy wklejony akapit zachowuje też
    /// swój styl i formatowanie akapitu (nagłówek zostaje nagłówkiem), jak w Wordzie.
    pub fn paste_rich(&mut self, frag: &[Paragraph]) {
        let p = self.head.para;
        let empty = !self.has_sel() && self.story_ref().paras.get(p).is_some_and(|x| x.is_empty());
        self.paste(frag);
        if let (true, Some(first)) = (empty, frag.first()) {
            if let Some(dst) = self.story_mut().paras.get_mut(p) {
                dst.style = first.style.clone();
                dst.fmt = first.fmt.clone();
            }
            self.changed();
        }
    }

    pub fn paste_text(&mut self, text: &str) {
        let t = text.replace("\r\n", "\n").replace('\r', "\n");
        let fmt = self.caret_fmt();
        let frag: Vec<Paragraph> = t.split('\n').map(|l| Paragraph { style: String::new(), fmt: ParaFmt::default(), runs: vec![Run { text: l.to_string(), fmt: fmt.clone(), field: None }] }).collect();
        // Akapity wklejonego tekstu przyjmują styl bieżącego akapitu.
        let style = self.story_ref().paras[self.head.para].style.clone();
        let frag: Vec<Paragraph> = frag.into_iter().map(|mut p| {
            p.style = style.clone();
            p
        }).collect();
        self.paste(&frag);
    }

    // ------------------------------------------------------------ obiekty

    pub fn object(&self) -> Option<&Object> {
        self.obj.and_then(|o| self.doc.object(o))
    }

    pub fn edit_obj(&mut self, f: &dyn Fn(&mut Object)) {
        let Some(id) = self.obj else { return };
        self.begin(Kind::Other);
        let ids: Vec<u64> = std::iter::once(id).chain(self.multi.iter().copied().filter(|m| *m != id)).collect();
        for i in ids {
            if let Some(o) = self.doc.object_mut(i) {
                f(o);
            }
        }
        self.changed();
    }

    /// Zmiana w trakcie przeciągania – bez nowego kroku cofania (krok zapisano przy naciśnięciu).
    pub fn drag_obj(&mut self, id: u64, f: &dyn Fn(&mut Object)) {
        if let Some(o) = self.doc.object_mut(id) {
            f(o);
        }
        self.dirty = true;
        self.relayout();
    }

    pub fn add_object(&mut self, mut o: Object) -> u64 {
        self.begin(Kind::Other);
        o.id = self.doc.new_id();
        let id = o.id;
        if o.page >= self.layout.pages.len() {
            self.doc.min_pages = self.doc.min_pages.max(o.page + 1);
        }
        self.doc.objects.push(o);
        self.obj = Some(id);
        self.multi.clear();
        self.changed();
        id
    }

    pub fn delete_object(&mut self) {
        let Some(id) = self.obj else { return };
        self.begin(Kind::Other);
        let ids: Vec<u64> = std::iter::once(id).chain(self.multi.iter().copied()).collect();
        for id in &ids {
            // Ramki łańcucha: tekst przechodzi do następnej, łącza omijają usuwaną.
            let next = match self.doc.object(*id).map(|o| &o.kind) {
                Some(ObjKind::Text { next, .. }) => *next,
                _ => None,
            };
            let story = self.doc.object(*id).and_then(|o| o.story().cloned());
            let is_head = self.doc.chain_head(*id) == *id;
            for o in self.doc.objects.iter_mut() {
                if let ObjKind::Text { next: n, .. } = &mut o.kind {
                    if *n == Some(*id) {
                        *n = next;
                    }
                }
            }
            if let (true, Some(n), Some(s)) = (is_head, next, story) {
                if let Some(ObjKind::Text { story, .. }) = self.doc.object_mut(n).map(|o| &mut o.kind) {
                    *story = s;
                }
            }
            self.doc.objects.retain(|o| o.id != *id && o.cont_of != Some(*id));
            for p in self.doc.body.paras.iter_mut() {
                p.runs.retain(|r| r.field != Some(Field::Anchor(*id)));
                p.normalize();
            }
        }
        self.obj = None;
        self.multi.clear();
        self.node_mode = false;
        if let StoryRef::Obj(_) = self.story {
            self.story = StoryRef::Body;
        }
        self.changed();
    }

    /// Zmiana tabeli zaznaczonego obiektu.
    pub fn edit_table(&mut self, f: &dyn Fn(&mut Table)) {
        let Some(id) = self.obj else { return };
        self.begin(Kind::Other);
        if let Some(ObjKind::Table { table }) = self.doc.object_mut(id).map(|o| &mut o.kind) {
            f(table);
            let cols = table.cols();
            for r in table.rows.iter_mut() {
                r.resize(cols, String::new());
            }
            if !table.widths.is_empty() && table.widths.len() != cols {
                table.widths.clear();
            }
        }
        self.changed();
    }

    pub fn duplicate(&mut self) {
        let Some(o) = self.object().cloned() else { return };
        let mut c = o;
        c.x += 14.0;
        c.y += 14.0;
        if let ObjKind::Text { next, .. } = &mut c.kind {
            *next = None;
        }
        self.add_object(c);
    }

    pub fn z_order(&mut self, how: &str) {
        let Some(id) = self.obj else { return };
        let Some(i) = self.doc.objects.iter().position(|o| o.id == id) else { return };
        self.begin(Kind::Other);
        let o = self.doc.objects.remove(i);
        let n = self.doc.objects.len();
        let j = match how {
            "front" => n,
            "back" => 0,
            "forward" => (i + 1).min(n),
            _ => i.saturating_sub(1),
        };
        self.doc.objects.insert(j, o);
        self.changed();
    }

    pub fn align_obj(&mut self, how: &str) {
        let ps = self.doc.page.clone();
        let how = how.to_string();
        self.edit_obj(&move |o| match how.as_str() {
            "left" => o.x = ps.margin_left,
            "center" => o.x = (ps.width - o.w) / 2.0,
            "right" => o.x = ps.width - ps.margin_right - o.w,
            "top" => o.y = ps.margin_top,
            "middle" => o.y = (ps.height - o.h) / 2.0,
            _ => o.y = ps.height - ps.margin_bottom - o.h,
        });
    }

    pub fn flip(&mut self, vertical: bool) {
        self.edit_obj(&move |o| {
            let f = |v: f32| 1.0 - v;
            for c in o.shape.contours.iter_mut() {
                for n in c.nodes.iter_mut() {
                    if vertical {
                        n.y = f(n.y);
                        n.cin = n.cin.map(|(x, y)| (x, f(y)));
                        n.cout = n.cout.map(|(x, y)| (x, f(y)));
                    } else {
                        n.x = f(n.x);
                        n.cin = n.cin.map(|(x, y)| (f(x), y));
                        n.cout = n.cout.map(|(x, y)| (f(x), y));
                    }
                }
            }
        });
    }

    /// Operacja logiczna na dwóch zaznaczonych kształtach – wynik zastępuje pierwszy.
    pub fn boolean(&mut self, op: geom::BoolOp) -> bool {
        let Some(a) = self.obj else { return false };
        let Some(&b) = self.multi.iter().find(|m| **m != a) else { return false };
        let (Some(oa), Some(ob)) = (self.doc.object(a).cloned(), self.doc.object(b).cloned()) else { return false };
        if oa.page != ob.page {
            return false;
        }
        let Some((shape, r)) = geom::boolean(&oa.shape, Rect::new(oa.x, oa.y, oa.w, oa.h), &ob.shape, Rect::new(ob.x, ob.y, ob.w, ob.h), op) else { return false };
        self.begin(Kind::Other);
        if let Some(o) = self.doc.object_mut(a) {
            o.shape = shape;
            o.x = r.x;
            o.y = r.y;
            o.w = r.w;
            o.h = r.h;
            o.corner = 0.0;
        }
        self.doc.objects.retain(|o| o.id != b);
        self.multi.clear();
        self.changed();
        true
    }

    // ------------------------------------------------------------ węzły

    pub fn shape_edit(&mut self, f: &dyn Fn(&mut Shape, Option<(usize, usize)>) -> Option<(usize, usize)>) {
        let Some(id) = self.obj else { return };
        let sel = self.sel_node;
        self.begin(Kind::Other);
        let mut new_sel = sel;
        if let Some(o) = self.doc.object_mut(id) {
            new_sel = f(&mut o.shape, sel);
            o.corner = 0.0;
        }
        self.sel_node = new_sel;
        self.changed();
    }

    /// Dopasowuje ramkę obiektu do kształtu po przesunięciu węzłów poza 0..1.
    pub fn normalize_shape(&mut self, id: u64) {
        let Some(o) = self.doc.object_mut(id) else { return };
        let mut x0 = f32::MAX;
        let mut y0 = f32::MAX;
        let mut x1 = f32::MIN;
        let mut y1 = f32::MIN;
        for c in &o.shape.contours {
            for n in &c.nodes {
                for (x, y) in [Some((n.x, n.y)), n.cin, n.cout].into_iter().flatten() {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        if x0 >= -0.0001 && y0 >= -0.0001 && x1 <= 1.0001 && y1 <= 1.0001 {
            return;
        }
        let (w, h) = ((x1 - x0).max(0.01), (y1 - y0).max(0.01));
        let nr = Rect::new(o.x + x0 * o.w, o.y + y0 * o.h, w * o.w, h * o.h);
        let f = |(x, y): (f32, f32)| ((x - x0) / w, (y - y0) / h);
        for c in o.shape.contours.iter_mut() {
            for n in c.nodes.iter_mut() {
                let (x, y) = f((n.x, n.y));
                n.x = x;
                n.y = y;
                n.cin = n.cin.map(f);
                n.cout = n.cout.map(f);
            }
        }
        o.x = nr.x;
        o.y = nr.y;
        o.w = nr.w;
        o.h = nr.h;
    }

    pub fn corner_reset(&mut self, id: u64) {
        if let Some(o) = self.doc.object_mut(id) {
            o.corner = 0.0;
        }
    }

    // ------------------------------------------------------------ inne

    pub fn insert_paragraphs_at_caret(&mut self, paras: Vec<Paragraph>) {
        if paras.is_empty() {
            return;
        }
        self.begin(Kind::Other);
        let h = self.head;
        let s = self.story_mut();
        let at = if s.paras[h.para].is_empty() { h.para } else { h.para + 1 };
        if s.paras[h.para].is_empty() {
            s.paras.remove(h.para);
        }
        let n = paras.len();
        for (i, p) in paras.into_iter().enumerate() {
            s.paras.insert(at + i, p);
        }
        let end = At::new(at + n - 1, s.paras[at + n - 1].len());
        self.anchor = end;
        self.head = end;
        self.changed();
    }

    /// Zakres śledzonej zmiany w miejscu kursora (ciągłe przebiegi z tym samym oznaczeniem).
    pub fn rev_at(&self) -> Option<(usize, usize, usize, Rev)> {
        let h = self.head;
        let p = self.story_ref().paras.get(h.para)?;
        let mut off = 0;
        let mut spans: Vec<(usize, usize, Option<Rev>)> = Vec::new();
        for r in &p.runs {
            let l = r.text.len();
            match spans.last_mut() {
                Some(last) if last.2 == r.fmt.rev => last.1 = off + l,
                _ => spans.push((off, off + l, r.fmt.rev.clone())),
            }
            off += l;
        }
        spans.into_iter().find_map(|(a, b, rev)| {
            let rev = rev?;
            (h.off >= a && h.off <= b).then_some((h.para, a, b, rev))
        })
    }

    /// Akceptacja albo odrzucenie zmiany w miejscu kursora.
    pub fn resolve_rev_here(&mut self, accept: bool) -> bool {
        let Some((p, a, b, rev)) = self.rev_at() else { return false };
        self.begin(Kind::Other);
        let remove = (rev.kind == RevKind::Delete) == accept;
        if remove {
            edit::delete(self.story_mut(), At::new(p, a), At::new(p, b));
            self.anchor = At::new(p, a);
            self.head = At::new(p, a);
        } else {
            edit::format_chars(self.story_mut(), At::new(p, a), At::new(p, b), &|f| f.rev = None);
        }
        self.changed();
        true
    }

    /// Następna śledzona zmiana w bieżącym tekście (od kursora, z zawinięciem) – zaznacza ją.
    pub fn next_rev(&mut self) -> bool {
        let h = self.head;
        let mut hits: Vec<(usize, usize, usize)> = Vec::new();
        for (pi, p) in self.story_ref().paras.iter().enumerate() {
            let mut off = 0;
            let mut prev: Option<&Rev> = None;
            for r in &p.runs {
                let l = r.text.len();
                if r.fmt.rev.is_some() {
                    match hits.last_mut() {
                        Some(last) if last.0 == pi && last.2 == off && prev == r.fmt.rev.as_ref() => last.2 = off + l,
                        _ => hits.push((pi, off, off + l)),
                    }
                }
                prev = r.fmt.rev.as_ref();
                off += l;
            }
        }
        let next = hits.iter().find(|x| (x.0, x.1) > (h.para, h.off)).or(hits.first()).copied();
        let Some((p, a, b)) = next else { return false };
        self.anchor = At::new(p, a);
        self.head = At::new(p, b);
        true
    }

    pub fn find_run(&mut self, needle: &str, opts: &edit::FindOpts) {
        self.find = edit::find_all_ex(self.story_ref(), needle, opts);
        let (a, _) = self.range();
        self.find_i = self.find.iter().position(|&(p, s, _)| (p, s) >= (a.para, a.off)).unwrap_or(0);
    }

    pub fn find_select(&mut self, delta: i32) -> bool {
        if self.find.is_empty() {
            return false;
        }
        let n = self.find.len() as i32;
        self.find_i = ((self.find_i as i32 + delta).rem_euclid(n)) as usize;
        let (p, a, b) = self.find[self.find_i];
        self.anchor = At::new(p, a);
        self.head = At::new(p, b);
        true
    }

    pub fn replace_all(&mut self, needle: &str, with: &str, opts: &edit::FindOpts) -> usize {
        let hits = edit::find_all_ex(self.story_ref(), needle, opts);
        if hits.is_empty() {
            return 0;
        }
        // Wyrażenie regularne: w zamienniku działają odwołania do grup ($1, ${nazwa}).
        let re = if opts.regex { opts.pattern(needle).ok() } else { None };
        self.begin(Kind::Other);
        for &(p, a, b) in hits.iter().rev() {
            let fmt = self.story_ref().paras[p].fmt_at(a + 1);
            let text = match &re {
                Some(re) => {
                    let found = self.story_ref().paras[p].text()[a..b].to_string();
                    re.replace(&found, with).to_string()
                }
                None => with.to_string(),
            };
            edit::delete(self.story_mut(), At::new(p, a), At::new(p, b));
            edit::insert_text(self.story_mut(), At::new(p, a), &text, &fmt);
        }
        self.changed();
        hits.len()
    }

    pub fn stats(&self) -> (usize, usize, usize, usize, usize) {
        let (w, ns, c) = edit::stats(&self.doc.body);
        let mut words = w;
        let mut nospace = ns;
        let mut chars = c;
        for o in &self.doc.objects {
            if let Some(s) = o.story() {
                let (a, b, cc) = edit::stats(s);
                words += a;
                nospace += b;
                chars += cc;
            }
        }
        (words, nospace, chars, self.doc.body.paras.len(), self.layout.pages.len())
    }

    /// Strona i prostokąt (pt) kursora.
    pub fn caret_page(&self) -> Option<(usize, f32, f32, f32)> {
        let c = self.layout.caret(self.story, self.head.para, self.head.off)?;
        Some((c.page, c.x, c.top, c.height))
    }
}
