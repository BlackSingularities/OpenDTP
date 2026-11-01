//! Kontrola przed drukiem (preflight): przepełnione ramki, brakujące pliki i czcionki, obrazy
//! o zbyt niskiej rozdzielczości, elementy przy krawędzi bez spadu, nierozwiązane odsyłacze.

use crate::layout::Layout;
use crate::model::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug)]
pub struct Issue {
    pub level: Level,
    pub text: String,
    pub page: Option<usize>,
    pub obj: Option<u64>,
}

fn issue(level: Level, text: String, page: Option<usize>, obj: Option<u64>) -> Issue {
    Issue { level, text, page, obj }
}

/// Wszystkie teksty dokumentu (treść, nagłówki, stopki, przypisy, ramki).
fn stories(doc: &Document) -> Vec<&Story> {
    let mut v: Vec<&Story> = vec![&doc.body];
    v.extend([&doc.header, &doc.footer, &doc.header_first, &doc.footer_first, &doc.header_even, &doc.footer_even].into_iter().flatten());
    v.extend(doc.notes.iter().map(|n| &n.story));
    v.extend(doc.objects.iter().filter_map(|o| o.story()));
    v
}

pub fn check(doc: &Document, layout: &Layout) -> Vec<Issue> {
    let tr = |s: &'static str| crate::lang::tr(s).to_string();
    let mut out = Vec::new();

    // Tekst, który nie zmieścił się w ramce.
    for &id in &layout.overflow {
        let page = doc.object(id).map(|o| o.page);
        out.push(issue(Level::Error, tr("Tekst nie mieści się w ramce (nadmiar tekstu)"), page, Some(id)));
    }

    // Obrazy: brak pliku, rozdzielczość, kolory RGB, krawędź bez spadu.
    let bleed = doc.print.bleed;
    let mut rgb_images = 0;
    for o in doc.objects.iter().filter(|o| o.cont_of.is_none()) {
        if let ObjKind::Image { asset, .. } = &o.kind {
            if doc.missing.contains(asset) {
                out.push(issue(Level::Error, tr("Brak pliku obrazu linkowanego – w druku będzie podgląd niskiej jakości"), Some(o.page), Some(o.id)));
            }
            let svg = doc.assets.get(asset).is_some_and(|a| a.mime == "image/svg+xml");
            if !svg {
                rgb_images += 1;
                if let Some((pw, _)) = doc.image_px(asset) {
                    let ppi = if o.w > 0.0 { pw as f32 / (o.w / 72.0) } else { 0.0 };
                    if ppi < 150.0 {
                        out.push(issue(Level::Error, crate::trf!("Obraz ma tylko {} ppi – w druku będzie nieostry (potrzeba 300)", ppi.round() as u32), Some(o.page), Some(o.id)));
                    } else if ppi < 250.0 {
                        out.push(issue(Level::Warning, crate::trf!("Obraz ma {} ppi – do druku zalecane 300", ppi.round() as u32), Some(o.page), Some(o.id)));
                    }
                }
            }
        }
        // Element dochodzi do krawędzi kartki, ale nie wychodzi na spad – po obcięciu zostanie biała szczelina.
        let (pw, ph) = layout.page_size(doc, o.page);
        let touches = |a: f32, edge: f32| (a - edge).abs() < 1.0;
        let edges = [touches(o.x, 0.0), touches(o.y, 0.0), touches(o.x + o.w, pw), touches(o.y + o.h, ph)];
        if bleed > 0.0 && edges.iter().any(|e| *e) && !matches!(o.kind, ObjKind::Text { .. }) {
            out.push(issue(Level::Warning, tr("Element dochodzi do krawędzi strony bez spadu – wysuń go 3 mm poza kartkę"), Some(o.page), Some(o.id)));
        }
        // Tekst za blisko krawędzi – może zostać ucięty.
        if matches!(o.kind, ObjKind::Text { .. } | ObjKind::PathText { .. }) {
            let safe = 3.0 * MM;
            if o.x < safe || o.y < safe || o.x + o.w > pw - safe || o.y + o.h > ph - safe {
                out.push(issue(Level::Warning, tr("Tekst leży bliżej niż 3 mm od krawędzi – może zostać ucięty"), Some(o.page), Some(o.id)));
            }
        }
    }
    if rgb_images > 0 && doc.print.cmyk {
        out.push(issue(Level::Info, crate::trf!("Obrazy w RGB: {} – drukarnia przeliczy je do CMYK (kolory mogą się nieco zmienić)", rgb_images), None, None));
    }

    // Czcionki, których nie ma w systemie (zastąpione innymi).
    let mut fonts: Vec<String> = Vec::new();
    let mut add = |f: &Option<String>| {
        if let Some(f) = f {
            if !f.is_empty() && !fonts.iter().any(|x| x.eq_ignore_ascii_case(f)) {
                fonts.push(f.clone());
            }
        }
    };
    for s in &doc.styles {
        add(&s.chars.font);
    }
    for st in stories(doc) {
        for p in &st.paras {
            for r in &p.runs {
                add(&r.fmt.font);
            }
        }
    }
    for f in fonts {
        if !crate::fonts::has_family(&f) {
            out.push(issue(Level::Error, crate::trf!("Brak czcionki „{}” – tekst jest wyświetlany zastępczą", f), None, None));
        }
    }

    // Odsyłacze do nieistniejących celów, podpisy bez numeru.
    let targets: std::collections::HashSet<u64> = doc.ref_targets().into_iter().map(|t| t.0).collect();
    for (i, p) in doc.body.paras.iter().enumerate() {
        for r in &p.runs {
            if let Some(Field::Ref(id, _)) = r.field {
                if !targets.contains(&id) {
                    let page = layout.body_pages.get(i).copied();
                    out.push(issue(Level::Error, tr("Odsyłacz do usuniętej zakładki albo podpisu (pokazuje „?”)"), page, None));
                }
            }
        }
    }

    // Recenzja: niezaakceptowane zmiany drukują się jako podkreślenia i przekreślenia.
    let revs = doc.rev_count();
    if revs > 0 {
        out.push(issue(Level::Error, crate::trf!("Niezaakceptowane zmiany: {} – zaakceptuj albo odrzuć je przed drukiem", revs), None, None));
    }
    let open = doc.comments.iter().filter(|c| !c.resolved).count();
    if open > 0 {
        out.push(issue(Level::Info, crate::trf!("Otwarte komentarze: {} (nie drukują się)", open), None, None));
    }

    // Spis treści i indeks mogą być nieaktualne – przypomnienie.
    if doc.body.paras.iter().any(|p| p.style.starts_with("toc")) {
        out.push(issue(Level::Info, tr("Przed drukiem zaktualizuj spis treści (Odwołania → Aktualizuj)"), None, None));
    }
    if doc.body.paras.iter().any(|p| p.style.starts_with("index")) {
        out.push(issue(Level::Info, tr("Przed drukiem odśwież indeks (Odwołania → Indeks)"), None, None));
    }
    let spots = doc.swatches.iter().filter(|s| s.spot).count();
    if spots > 0 {
        out.push(issue(Level::Info, crate::trf!("Kolory dodatkowe (osobne farby): {}", spots), None, None));
    }

    out.sort_by_key(|i| (i.level, i.page.unwrap_or(usize::MAX)));
    out
}
