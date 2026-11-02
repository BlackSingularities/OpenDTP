//! Polecenia ustawień, sztucznej inteligencji i pisowni z menu.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle(st: &mut State, ui: &crate::AppWindow, app: &App, unit: i32, uk: f32, is_step: bool, base: &str, id: &str, arg: &str, scroll: &mut bool) -> Flow {
    let _ = (ui, unit, uk, is_step, id, &scroll);
    match base {
        // ---------------------------------------------------------- ustawienia
        "s-lang" => {
            st.prefs.lang = match arg {
                "1" => "pl".into(),
                "2" => "en".into(),
                _ => String::new(),
            };
            st.prefs.save();
            app.set_s_lang(arg.parse().unwrap_or(0));
            toast(st, tr("Język zmieni się po ponownym uruchomieniu"));
            return Flow::Skip;
        }
        "s-screen-ppi" => {
            st.prefs.screen_ppi = parse_num(arg).filter(|v| (40.0..=600.0).contains(v)).unwrap_or(0.0);
            st.prefs.save();
            crate::apply_screen_ppi(st, &ui);
            st.view.place(&st.ed);
        }
        "s-unit" => {
            st.prefs.unit = arg.parse().unwrap_or(0);
            st.prefs.save();
            app.set_s_unit(st.prefs.unit);
        }
        "s-autosave" => {
            st.prefs.autosave = arg.parse().unwrap_or(2);
            st.prefs.save();
            app.set_s_autosave(st.prefs.autosave);
            return Flow::Skip;
        }
        "s-update-check" | "s-update-auto" | "s-smart-quotes" | "s-autocorrect" | "s-capitalize" | "s-autolist" | "s-restore" | "s-snap" => {
            let p = &mut st.prefs;
            let v = match base {
                "s-update-check" => &mut p.update_check,
                "s-update-auto" => &mut p.update_auto,
                "s-smart-quotes" => &mut p.smart_quotes,
                "s-autocorrect" => &mut p.autocorrect,
                "s-capitalize" => &mut p.capitalize,
                "s-autolist" => &mut p.autolist,
                "s-restore" => &mut p.restore,
                _ => &mut p.snap,
            };
            *v = !*v;
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return Flow::Skip;
        }
        "s-author" => {
            st.prefs.author = arg.to_string();
            st.ed.doc.meta.author = arg.to_string();
            st.prefs.save();
            return Flow::Skip;
        }
        "s-default-font" => {
            st.prefs.default_font = arg.to_string();
            st.prefs.save();
            return Flow::Skip;
        }
        "s-default-size" => {
            if let Some(v) = parse_num(arg) {
                st.prefs.default_size = v.clamp(4.0, 96.0);
                st.prefs.save();
            }
            return Flow::Skip;
        }

        // ---------------------------------------------------------- sztuczna inteligencja
        "ai" => {
            if st.prefs.ai_model.is_empty() || (opendtp_core::ai::provider(&st.prefs.ai_provider).needs_key && st.prefs.ai_key.is_empty()) {
                app.set_ai_status(tr("Najpierw ustaw dostawcę, klucz API i model").into());
            }
            app.set_backstage(false);
            app.set_dialog("ai".into());
            return Flow::Skip;
        }
        "ai-provider" => {
            st.prefs.ai_provider = arg.to_string();
            st.prefs.ai_base = String::new();
            st.prefs.ai_model = String::new();
            st.prefs.save();
            app.set_ai_models(ModelRc::new(VecModel::from(Vec::<ItemV>::new())));
            app.set_ai_status("".into());
            crate::sync_ai(st, &ui);
            return Flow::Skip;
        }
        "ai-key" => {
            if !arg.trim().is_empty() {
                st.prefs.ai_key = crate::settings::protect(arg.trim());
                st.prefs.save();
                app.set_ai_key("".into());
                app.set_ai_has_key(true);
                toast(st, tr("Klucz API zapisany (zaszyfrowany)"));
            }
            return Flow::Skip;
        }
        "ai-base" => {
            st.prefs.ai_base = arg.trim().to_string();
            st.prefs.save();
            return Flow::Skip;
        }
        "ai-model" => {
            st.prefs.ai_model = arg.trim().to_string();
            st.prefs.save();
            app.set_ai_model(st.prefs.ai_model.clone().into());
            app.set_ai_status(opendtp_core::trf!("Model: {}", st.prefs.ai_model).into());
            return Flow::Skip;
        }
        "ai-temp" => {
            st.prefs.ai_temp = arg.parse::<f32>().unwrap_or(0.7).clamp(0.0, 2.0);
            st.prefs.save();
            app.set_ai_temp(st.prefs.ai_temp);
            return Flow::Skip;
        }
        "ai-models" => {
            let cfg = ai_config(st);
            app.set_ai_busy(true);
            app.set_ai_status(tr("Pobieranie listy modeli…").into());
            std::thread::spawn(move || {
                let res = opendtp_core::ai::list_models(&cfg);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(sh) = crate::shared() else { return };
                    let st = sh.borrow();
                    let Some(ui) = st.ui.upgrade() else { return };
                    let app = ui.global::<App>();
                    app.set_ai_busy(false);
                    match res {
                        Ok(list) => {
                            let n = list.len();
                            let items: Vec<ItemV> = list.into_iter().map(|m| ItemV { id: m.clone().into(), label: m.into(), ..Default::default() }).collect();
                            app.set_ai_models(ModelRc::new(VecModel::from(items)));
                            app.set_ai_status(opendtp_core::trf!("Dostępne modele: {}", n).into());
                        }
                        Err(e) => app.set_ai_status(opendtp_core::trf!("Nie udało się pobrać modeli: {}", e).into()),
                    }
                });
            });
            return Flow::Skip;
        }
        "ai-generate" => {
            let insert = app.get_ai_insert();
            if !insert && !files::may_replace(st, "ai-generate") {
                app.set_dialog("ai".into());
                return Flow::Skip;
            }
            let prompt = opendtp_core::ai::prompt(&app.get_ai_prompt(), app.get_ai_length());
            let cfg = ai_config(st);
            let template = files::blank(&st.prefs);
            app.set_dialog("ai".into());
            app.set_ai_busy(true);
            std::thread::spawn(move || {
                let res = opendtp_core::ai::complete(&cfg, &prompt).map(|answer| opendtp_core::ai::build(&answer, &template));
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(sh) = crate::shared() else { return };
                    let mut st = sh.borrow_mut();
                    let Some(ui) = st.ui.upgrade() else { return };
                    let app = ui.global::<App>();
                    app.set_ai_busy(false);
                    match res {
                        Ok(doc) => {
                            app.set_dialog("".into());
                            if insert {
                                let paras = doc.body.paras.clone();
                                st.ed.insert_paragraphs_at_caret(paras);
                                refresh_scroll(&mut st);
                            } else {
                                files::set_doc(&mut st, doc, None);
                                st.ed.dirty = true;
                                refresh(&mut st);
                            }
                            toast(&st, tr("Dokument wygenerowany – możesz go dowolnie edytować"));
                        }
                        Err(e) => app.set_ai_status(opendtp_core::trf!("Błąd: {}", e).into()),
                    }
                });
            });
            return Flow::Skip;
        }

        // ---------------------------------------------------------- pisownia z menu
        _ if id.starts_with("spell-fix:") => {
            let rest = &id["spell-fix:".len()..];
            let mut it = rest.splitn(3, ':');
            let a: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let b: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let word = it.next().unwrap_or("").to_string();
            let p = st.ed.head.para;
            st.ed.anchor = At::new(p, a);
            st.ed.head = At::new(p, b);
            st.ed.insert(&word);
        }
        _ if id.starts_with("spell-add:") => {
            opendtp_core::spell::add_to_user(&id["spell-add:".len()..]);
            st.view.invalidate();
        }
        _ if id.starts_with("spell-ignore:") => {
            opendtp_core::spell::ignore_word(&id["spell-ignore:".len()..]);
            st.view.invalidate();
        }
        _ => return Flow::Unhandled,
    }
    Flow::Done
}
