//! Polecenia formatowania znaków i akapitów.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle(st: &mut State, ui: &crate::AppWindow, app: &App, unit: i32, uk: f32, is_step: bool, base: &str, id: &str, arg: &str, scroll: &mut bool) -> Flow {
    let _ = (ui, unit, uk, is_step, id, &scroll);
    match base {
        // ---------------------------------------------------------- znaki
        "bold" => st.ed.toggle(|c| c.bold, |f, v| f.bold = Some(v)),
        "italic" => st.ed.toggle(|c| c.italic, |f, v| f.italic = Some(v)),
        "underline" => st.ed.toggle(|c| c.underline != Underline::None, |f, v| f.underline = Some(if v { Underline::Single } else { Underline::None })),
        "underline-kind" => {
            let u = match arg {
                "double" => Underline::Double,
                "dotted" => Underline::Dotted,
                "wave" => Underline::Wave,
                "none" => Underline::None,
                _ => Underline::Single,
            };
            st.ed.format(&move |f| f.underline = Some(u));
        }
        "strike" => st.ed.toggle(|c| c.strike, |f, v| f.strike = Some(v)),
        "sup" => st.ed.toggle(|c| c.script == Script::Super, |f, v| f.script = Some(if v { Script::Super } else { Script::Normal })),
        "sub" => st.ed.toggle(|c| c.script == Script::Sub, |f, v| f.script = Some(if v { Script::Sub } else { Script::Normal })),
        "caps" => match arg {
            "0" | "1" | "2" => {
                let c = match arg {
                    "1" => Caps::All,
                    "2" => Caps::Small,
                    _ => Caps::None,
                };
                st.ed.format(&move |f| f.caps = Some(c));
            }
            how => change_case(st, how),
        },
        "font" => {
            let f = arg.to_string();
            st.prefs.add_recent_font(&f);
            sync_lists(st);
            st.ed.format(&move |x| x.font = Some(f.clone()));
            app.set_focus_serial(app.get_focus_serial() + 1);
        }
        "size" => {
            if let Some(v) = parse_num(arg) {
                let v = v.clamp(1.0, 1600.0);
                st.ed.format(&move |f| f.size = Some(v));
            }
            app.set_focus_serial(app.get_focus_serial() + 1);
        }
        "grow" | "shrink" => {
            let (cp, _, _) = st.ed.caret_props();
            let steps = [6.0, 7.0, 8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 28.0, 32.0, 36.0, 40.0, 48.0, 54.0, 60.0, 72.0, 96.0, 120.0, 144.0];
            let cur = cp.size;
            let v = if base == "grow" { steps.iter().copied().find(|s| *s > cur + 0.01).unwrap_or(cur + 12.0) } else { steps.iter().rev().copied().find(|s| *s < cur - 0.01).unwrap_or((cur - 1.0).max(1.0)) };
            st.ed.format(&move |f| f.size = Some(v));
        }
        "clear-format" => st.ed.clear_format(),
        "color" => {
            let c = color_of(app.get_last_color());
            st.ed.format(&move |f| f.color = Some(c));
        }
        "highlight" => {
            let c = color_of(app.get_last_highlight());
            st.ed.format(&move |f| f.highlight = Some(c));
        }
        "pick-hex" | "color-hex-check" => {
            if let Some(c) = Color::parse(arg) {
                app.set_last_color(slint::Color::from_rgb_u8(c.r, c.g, c.b));
                st.ed.format(&move |f| f.color = Some(c));
            } else {
                toast(st, tr("Nieprawidłowy kolor – wpisz np. #1f4e79"));
                return Flow::Skip;
            }
        }
        "tracking" => {
            let (cp, _, _) = st.ed.caret_props();
            if let Some(v) = value_or_step(arg, cp.tracking, 0.5, is_step) {
                st.ed.format(&move |f| f.tracking = Some(v));
            }
        }
        "lang" => {
            let l = arg.to_string();
            st.ed.format(&move |f| f.lang = Some(l.clone()));
            st.view.invalidate();
        }
        "link" => {
            let f = st.ed.caret_fmt();
            app.set_link_url(f.link.unwrap_or_default().into());
            app.set_dialog("link".into());
            return Flow::Skip;
        }
        "link-apply" => {
            app.set_dialog("".into());
            let url = arg.trim().to_string();
            if url.is_empty() {
                st.ed.format(&|f| {
                    f.link = Some(String::new());
                    f.underline = None;
                    f.color = None;
                });
            } else {
                let url = if url.contains("://") || url.starts_with("mailto:") { url } else { format!("https://{url}") };
                if !st.ed.has_sel() {
                    st.ed.insert(&url);
                    let h = st.ed.head;
                    st.ed.anchor = At::new(h.para, h.off - url.len());
                }
                st.ed.format(&move |f| {
                    f.link = Some(url.clone());
                    f.underline = Some(Underline::Single);
                    f.color = Some(Color::rgb(0x05, 0x63, 0xc1));
                });
            }
        }

        // ---------------------------------------------------------- akapit
        "align" => {
            let a = match arg {
                "1" => Align::Center,
                "2" => Align::Right,
                "3" => Align::Justify,
                _ => Align::Left,
            };
            st.ed.format_paras(&move |p| p.fmt.align = Some(a));
        }
        "list" => match arg {
            "restart" => st.ed.restart_numbering(),
            k => st.ed.set_list(list_kind(k)),
        },
        "indent" => st.ed.indent(1),
        "outdent" => st.ed.indent(-1),
        "line-spacing" => match arg {
            "exact" => {
                let (cp, _, _) = st.ed.caret_props();
                let v = cp.size * 1.2;
                st.ed.format_paras(&move |p| p.fmt.line = Some(LineSpacing::Exact(v)));
            }
            "before+" => st.ed.format_paras(&|p| p.fmt.space_before = Some(12.0)),
            "after0" => st.ed.format_paras(&|p| p.fmt.space_after = Some(0.0)),
            v => {
                if let Some(k) = parse_num(v) {
                    st.ed.format_paras(&move |p| p.fmt.line = Some(LineSpacing::Multiple(k)));
                }
            }
        },
        "shading" => {
            let c = color_of(app.get_shading());
            st.ed.format_paras(&move |p| p.fmt.shading = Some(c));
        }
        "indent-left" | "indent-right" | "first-line" | "space-before" | "space-after" => {
            let (_, pp, _) = st.ed.caret_props();
            let (cur, k, step) = match base {
                "indent-left" => (pp.indent_left / uk, uk, 1.0),
                "indent-right" => (pp.indent_right / uk, uk, 1.0),
                "first-line" => (pp.first_line / uk, uk, 1.0),
                "space-before" => (pp.space_before, 1.0, 2.0),
                _ => (pp.space_after, 1.0, 2.0),
            };
            let step = if k > 1.0 && unit == 0 { 2.5 } else if k > 1.0 && unit == 2 { 0.1 } else { step };
            if let Some(v) = value_or_step(arg, cur, step, is_step) {
                let v = v * k;
                let b = base.to_string();
                st.ed.format_paras(&move |p| match b.as_str() {
                    "indent-left" => p.fmt.indent_left = Some(v.max(0.0)),
                    "indent-right" => p.fmt.indent_right = Some(v.max(0.0)),
                    "first-line" => p.fmt.first_line = Some(v),
                    "space-before" => p.fmt.space_before = Some(v.max(0.0)),
                    _ => p.fmt.space_after = Some(v.max(0.0)),
                });
            }
        }
        "drop-cap" => {
            let (_, pp, _) = st.ed.caret_props();
            let on = pp.drop_cap == 0;
            st.ed.format_paras(&move |p| p.fmt.drop_cap = Some(if on { 3 } else { 0 }));
        }
        "para-border" => {
            let b = match arg {
                "thin" => Some(BoxBorder { width: 0.75, color: Color::rgb(0x40, 0x40, 0x40), radius: 0.0, padding: 4.0 }),
                "thick" => Some(BoxBorder { width: 2.25, color: Color::rgb(0x40, 0x40, 0x40), radius: 0.0, padding: 5.0 }),
                "round" => Some(BoxBorder { width: 1.0, color: Color::rgb(0x40, 0x40, 0x40), radius: 6.0, padding: 6.0 }),
                _ => None,
            };
            st.ed.format_paras(&move |p| p.fmt.border = Some(b));
        }
        "sort" => st.ed.sort_paras(arg == "desc"),
        "effect" => {
            let (cp, _, _) = st.ed.caret_props();
            let e = match arg {
                "shadow" => TextEffect::Shadow,
                "glow" => TextEffect::Glow,
                _ => TextEffect::None,
            };
            // Ponowny wybór tego samego efektu go zdejmuje.
            let e = if cp.effect == e { TextEffect::None } else { e };
            st.ed.format(&move |f| f.effect = Some(e));
        }
        "outline-text" => {
            let (cp, _, _) = st.ed.caret_props();
            let v = if cp.outline > 0.0 { 0.0 } else { 0.6 };
            st.ed.format(&move |f| f.outline = Some(v));
        }
        "goto" => {
            app.set_left_panel(if arg == "comments" { "comments" } else { "nav" }.into());
        }
        "style" => {
            st.ed.set_style(arg);
        }
        "style-update" => {
            let sid = if arg.is_empty() { st.ed.caret_props().2 } else { arg.to_string() };
            let f = st.ed.caret_fmt();
            st.ed.begin(Kind::Other);
            if let Some(s) = st.ed.doc.styles.iter_mut().find(|s| s.id == sid) {
                s.chars.merge(&f);
            }
            st.ed.changed();
            toast(st, tr("Styl zaktualizowany według zaznaczenia"));
        }
        "style-new" => {
            let f = st.ed.caret_fmt();
            let (_, _, cur) = st.ed.caret_props();
            let n = st.ed.doc.styles.len();
            let id = format!("custom{}", st.ed.doc.new_id());
            let name = opendtp_core::trf!("Mój styl {}", n - 14);
            st.ed.begin(Kind::Other);
            st.ed.doc.styles.push(Style { id: id.clone(), name, based_on: Some(cur), next: None, para: ParaFmt::default(), chars: f, outline: None, gallery: true });
            st.ed.changed();
            st.ed.set_style(&id);
            app.set_left_panel("styles".into());
        }

        _ => return Flow::Unhandled,
    }
    Flow::Done
}
