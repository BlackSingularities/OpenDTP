//! Polecenia obiektów: położenie, rozmiar, warstwy, wyrównanie, kształty.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle(st: &mut State, ui: &crate::AppWindow, app: &App, unit: i32, uk: f32, is_step: bool, base: &str, id: &str, arg: &str, scroll: &mut bool) -> Flow {
    let _ = (ui, unit, uk, is_step, id, &scroll);
    match base {
        // ---------------------------------------------------------- obiekty
        "wrap" => {
            let m = wrap_mode(arg.parse().unwrap_or(0));
            if st.ed.obj.is_none() {
                toast(st, tr("Najpierw zaznacz obiekt"));
                return Flow::Skip;
            }
            st.ed.edit_obj(&move |o| o.wrap.mode = m);
        }
        "wrap-dist" => {
            let cur = st.ed.object().map(|o| o.wrap.dist).unwrap_or(9.0);
            if let Some(v) = value_or_step(arg, cur, 2.0, is_step) {
                st.ed.edit_obj(&move |o| o.wrap.dist = v.max(0.0));
            }
        }
        "wrap-contour" => st.ed.edit_obj(&|o| o.wrap.contour = !o.wrap.contour),
        "detect-contour" => {
            let Some(o) = st.ed.object().cloned() else { return Flow::Skip };
            let ObjKind::Image { asset, .. } = &o.kind else {
                toast(st, tr("Kontur wykrywamy dla obrazów"));
                return Flow::Skip;
            };
            let Some(a) = st.ed.doc.assets.get(asset) else { return Flow::Skip };
            let found = image::load_from_memory(&a.data).ok().map(|i| i.to_rgba8()).and_then(|img| {
                let (w, h) = img.dimensions();
                geom::image_contour(img.as_raw(), w, h)
            });
            match found {
                Some(c) => {
                    st.ed.edit_obj(&move |o| {
                        if let ObjKind::Image { contour, .. } = &mut o.kind {
                            *contour = Some(c.clone());
                        }
                        o.wrap.contour = true;
                        if !o.wrap.mode.excludes() {
                            o.wrap.mode = WrapMode::Both;
                        }
                    });
                    toast(st, tr("Wykryto kontur – tekst opływa kształt obrazu"));
                }
                None => toast(st, tr("Nie znaleziono konturu (obraz bez przezroczystości i jednolitego tła)")),
            }
        }
        "z-front" => st.ed.z_order("front"),
        "z-back" => st.ed.z_order("back"),
        "z-forward" => st.ed.z_order("forward"),
        "z-backward" => st.ed.z_order("backward"),
        "align-obj" => st.ed.align_obj(arg),
        "duplicate" => {
            if st.ed.obj.is_some() {
                st.ed.duplicate();
            }
        }
        "delete-obj" => st.ed.delete_object(),
        "lock" => st.ed.edit_obj(&|o| o.locked = !o.locked),
        "flip-h" => st.ed.flip(false),
        "flip-v" => st.ed.flip(true),
        "obj-x" | "obj-y" | "obj-w" | "obj-h" => {
            let Some(o) = st.ed.object().cloned() else { return Flow::Skip };
            let cur = match base {
                "obj-x" => o.x,
                "obj-y" => o.y,
                "obj-w" => o.w,
                _ => o.h,
            } / uk;
            let step = if unit == 0 { 1.0 } else if unit == 3 { 2.0 } else { 0.1 };
            if let Some(v) = value_or_step(arg, cur, step, is_step) {
                let v = v * uk;
                let b = base.to_string();
                st.ed.edit_obj(&move |o| match b.as_str() {
                    "obj-x" => o.x = v,
                    "obj-y" => o.y = v,
                    "obj-w" => o.w = v.max(2.0),
                    _ => o.h = v.max(2.0),
                });
            }
        }
        "obj-name" => {
            let n = arg.to_string();
            st.ed.edit_obj(&move |o| o.name = n.clone());
        }
        "fill" => {
            let c = color_of(app.get_fill());
            let kind = app.get_fill_kind();
            let c2 = color_of(app.get_fill2());
            st.ed.edit_obj(&move |o| {
                o.fill = if c.is_none() {
                    None
                } else {
                    Some(match kind {
                        1 => Fill::Linear { from: c, to: c2, angle: 90.0 },
                        2 => Fill::Radial { from: c, to: c2 },
                        _ => Fill::Solid { color: c },
                    })
                }
            });
        }
        "fill-kind" => {
            let k: i32 = arg.parse().unwrap_or(0);
            st.ed.edit_obj(&move |o| {
                let base_c = match &o.fill {
                    Some(Fill::Solid { color }) => *color,
                    Some(Fill::Linear { from, .. }) | Some(Fill::Radial { from, .. }) => *from,
                    None => Color::rgb(0x5b, 0x8c, 0xff),
                };
                let second = Color::rgb((base_c.r as u16 * 6 / 10) as u8, (base_c.g as u16 * 6 / 10) as u8, (base_c.b as u16 * 6 / 10) as u8);
                o.fill = Some(match k {
                    1 => Fill::Linear { from: base_c, to: second, angle: 90.0 },
                    2 => Fill::Radial { from: Color::WHITE, to: base_c },
                    _ => Fill::Solid { color: base_c },
                });
            });
        }
        "stroke" => {
            let c = color_of(app.get_stroke());
            st.ed.edit_obj(&move |o| {
                if c.is_none() {
                    o.stroke = None;
                } else {
                    let w = o.stroke.as_ref().map(|s| s.width).filter(|w| *w > 0.0).unwrap_or(1.0);
                    let dash = o.stroke.as_ref().map(|s| s.dash.clone()).unwrap_or_default();
                    o.stroke = Some(StrokeStyle { color: c, width: w, dash });
                }
            });
        }
        "stroke-w" => {
            let cur = st.ed.object().and_then(|o| o.stroke.as_ref().map(|s| s.width)).unwrap_or(0.0);
            if let Some(v) = value_or_step(arg, cur, 0.5, is_step) {
                let v = v.max(0.0);
                st.ed.edit_obj(&move |o| match &mut o.stroke {
                    Some(s) => s.width = v,
                    None if v > 0.0 => o.stroke = Some(StrokeStyle { color: Color::BLACK, width: v, dash: vec![] }),
                    None => {}
                });
            }
        }
        "dashed" => st.ed.edit_obj(&|o| {
            if let Some(s) = &mut o.stroke {
                s.dash = if s.dash.len() >= 2 { vec![] } else { vec![s.width * 3.0, s.width * 2.0] };
            } else {
                o.stroke = Some(StrokeStyle { color: Color::BLACK, width: 1.0, dash: vec![3.0, 2.0] });
            }
        }),
        "shadow" => st.ed.edit_obj(&|o| o.shadow = !o.shadow),
        "opacity" => {
            let v: f32 = arg.parse().unwrap_or(1.0);
            let id = st.ed.obj;
            if let Some(id) = id {
                if !matches!(st.drag, crate::input::Drag::None) {
                    return Flow::Skip;
                }
                st.ed.begin(Kind::Typing);
                st.ed.drag_obj(id, &move |o| o.opacity = v.clamp(0.05, 1.0));
                st.ed.changed();
            }
        }
        "corner" => {
            let cur = st.ed.object().map(|o| o.corner).unwrap_or(0.0);
            if let Some(v) = value_or_step(arg, cur, 2.0, is_step) {
                st.ed.edit_obj(&move |o| o.corner = v.max(0.0));
            }
        }
        "padding" => {
            let cur = match st.ed.object().map(|o| &o.kind) {
                Some(ObjKind::Text { padding, .. }) => *padding,
                _ => 4.0,
            };
            if let Some(v) = value_or_step(arg, cur, 1.0, is_step) {
                st.ed.edit_obj(&move |o| {
                    if let ObjKind::Text { padding, .. } = &mut o.kind {
                        *padding = v.max(0.0);
                    }
                });
            }
        }
        "frame-cols" => {
            let cur = match st.ed.object().map(|o| &o.kind) {
                Some(ObjKind::Text { columns, .. }) => *columns as f32,
                _ => 1.0,
            };
            if let Some(v) = value_or_step(arg, cur, 1.0, is_step) {
                st.ed.edit_obj(&move |o| {
                    if let ObjKind::Text { columns, .. } = &mut o.kind {
                        *columns = v.round().clamp(1.0, 8.0) as u8;
                    }
                });
            }
        }
        "valign" => {
            let v = match arg {
                "1" => VAlign::Center,
                "2" => VAlign::Bottom,
                _ => VAlign::Top,
            };
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Text { valign, .. } = &mut o.kind {
                    *valign = v;
                }
            });
        }
        "image-fit" => {
            let cover = arg == "0";
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Image { fit, .. } = &mut o.kind {
                    *fit = if cover { ImageFit::Cover } else { ImageFit::Contain };
                }
            });
        }
        "link-frame" => {
            if !matches!(st.ed.object().map(|o| &o.kind), Some(ObjKind::Text { .. })) {
                toast(st, tr("Zaznacz ramkę tekstową"));
                return Flow::Skip;
            }
            set_tool(st, 5);
            toast(st, tr("Kliknij ramkę, do której ma popłynąć dalszy tekst"));
            return Flow::Skip;
        }
        "unlink-frame" => {
            let Some(id) = st.ed.obj else { return Flow::Skip };
            st.ed.begin(Kind::Other);
            let head = st.ed.doc.chain_head(id);
            let chain = st.ed.doc.chain(head);
            for c in chain {
                if let Some(ObjKind::Text { next, .. }) = st.ed.doc.object_mut(c).map(|o| &mut o.kind) {
                    *next = None;
                }
            }
            st.ed.changed();
        }
        "bool" => {
            let op = match arg {
                "difference" => BoolOp::Difference,
                "intersect" => BoolOp::Intersect,
                "xor" => BoolOp::Xor,
                _ => BoolOp::Union,
            };
            if !st.ed.boolean(op) {
                toast(st, tr("Zaznacz dwa kształty na tej samej stronie (drugi z Shift)"));
            }
        }
        "node-mode" => {
            if st.ed.obj.is_some() {
                st.ed.node_mode = !st.ed.node_mode;
                st.ed.sel_node = None;
                if let StoryRef::Obj(_) = st.ed.story {
                    st.ed.story = StoryRef::Body;
                    st.ed.fix_selection();
                }
            }
        }
        "node-add" => st.ed.shape_edit(&|s, sel| {
            let (ci, ni) = sel.unwrap_or((0, 0));
            if s.contours.get(ci).is_some_and(|c| c.nodes.len() >= 2) {
                geom::insert_node(s, ci, ni, 0.5);
                Some((ci, ni + 1))
            } else {
                sel
            }
        }),
        "node-delete" => st.ed.shape_edit(&|s, sel| {
            if let Some((ci, ni)) = sel {
                if let Some(c) = s.contours.get_mut(ci) {
                    if c.nodes.len() > if c.closed { 3 } else { 2 } {
                        c.nodes.remove(ni);
                        return Some((ci, ni.min(c.nodes.len() - 1)));
                    }
                }
            }
            sel
        }),
        "node-smooth" => st.ed.shape_edit(&|s, sel| {
            if let Some((ci, ni)) = sel {
                geom::toggle_smooth(s, ci, ni);
            }
            sel
        }),
        "seg-line" | "seg-curve" => {
            let curve = base == "seg-curve";
            st.ed.shape_edit(&move |s, sel| {
                let Some((ci, ni)) = sel else { return sel };
                let Some(c) = s.contours.get_mut(ci) else { return sel };
                let n = c.nodes.len();
                let j = (ni + 1) % n;
                let (a, b) = (c.nodes[ni], c.nodes[j]);
                if curve {
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    c.nodes[ni].cout = Some((a.x + dx / 3.0 - dy * 0.25, a.y + dy / 3.0 + dx * 0.25));
                    c.nodes[j].cin = Some((a.x + dx * 2.0 / 3.0 - dy * 0.25, a.y + dy * 2.0 / 3.0 + dx * 0.25));
                } else {
                    c.nodes[ni].cout = None;
                    c.nodes[j].cin = None;
                }
                sel
            });
            if let Some(id) = st.ed.obj {
                st.ed.normalize_shape(id);
                st.ed.relayout();
            }
        }
        "chart-kind" => {
            let k = match arg {
                "1" => ChartKind::Bar,
                "2" => ChartKind::Line,
                "3" => ChartKind::Area,
                "4" => ChartKind::Pie,
                "5" => ChartKind::Doughnut,
                _ => ChartKind::Column,
            };
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Chart { chart } = &mut o.kind {
                    chart.kind = k;
                }
            });
        }
        "chart-title" => {
            let t = arg.to_string();
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Chart { chart } = &mut o.kind {
                    chart.title = t.clone();
                }
            });
        }
        "chart-legend" => st.ed.edit_obj(&|o| {
            if let ObjKind::Chart { chart } = &mut o.kind {
                chart.legend = !chart.legend;
            }
        }),
        "chart-values" => st.ed.edit_obj(&|o| {
            if let ObjKind::Chart { chart } = &mut o.kind {
                chart.values = !chart.values;
            }
        }),
        "chart-data" => {
            let (labels, series) = parse_chart(arg);
            if labels.is_empty() {
                toast(st, tr("Nie rozpoznano danych wykresu"));
                return Flow::Skip;
            }
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Chart { chart } = &mut o.kind {
                    chart.labels = labels.clone();
                    chart.series = series.clone();
                }
            });
            toast(st, tr("Dane wykresu zastosowane"));
        }
        "path-text-set" => {
            let t = arg.to_string();
            let Some(id) = st.ed.obj else { return Flow::Skip };
            st.ed.begin(Kind::Typing);
            st.ed.drag_obj(id, &move |o| {
                if let ObjKind::PathText { story, .. } = &mut o.kind {
                    let p = &mut story.paras[0];
                    let fmt = p.runs.first().map(|r| r.fmt.clone()).unwrap_or_default();
                    p.runs = vec![Run { text: t.clone(), fmt, field: None }];
                }
            });
            st.ed.changed();
        }
        "path-offset" => {
            let v: f32 = arg.parse().unwrap_or(0.0);
            let Some(id) = st.ed.obj else { return Flow::Skip };
            st.ed.begin(Kind::Typing);
            st.ed.drag_obj(id, &move |o| {
                if let ObjKind::PathText { offset, .. } = &mut o.kind {
                    *offset = v - 0.5;
                }
            });
            st.ed.changed();
        }

        _ => return Flow::Unhandled,
    }
    Flow::Done
}
