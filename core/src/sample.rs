//! Szablony dokumentów i przewodnik „Witaj w OpenDTP”, który pokazuje możliwości programu:
//! tekst w kształtach i na ścieżce, opływanie po konturze, sekcje z kolumnami, typografię,
//! tabele, wykresy z podpisami i odsyłaczami, przypisy, gradienty i grafikę wektorową.
//! Służy też testom składu.

use crate::geom;
use crate::model::*;

// ------------------------------------------------------------------ pomocniki

fn para(style: &str, t: &str) -> Paragraph {
    Paragraph::with_text(style, t)
}

/// Akapit z prostym formatowaniem: **pogrubienie**, *kursywa*, ==wyróżnienie==, [łącze](adres).
fn rich(style: &str, md: &str) -> Paragraph {
    Paragraph { style: style.into(), fmt: ParaFmt::default(), runs: crate::io::plain::md_inline(md) }
}

fn run(t: &str, fmt: CharFmt) -> Run {
    Run { text: t.into(), fmt, field: None }
}

fn plain(t: &str) -> Run {
    run(t, CharFmt::default())
}

fn field(f: Field, fmt: CharFmt) -> Run {
    Run { text: FIELD_CHAR.to_string(), fmt, field: Some(f) }
}

/// Akapit z gotowych przebiegów.
fn runs(style: &str, r: Vec<Run>) -> Paragraph {
    Paragraph { style: style.into(), fmt: ParaFmt::default(), runs: r }
}

/// Akapit ramki: tekst, rozmiar, kolor, pogrubienie, wyrównanie.
fn line(t: &str, size: f32, color: Color, bold: bool, align: Align) -> Paragraph {
    let mut p = para("normal", t);
    p.fmt.align = Some(align);
    p.fmt.space_after = Some(0.0);
    p.fmt.space_before = Some(0.0);
    p.fmt.first_line = Some(0.0);
    p.fmt.drop_cap = Some(0);
    p.runs[0].fmt = CharFmt { size: Some(size), color: Some(color), bold: Some(bold), ..Default::default() };
    p
}

/// Stopka „Strona X z Y” na środku.
fn footer_of(color: Color) -> Story {
    let f = CharFmt { size: Some(8.5), color: Some(color), ..Default::default() };
    let mut p = Paragraph {
        style: "footer".into(),
        fmt: ParaFmt::default(),
        runs: vec![run("Strona ", f.clone()), field(Field::PageNumber, f.clone()), run(" z ", f.clone()), field(Field::PageCount, f)],
    };
    p.fmt.align = Some(Align::Center);
    Story { paras: vec![p] }
}

/// Nagłówek z tytułem bieżącego rozdziału (żywa pagina) po prawej.
fn header_running(label: &str, color: Color) -> Story {
    let f = CharFmt { size: Some(8.5), color: Some(color), caps: Some(Caps::Small), tracking: Some(0.6), ..Default::default() };
    let mut p = Paragraph { style: "header".into(), fmt: ParaFmt::default(), runs: vec![run(label, f.clone()), run("  ·  ", f.clone()), field(Field::Running(1), f)] };
    p.fmt.align = Some(Align::Right);
    Story { paras: vec![p] }
}

fn shape_obj(id: u64, page: usize, r: (f32, f32, f32, f32), shape: Shape, kind: ObjKind, wrap: WrapMode, fill: Option<Fill>) -> Object {
    Object {
        id,
        name: String::new(),
        kind,
        page,
        x: r.0,
        y: r.1,
        w: r.2,
        h: r.3,
        shape,
        wrap: Wrap { mode: wrap, dist: 10.0, contour: true },
        fill,
        stroke: None,
        opacity: 1.0,
        corner: 0.0,
        shadow: false,
        locked: false,
        anchor_dy: 4.0,
        cont_of: None,
        repeat: Default::default(),
    }
}

fn frame(story: Story, padding: f32, valign: VAlign) -> ObjKind {
    ObjKind::Text { story, padding, columns: 1, gap: 12.0, valign, next: None }
}

/// Dodaje obiekt i zwraca jego id (do dalszych zmian przez `object_mut`).
fn add(d: &mut Document, page: usize, r: (f32, f32, f32, f32), shape: Shape, kind: ObjKind, wrap: WrapMode, fill: Option<Fill>) -> u64 {
    let id = d.new_id();
    d.objects.push(shape_obj(id, page, r, shape, kind, wrap, fill));
    id
}

/// Ramka tekstowa z akapitami.
fn text_kind(paras: Vec<Paragraph>, padding: f32, valign: VAlign) -> ObjKind {
    frame(Story { paras }, padding, valign)
}

fn preset(id: &str) -> Shape {
    geom::preset(id).unwrap_or_else(Shape::rect)
}

/// Grafika wektorowa (SVG) jako zasób dokumentu.
fn svg_asset(d: &mut Document, svg: &str) -> Option<String> {
    let a = crate::io::odtp::asset_from(svg.as_bytes().to_vec())?;
    let key = format!("img{}", d.new_id());
    d.assets.insert(key.clone(), a);
    Some(key)
}

/// Kotwica obiektu na końcu treści: obiekt staje pod poprzednim akapitem i jedzie z tekstem.
fn anchor_para(ids: &[u64]) -> Paragraph {
    let mut p = Paragraph::default();
    p.runs = ids.iter().map(|&id| Run { text: FIELD_CHAR.to_string(), field: Some(Field::Anchor(id)), ..Default::default() }).collect();
    p.fmt.space_after = Some(0.0);
    p
}

fn bullet(kind: ListKind, level: u8, md: &str) -> Paragraph {
    let mut p = rich("list", md);
    p.fmt.list = Some(Some(ListFmt { kind, level, start: None }));
    p
}

fn set_style(d: &mut Document, id: &str, f: impl Fn(&mut Style)) {
    if let Some(s) = d.styles.iter_mut().find(|s| s.id == id) {
        f(s);
    }
}

fn gradient(from: Color, to: Color, angle: f32) -> Option<Fill> {
    Some(Fill::Linear { from, to, angle })
}

fn solid(c: Color) -> Option<Fill> {
    Some(Fill::Solid { color: c })
}

// ------------------------------------------------------------------ przewodnik

const INK: Color = Color::rgb(0x16, 0x1a, 0x2b);
const VIOLET: Color = Color::rgb(0x5b, 0x3d, 0xf5);
const TEAL: Color = Color::rgb(0x00, 0xa8, 0x9d);
const AMBER: Color = Color::rgb(0xff, 0xb4, 0x3b);
const ROSE: Color = Color::rgb(0xe8, 0x3e, 0x7a);
const MIST: Color = Color::rgb(0x6b, 0x72, 0x88);

const ILLUSTRATION: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200"><defs><linearGradient id="a" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#5b3df5"/><stop offset="1" stop-color="#00a89d"/></linearGradient><radialGradient id="b" cx="0.35" cy="0.3" r="0.8"><stop offset="0" stop-color="#ffffff" stop-opacity="0.9"/><stop offset="1" stop-color="#ffffff" stop-opacity="0"/></radialGradient></defs><circle cx="100" cy="100" r="92" fill="url(#a)"/><circle cx="100" cy="100" r="92" fill="url(#b)"/><path d="M40 128 Q70 70 100 112 T160 92" fill="none" stroke="#ffffff" stroke-width="7" stroke-linecap="round"/><path d="M40 150 Q70 100 100 136 T160 118" fill="none" stroke="#ffb43b" stroke-width="5" stroke-linecap="round"/><circle cx="62" cy="62" r="10" fill="#ffb43b"/><rect x="118" y="44" width="40" height="26" rx="6" fill="#ffffff" fill-opacity="0.85"/><rect x="124" y="50" width="28" height="4" rx="2" fill="#5b3df5"/><rect x="124" y="58" width="18" height="4" rx="2" fill="#00a89d"/></svg>"##;

pub fn welcome() -> Document {
    let mut d = Document::new();
    d.meta.title = "Witaj w OpenDTP".into();
    d.meta.author = "OpenDTP".into();
    d.meta.subject = "Przewodnik po możliwościach programu".into();
    d.settings.hyphenate = true;
    let (w, h) = (d.page.width, d.page.height);
    d.page.margin_left = 64.0;
    d.page.margin_right = 64.0;
    d.page.margin_top = 78.0;
    d.page.margin_bottom = 72.0;
    let cw = w - 128.0;
    set_style(&mut d, "normal", |s| {
        s.chars.font = Some("Georgia".into());
        s.chars.size = Some(10.5);
        s.chars.color = Some(INK);
        s.para.line = Some(LineSpacing::Multiple(1.22));
        s.para.space_after = Some(7.0);
        s.para.align = Some(Align::Justify);
    });
    set_style(&mut d, "heading1", |s| {
        s.chars.font = Some("Segoe UI Light".into());
        s.chars.size = Some(30.0);
        s.chars.color = Some(VIOLET);
        s.chars.bold = Some(false);
        s.para.space_before = Some(0.0);
        s.para.space_after = Some(14.0);
        s.para.align = Some(Align::Left);
    });
    set_style(&mut d, "heading2", |s| {
        s.chars.font = Some("Segoe UI Semibold".into());
        s.chars.size = Some(13.0);
        s.chars.color = Some(INK);
        s.para.space_before = Some(14.0);
        s.para.space_after = Some(5.0);
        s.para.align = Some(Align::Left);
    });
    set_style(&mut d, "quote", |s| {
        s.chars.font = Some("Georgia".into());
        s.chars.italic = Some(true);
        s.chars.size = Some(13.0);
        s.chars.color = Some(VIOLET);
        s.para.align = Some(Align::Center);
        s.para.space_before = Some(10.0);
        s.para.space_after = Some(12.0);
    });
    set_style(&mut d, "callout", |s| {
        s.chars.font = Some("Segoe UI".into());
        s.chars.size = Some(9.5);
        s.chars.color = Some(INK);
        s.para.shading = Some(Color::rgb(0xef, 0xec, 0xff));
        s.para.border = Some(Some(BoxBorder { width: 0.0, color: Color::NONE, radius: 8.0, padding: 9.0 }));
        s.para.space_before = Some(10.0);
        s.para.space_after = Some(12.0);
        s.para.align = Some(Align::Left);
    });
    set_style(&mut d, "list", |s| {
        s.para.space_after = Some(3.0);
        s.para.align = Some(Align::Left);
    });
    set_style(&mut d, "caption", |s| {
        s.chars.font = Some("Segoe UI".into());
        s.chars.size = Some(8.5);
        s.chars.color = Some(MIST);
        s.chars.italic = Some(false);
        s.para.align = Some(Align::Center);
        s.para.space_before = Some(4.0);
        s.para.space_after = Some(12.0);
    });
    set_style(&mut d, "code", |s| {
        s.chars.size = Some(9.0);
        s.para.shading = Some(Color::rgb(0x16, 0x1a, 0x2b));
        s.chars.color = Some(Color::rgb(0xd8, 0xe1, 0xff));
        s.para.border = Some(Some(BoxBorder { width: 0.0, color: Color::NONE, radius: 6.0, padding: 8.0 }));
        s.para.align = Some(Align::Left);
    });

    // ---------------------------------------------------------- strona 1: okładka
    add(&mut d, 0, (0.0, 0.0, w, h), Shape::rect(), ObjKind::Shape, WrapMode::Behind, gradient(Color::rgb(0x0c, 0x0f, 0x2e), Color::rgb(0x3a, 0x1a, 0x78), 125.0));
    add(&mut d, 0, (w - 300.0, -110.0, 440.0, 440.0), geom::ellipse(), ObjKind::Shape, WrapMode::Behind, Some(Fill::Radial { from: VIOLET.with_alpha(210), to: VIOLET.with_alpha(0) }));
    add(&mut d, 0, (-170.0, h - 420.0, 480.0, 480.0), geom::ellipse(), ObjKind::Shape, WrapMode::Behind, Some(Fill::Radial { from: TEAL.with_alpha(170), to: TEAL.with_alpha(0) }));
    add(&mut d, 0, (w - 190.0, 118.0, 104.0, 104.0), preset("ring"), ObjKind::Shape, WrapMode::Behind, solid(AMBER.with_alpha(230)));
    add(&mut d, 0, (0.0, h - 120.0, w, 120.0), geom::wave(), ObjKind::Shape, WrapMode::Behind, solid(Color::WHITE.with_alpha(22)));
    add(&mut d, 0, (0.0, h - 90.0, w, 90.0), geom::wave(), ObjKind::Shape, WrapMode::Behind, solid(Color::WHITE.with_alpha(30)));

    let mut kicker = line("PRZEWODNIK  ·  POKAZ MOŻLIWOŚCI", 9.5, AMBER, true, Align::Left);
    kicker.runs[0].fmt.tracking = Some(3.5);
    let mut title = line("OpenDTP", 76.0, Color::WHITE, true, Align::Left);
    title.runs[0].fmt.font = Some("Segoe UI Black".into());
    title.runs[0].fmt.effect = Some(TextEffect::Glow);
    title.runs[0].fmt.tracking = Some(-1.5);
    let mut sub = line("Edytor tekstu i skład dokumentów,\nw którym tekst płynie w dowolnym kształcie.", 17.0, Color::rgb(0xdc, 0xd8, 0xff), false, Align::Left);
    sub.runs[0].fmt.font = Some("Segoe UI Light".into());
    sub.fmt.line = Some(LineSpacing::Multiple(1.15));
    add(&mut d, 0, (56.0, 250.0, w - 112.0, 210.0), Shape::rect(), text_kind(vec![kicker, title, sub], 0.0, VAlign::Top), WrapMode::None, None);

    let mut arc = line("TEKST  ·  KSZTAŁT  ·  KOLOR  ·  DRUK", 12.0, Color::WHITE, true, Align::Center);
    arc.runs[0].fmt.tracking = Some(4.0);
    add(&mut d, 0, (w / 2.0 - 200.0, 120.0, 400.0, 90.0), geom::arc_path(), ObjKind::PathText { story: Story { paras: vec![arc] }, offset: 0.0 }, WrapMode::None, None);

    let mut star_t = line("100%\nwektorowo", 15.0, INK, true, Align::Center);
    star_t.fmt.line = Some(LineSpacing::Multiple(0.95));
    let star = add(&mut d, 0, (w - 240.0, h - 360.0, 170.0, 170.0), geom::star(12, 0.78), text_kind(vec![star_t], 30.0, VAlign::Center), WrapMode::None, Some(Fill::Radial { from: Color::rgb(0xff, 0xe0, 0x8a), to: Color::rgb(0xff, 0x7a, 0x3b) }));
    if let Some(o) = d.object_mut(star) {
        o.shadow = true;
    }
    for (k, t) in ["Tekst w kształtach", "PDF dla drukarni", "DOCX · EPUB · HTML"].iter().enumerate() {
        let id = add(&mut d, 0, (56.0 + k as f32 * 160.0, h - 196.0, 150.0, 34.0), Shape::rect(), text_kind(vec![line(t, 10.0, Color::WHITE, true, Align::Center)], 4.0, VAlign::Center), WrapMode::None, solid(Color::WHITE.with_alpha(26)));
        if let Some(o) = d.object_mut(id) {
            o.corner = 17.0;
            o.stroke = Some(StrokeStyle { color: Color::WHITE.with_alpha(110), width: 0.8, dash: vec![] });
        }
    }

    let mut paras = vec![Paragraph::default()];

    // ---------------------------------------------------------- strona 2: tekst w kształtach
    let mut h1 = para("heading1", "Tekst, który płynie");
    h1.fmt.break_before = Some(Break::Page);
    paras.push(h1);
    let mut lead = para("normal", "Klasyczny edytor zna tylko prostokąt strony. OpenDTP traktuje stronę jak płótno: ramka tekstowa może być kołem, falą, sercem albo dowolnym konturem narysowanym z węzłów i krzywych Béziera. Tekst wypełnia jej wnętrze wiersz po wierszu, a każdy wiersz ma dokładnie taką szerokość, na jaką pozwala kształt.");
    lead.fmt.drop_cap = Some(3);
    paras.push(lead);
    paras.push(para("normal", "Kształt po prawej to zwykła ramka z tekstem. Reszta strony go opływa – nie po prostokącie, tylko po samym konturze, z odstępem, który ustawiasz na karcie Kształty tekstu. Przeciągnij węzeł, a tekst w środku i wokół od razu ułoży się na nowo. Dwuklik na kształcie pokazuje węzły; dwuklik na krawędzi dodaje nowy, a każdy odcinek można zmienić w łuk."));
    paras.push(para("normal", "Kształty da się też łączyć: suma, różnica, część wspólna i wykluczenie działają jak w programach graficznych. Pierścień z otworem? Tekst przeskoczy na drugą stronę otworu i będzie płynął dalej. Ramki można połączyć w łańcuch – to, co nie mieści się w pierwszej, płynie do następnej, także na innej stronie."));
    paras.push(rich("heading2", "Opływanie na sześć sposobów"));
    for t in ["**Z obu stron** – tekst otacza obiekt i wypełnia wolne miejsce po obu stronach.", "**Po lewej** albo **po prawej** – jak w czasopismach, z wolną drugą stroną.", "**Po większej stronie** – program sam wybiera, gdzie jest więcej miejsca.", "**Nad i pod** – obiekt przerywa tekst na całej szerokości.", "**Za tekstem** i **przed tekstem** – tła, znaki wodne i elementy dekoracyjne."] {
        paras.push(bullet(ListKind::Bullet, 0, t));
    }
    paras.push(rich("normal", "Opływanie działa także dla obrazów: przy grafice z przezroczystym tłem program sam **wykrywa kontur** i tekst przylega do kształtu ilustracji, a nie do jej prostokąta."));
    paras.push(rich("callout", "**Spróbuj:** zaznacz akapit i wybierz Kształty tekstu → Tekst w kształt. Zwykły tekst stanie się kształtem w tym samym miejscu strony – a reszta dokumentu go opłynie."));

    let mut heart = line("Ten tekst wypełnia serce. Każdy wiersz ma inną szerokość, dopasowaną do konturu – a cały kształt opływa treść strony.", 10.0, Color::rgb(0x7a, 0x10, 0x35), false, Align::Center);
    heart.runs[0].fmt.font = Some("Georgia".into());
    heart.runs[0].fmt.italic = Some(true);
    let hid = add(&mut d, 1, (w - 64.0 - 205.0, 238.0, 205.0, 190.0), preset("heart"), text_kind(vec![heart], 22.0, VAlign::Center), WrapMode::Both, Some(Fill::Radial { from: Color::rgb(0xff, 0xe6, 0xef), to: Color::rgb(0xff, 0x9c, 0xbd) }));
    if let Some(o) = d.object_mut(hid) {
        o.wrap.dist = 12.0;
        o.shadow = true;
    }
    if let Some(key) = svg_asset(&mut d, ILLUSTRATION) {
        let id = add(&mut d, 1, (64.0, 520.0, 130.0, 130.0), geom::ellipse(), ObjKind::Image { asset: key, fit: ImageFit::Contain, contour: None }, WrapMode::Both, None);
        if let Some(o) = d.object_mut(id) {
            o.wrap.dist = 10.0;
        }
    }

    // ---------------------------------------------------------- strona 3: łamanie w kolumnach
    let mut h1 = para("heading1", "Łamanie jak w gazecie");
    h1.fmt.section = Some(Section { columns: 1, column_gap: 14.0, column_rule: false, landscape: false, continuous: false });
    paras.push(h1);
    let mut lead = rich("normal", "*Sekcje zmieniają liczbę kolumn w środku strony. Ten wstęp ma jedną kolumnę, a artykuły poniżej – trzy, z cienką linią między nimi. Inicjały, wyjustowanie i dzielenie wyrazów robią resztę.*");
    lead.fmt.space_after = Some(14.0);
    paras.push(lead);
    let articles = [
        ("Sekcje i kolumny", "Każda sekcja ma własną liczbę kolumn, odstęp, linię między kolumnami, a nawet orientację strony. Podział ciągły zaczyna nową sekcję w tym samym miejscu strony – dlatego nagłówek strony biegnie przez całą szerokość, a tekst pod nim rozkłada się na trzy łamy. Tekst płynie z kolumny do kolumny i ze strony na stronę, omijając obrazy i kształty."),
        ("Inicjały i justowanie", "Duża pierwsza litera na trzy wiersze to jeden przycisk. Wyjustowanie z polskim dzieleniem wyrazów daje równe łamy bez dziur, a kontrola sierotek pilnuje, żeby jednoliterowe spójniki nie zostawały na końcu wiersza. Wdowy i bękarty też nie mają szans – program przenosi wiersze tak, by akapit nie urywał się w złym miejscu."),
        ("Cytat w łamach", "Wyróżniony cytat to zwykła ramka tekstowa z tłem i zaokrąglonymi rogami. Ustawiona na granicy łamów rozpycha tekst obu kolumn – opływanie działa w każdej z nich osobno. Takie elementy przyciągają wzrok i porządkują długi artykuł, a zmiana treści cytatu nie psuje układu strony."),
        ("Siatka bazowa", "Włączona siatka bazowa ustawia wiersze wszystkich kolumn na tych samych wysokościach. Tekst w sąsiednich łamach zaczyna się i kończy w jednej linii – tak jak w drukowanych gazetach i książkach. Nagłówki i odstępy dopasowują się do siatki automatycznie."),
    ];
    for (k, (hd, body)) in articles.iter().enumerate() {
        let mut h2 = para("heading2", hd);
        if k == 0 {
            h2.fmt.section = Some(Section { columns: 3, column_gap: 16.0, column_rule: true, landscape: false, continuous: true });
            h2.fmt.space_before = Some(0.0);
        }
        paras.push(h2);
        let mut p = para("normal", body);
        p.fmt.drop_cap = Some(2);
        paras.push(p);
        paras.push(para("normal", "W OpenDTP te same narzędzia służą do listu, raportu i gazetki: zmienia się tylko układ, a tekst zostaje edytowalny od pierwszej do ostatniej strony."));
    }
    let mut q = line("„Kolumny, inicjały i cytaty – bez jednego tabulatora.”", 14.0, Color::WHITE, false, Align::Center);
    q.runs[0].fmt.font = Some("Georgia".into());
    q.runs[0].fmt.italic = Some(true);
    let qid = add(&mut d, 2, (w / 2.0 - 112.0, 470.0, 224.0, 104.0), Shape::rect(), text_kind(vec![q], 14.0, VAlign::Center), WrapMode::Both, gradient(VIOLET, Color::rgb(0x8b, 0x3d, 0xf5), 0.0));
    if let Some(o) = d.object_mut(qid) {
        o.corner = 14.0;
        o.wrap.contour = false;
        o.wrap.dist = 9.0;
        o.shadow = true;
    }

    // ---------------------------------------------------------- strona 4: typografia
    let mut h1 = para("heading1", "Typografia w szczegółach");
    h1.fmt.section = Some(Section { columns: 1, column_gap: 14.0, column_rule: false, landscape: false, continuous: false });
    paras.push(h1);
    paras.push(para("normal", "Każdy znak może mieć własny krój, rozmiar, kolor i efekt. Wszystko poniżej to zwykły tekst – zaznacz dowolny fragment, a wstążka pokaże jego formatowanie."));
    let b = |t: &str| run(t, CharFmt { bold: Some(true), ..Default::default() });
    let big = |t: &str, f: CharFmt| run(t, CharFmt { size: Some(15.0), bold: Some(true), font: Some("Segoe UI".into()), ..f });
    let mut demo = runs(
        "normal",
        vec![
            b("Pogrubienie"),
            plain(", "),
            run("kursywa", CharFmt { italic: Some(true), ..Default::default() }),
            plain(", "),
            run("podkreślenie podwójne", CharFmt { underline: Some(Underline::Double), ..Default::default() }),
            plain(", "),
            run("falowane", CharFmt { underline: Some(Underline::Wave), color: Some(ROSE), ..Default::default() }),
            plain(", "),
            run("przekreślenie", CharFmt { strike: Some(true), ..Default::default() }),
            plain(", "),
            run("Kapitaliki", CharFmt { caps: Some(Caps::Small), ..Default::default() }),
            plain(", "),
            run("wersaliki", CharFmt { caps: Some(Caps::All), tracking: Some(1.0), ..Default::default() }),
            plain(", E = mc"),
            run("2", CharFmt { script: Some(Script::Super), ..Default::default() }),
            plain(", H"),
            run("2", CharFmt { script: Some(Script::Sub), ..Default::default() }),
            plain("O, "),
            run("wyróżnienie", CharFmt { highlight: Some(Color::rgb(0xff, 0xe8, 0x7a)), ..Default::default() }),
            plain(" i "),
            run("rozstrzelenie", CharFmt { tracking: Some(3.0), color: Some(TEAL), ..Default::default() }),
            plain("."),
        ],
    );
    demo.fmt.line = Some(LineSpacing::Multiple(1.5));
    paras.push(demo);
    let mut fx = runs(
        "normal",
        vec![
            plain("Efekty tekstu: "),
            big("kontur", CharFmt { outline: Some(0.7), color: Some(VIOLET), ..Default::default() }),
            plain(", "),
            big("cień", CharFmt { effect: Some(TextEffect::Shadow), color: Some(INK), ..Default::default() }),
            plain(" i "),
            big("poświata", CharFmt { effect: Some(TextEffect::Glow), color: Some(TEAL), ..Default::default() }),
            plain(" – widoczne na ekranie i w PDF."),
        ],
    );
    fx.fmt.line = Some(LineSpacing::Multiple(1.5));
    paras.push(fx);
    paras.push(runs(
        "normal",
        vec![
            plain("Funkcje OpenType: cyfry nautyczne "),
            run("1234567890", CharFmt { features: Some("onum".into()), font: Some("Georgia".into()), ..Default::default() }),
            plain(", tabelaryczne "),
            run("1234567890", CharFmt { features: Some("tnum,lnum".into()), font: Some("Calibri".into()), ..Default::default() }),
            plain(", ligatury "),
            run("fi fl ff", CharFmt { font: Some("Calibri".into()), ..Default::default() }),
            plain(" i kerning par liter."),
        ],
    ));
    paras.push(rich("heading2", "Listy wielopoziomowe"));
    paras.push(bullet(ListKind::Multi, 0, "Przygotowanie dokumentu"));
    paras.push(bullet(ListKind::Multi, 1, "Wybór szablonu i stylów"));
    paras.push(bullet(ListKind::Multi, 1, "Ustawienie strony, marginesów i kolumn"));
    paras.push(bullet(ListKind::Multi, 2, "Spady i znaczniki cięcia do druku"));
    paras.push(bullet(ListKind::Multi, 0, "Skład i korekta"));
    paras.push(bullet(ListKind::Multi, 1, "Sprawdzanie pisowni podczas pisania"));
    paras.push(rich("heading2", "Do odhaczenia"));
    for t in ["Tekst w kształtach i na ścieżce", "Spis treści, indeks i przypisy", "Eksport do PDF, DOCX, EPUB i HTML"] {
        paras.push(bullet(ListKind::Check, 0, t));
    }
    paras.push(para("quote", "„Dobry układ strony jest niewidoczny – widać tylko treść.”"));
    paras.push(para("code", "Ctrl+K  →  paleta poleceń: wpisz „kształt”, „kolumny” albo „PDF”."));

    // ---------------------------------------------------------- strona 5: dane
    let mut h1 = para("heading1", "Dane w dokumencie");
    h1.fmt.break_before = Some(Break::Page);
    paras.push(h1);
    let table = Table {
        rows: vec![
            vec!["Format".into(), "Do czego".into(), "Kształty".into(), "Kolory".into()],
            vec!["**PDF**".into(), "Udostępnianie i archiwum".into(), "wektorowo".into(), "RGB".into()],
            vec!["**PDF do druku**".into(), "Drukarnia, offset".into(), "wektorowo".into(), "CMYK, Pantone".into()],
            vec!["**DOCX**".into(), "Współpraca z Wordem".into(), "kształty Worda".into(), "RGB".into()],
            vec!["**EPUB 3**".into(), "Czytniki e-booków".into(), "obrazy".into(), "RGB".into()],
            vec!["**HTML · Markdown**".into(), "Strony WWW, notatki".into(), "–".into(), "RGB".into()],
        ],
        header: true,
        stripes: true,
        font: "Segoe UI".into(),
        size: 9.0,
        accent: VIOLET,
        line: Color::rgb(0xc9, 0xc4, 0xf2),
        numbers_right: false,
        widths: Vec::new(),
        take: None,
    };
    let th = crate::paint::table_height(&table, cw);
    let tid = add(&mut d, 4, (64.0, 140.0, cw, th), Shape::rect(), ObjKind::Table { table }, WrapMode::TopBottom, solid(Color::WHITE));
    let table_seq = d.new_id();
    let fig_seq = d.new_id();
    paras.push(runs(
        "normal",
        vec![
            plain("Tabele mają nagłówek, pasy, kolor akcentu i formatowanie w komórkach. Gdy nie mieszczą się na stronie, dzielą się i powtarzają wiersz nagłówka. Podpisy numerują się same, a odsyłacz w tekście – „zob. rysunek "),
            field(Field::Ref(fig_seq, RefKind::Number), CharFmt::default()),
            plain("” – aktualizuje się po każdej zmianie."),
        ],
    ));
    paras.push(anchor_para(&[tid]));
    paras.push(runs("caption", vec![plain("Tabela "), field(Field::Seq(table_seq, SeqKind::Table), CharFmt::default()), plain(" – Formaty eksportu")]));
    paras.push(para("normal", "Wykresy są wektorowe i edytowalne: dane wpisujesz w panelu właściwości, a kolumny, słupki, linie, obszary i koła rysują się od nowa. W PDF pozostają ostre przy każdym powiększeniu."));
    let col = Chart {
        kind: ChartKind::Column,
        title: "Strony złożone w kwartale".into(),
        labels: vec!["I kw.".into(), "II kw.".into(), "III kw.".into(), "IV kw.".into()],
        series: vec![
            Series { name: "2025".into(), values: vec![120.0, 180.0, 150.0, 220.0], color: Some(Color::rgb(0xb9, 0xae, 0xff)) },
            Series { name: "2026".into(), values: vec![160.0, 230.0, 210.0, 290.0], color: Some(VIOLET) },
        ],
        ..Chart::default()
    };
    let pie = Chart {
        kind: ChartKind::Doughnut,
        title: "Eksport według formatu".into(),
        labels: vec!["PDF".into(), "DOCX".into(), "EPUB".into(), "HTML".into()],
        series: vec![Series { name: "Udział".into(), values: vec![48.0, 27.0, 15.0, 10.0], color: None }],
        ..Chart::default()
    };
    let half = (cw - 16.0) / 2.0;
    let c1 = add(&mut d, 4, (64.0, 420.0, half, 190.0), Shape::rect(), ObjKind::Chart { chart: col }, WrapMode::TopBottom, solid(Color::rgb(0xfa, 0xf9, 0xff)));
    let c2 = add(&mut d, 4, (64.0 + half + 16.0, 420.0, half, 190.0), Shape::rect(), ObjKind::Chart { chart: pie }, WrapMode::TopBottom, solid(Color::rgb(0xfa, 0xf9, 0xff)));
    for id in [c1, c2] {
        if let Some(o) = d.object_mut(id) {
            o.corner = 10.0;
            o.stroke = Some(StrokeStyle { color: Color::rgb(0xe2, 0xde, 0xfb), width: 0.8, dash: vec![] });
            o.anchor_dy = 8.0;
        }
    }
    if let Some(o) = d.object_mut(tid) {
        o.anchor_dy = 8.0;
    }
    paras.push(anchor_para(&[c1, c2]));
    paras.push(runs("caption", vec![plain("Rysunek "), field(Field::Seq(fig_seq, SeqKind::Figure), CharFmt::default()), plain(" – Wykres kolumnowy i pierścieniowy (dane przykładowe)")]));
    paras.push(rich("normal", "Spis treści, indeks haseł, przypisy i odsyłacze zbierają się z dokumentu automatycznie – karta **Odwołania → Aktualizuj wszystko** odświeża je naraz."));

    // ---------------------------------------------------------- strona 6: druk
    let mut h1 = para("heading1", "Od pomysłu do druku");
    h1.fmt.break_before = Some(Break::Page);
    paras.push(h1);
    let n1 = d.new_id();
    let n2 = d.new_id();
    let sup = CharFmt { script: Some(Script::Super), ..Default::default() };
    paras.push(runs(
        "normal",
        vec![
            plain("Gotowy dokument zapiszesz jako PDF dla drukarni: kolory w CMYK, kolory dodatkowe z próbek, spady"),
            field(Field::Footnote(n1), sup.clone()),
            plain(" i znaczniki cięcia. Kontrola przed drukiem – preflight – wskaże tekst, który się nie mieści, brakujące pliki i czcionki oraz obrazy o zbyt niskiej rozdzielczości"),
            field(Field::Footnote(n2), sup),
            plain("."),
        ],
    ));
    let note = |t: &str| {
        let mut p = para("normal", t);
        p.runs[0].fmt.size = Some(8.5);
        p.fmt.space_after = Some(0.0);
        p.fmt.align = Some(Align::Left);
        p
    };
    d.notes.push(Note { id: n1, end: false, story: Story { paras: vec![note("Spad to pas tła i grafiki wychodzący poza format kartki – po przycięciu nie zostaje biały margines. Zwykle 3 mm.")] } });
    d.notes.push(Note { id: n2, end: false, story: Story { paras: vec![note("Do druku zaleca się co najmniej 300 punktów na cal w rzeczywistym rozmiarze obrazu na stronie.")] } });
    paras.push(para("normal", "Cały proces mieści się w trzech krokach – i w jednym programie:"));
    let steps = [("Pomysł", Color::rgb(0x8b, 0x7b, 0xff)), ("Skład", VIOLET), ("Druk", Color::rgb(0x3a, 0x1a, 0x9a))];
    let mut step_ids = Vec::new();
    let sw = (cw - 20.0) / 3.0;
    for (k, (t, c)) in steps.iter().enumerate() {
        let id = add(&mut d, 5, (64.0 + k as f32 * (sw + 10.0), 300.0, sw, 70.0), preset("chevron"), text_kind(vec![line(t, 15.0, Color::WHITE, true, Align::Center)], 6.0, VAlign::Center), WrapMode::TopBottom, solid(*c));
        if let Some(o) = d.object_mut(id) {
            o.anchor_dy = 10.0;
        }
        step_ids.push(id);
    }
    paras.push(anchor_para(&step_ids));
    let mut sp = para("normal", "");
    sp.fmt.space_before = Some(14.0);
    paras.push(sp);
    paras.push(rich("normal", "Do Worda i z Worda: import DOCX zachowuje style, listy, tabele, kształty i pola tekstowe, a nawet wzory zapisane jako metapliki WMF – OpenDTP zamienia je na grafikę wektorową. Eksport odtwarza kształty tekstu jako kształty Worda."));
    let mut end1 = line("Zacznij od pustej strony", 20.0, Color::WHITE, true, Align::Center);
    end1.runs[0].fmt.font = Some("Segoe UI Semibold".into());
    let end2 = line("albo wybierz szablon w menu Plik → Nowy: raport, list, gazetka, ulotka, CV, zaproszenie, dyplom.", 10.5, Color::rgb(0xe8, 0xe4, 0xff), false, Align::Center);
    let banner = add(&mut d, 5, (64.0, 520.0, cw, 96.0), Shape::rect(), text_kind(vec![end1, end2], 16.0, VAlign::Center), WrapMode::TopBottom, gradient(VIOLET, TEAL, 0.0));
    if let Some(o) = d.object_mut(banner) {
        o.corner = 16.0;
        o.shadow = true;
        o.anchor_dy = 16.0;
    }
    paras.push(anchor_para(&[banner]));

    d.body = Story { paras };
    d.header = Some(header_running("OpenDTP", MIST));
    d.footer = Some(footer_of(MIST));
    d.page.different_first = true;
    d.header_first = Some(Story::default());
    d.footer_first = Some(Story::default());
    d
}

// ------------------------------------------------------------------ szablony

/// Dokument z szablonu (karta „Nowy” w menu Plik).
pub fn template(id: &str) -> Document {
    match id {
        "welcome" => welcome(),
        "textbook" => textbook(),
        "report" => report(),
        "letter" => letter(),
        "newsletter" => newsletter(),
        "flyer" => flyer(),
        "cv" => cv(),
        "invitation" => invitation(),
        "certificate" => certificate(),
        "menu" => menu(),
        "invoice" => invoice(),
        "poster" => poster(),
        "card" => card(),
        "brochure" => brochure(),
        "book" => book(),
        _ => Document::new(),
    }
}

/// Raport: okładka z pasem gradientu, spis treści, rozdziały z tabelą, wykresem i wnioskami.
fn report() -> Document {
    let mut d = Document::new();
    d.meta.title = "Raport roczny 2026".into();
    d.meta.subject = "Podsumowanie roku".into();
    d.settings.hyphenate = true;
    let navy = Color::rgb(0x0f, 0x2a, 0x4a);
    let blue = Color::rgb(0x1f, 0x6f, 0xd6);
    let sky = Color::rgb(0x4c, 0xc3, 0xff);
    let (w, h) = (d.page.width, d.page.height);
    let cw = d.page.content_width();
    let ml = d.page.margin_left;
    set_style(&mut d, "normal", |s| {
        s.chars.font = Some("Calibri".into());
        s.chars.size = Some(10.5);
        s.para.line = Some(LineSpacing::Multiple(1.2));
        s.para.space_after = Some(6.0);
        s.para.align = Some(Align::Justify);
    });
    set_style(&mut d, "heading1", |s| {
        s.chars.font = Some("Segoe UI Semibold".into());
        s.chars.size = Some(20.0);
        s.chars.color = Some(navy);
        s.para.space_before = Some(18.0);
        s.para.space_after = Some(8.0);
    });
    set_style(&mut d, "heading2", |s| {
        s.chars.font = Some("Segoe UI Semibold".into());
        s.chars.size = Some(12.5);
        s.chars.color = Some(blue);
    });
    set_style(&mut d, "callout", |s| {
        s.para.shading = Some(Color::rgb(0xe8, 0xf2, 0xff));
        s.para.border = Some(Some(BoxBorder { width: 0.0, color: Color::NONE, radius: 6.0, padding: 10.0 }));
        s.chars.color = Some(navy);
    });

    // Okładka.
    add(&mut d, 0, (0.0, 0.0, w, 360.0), Shape::rect(), ObjKind::Shape, WrapMode::Behind, gradient(navy, blue, 20.0));
    add(&mut d, 0, (w - 260.0, 150.0, 330.0, 330.0), geom::ellipse(), ObjKind::Shape, WrapMode::Behind, Some(Fill::Radial { from: sky.with_alpha(150), to: sky.with_alpha(0) }));
    add(&mut d, 0, (0.0, 300.0, w, 90.0), geom::wave(), ObjKind::Shape, WrapMode::Behind, solid(Color::WHITE));
    let mut k = line("RAPORT ROCZNY", 10.0, sky, true, Align::Left);
    k.runs[0].fmt.tracking = Some(4.0);
    let mut t = line("Rok, w którym\nurośliśmy razem", 34.0, Color::WHITE, true, Align::Left);
    t.runs[0].fmt.font = Some("Segoe UI Semibold".into());
    t.fmt.line = Some(LineSpacing::Multiple(1.0));
    let s = line("Podsumowanie wyników, projektów i planów na kolejny rok", 13.0, Color::rgb(0xd6, 0xe8, 0xff), false, Align::Left);
    add(&mut d, 0, (ml, 110.0, cw, 170.0), Shape::rect(), text_kind(vec![k, t, s], 0.0, VAlign::Top), WrapMode::None, None);
    let mut kpi = Vec::new();
    for (k2, (v, l)) in [("+38%", "przychodów"), ("124", "projekty"), ("4,9", "ocena klientów")].iter().enumerate() {
        let mut big = line(v, 26.0, blue, true, Align::Center);
        big.runs[0].fmt.font = Some("Segoe UI Semibold".into());
        let small = line(l, 9.5, Color::rgb(0x55, 0x60, 0x70), false, Align::Center);
        let kw = (cw - 24.0) / 3.0;
        let id = add(&mut d, 0, (ml + k2 as f32 * (kw + 12.0), 440.0, kw, 84.0), Shape::rect(), text_kind(vec![big, small], 8.0, VAlign::Center), WrapMode::None, solid(Color::rgb(0xf3, 0xf7, 0xfd)));
        if let Some(o) = d.object_mut(id) {
            o.corner = 10.0;
        }
        kpi.push(id);
    }
    let mut who = line("Przygotował: Zespół analiz  ·  Warszawa, grudzień 2026", 9.5, Color::rgb(0x70, 0x78, 0x88), false, Align::Left);
    who.fmt.space_before = Some(0.0);
    add(&mut d, 0, (ml, h - 110.0, cw, 30.0), Shape::rect(), text_kind(vec![who], 0.0, VAlign::Center), WrapMode::None, None);

    let mut paras = vec![Paragraph::default()];
    let mut toc = para("heading1", "Spis treści");
    toc.fmt.break_before = Some(Break::Page);
    paras.push(toc);
    for (t, n) in [("1. Najważniejsze liczby", "3"), ("2. Wyniki finansowe", "3"), ("3. Projekty i klienci", "4"), ("4. Plany na 2027", "4")] {
        let mut p = para("toc1", &format!("{t}\t{n}"));
        p.fmt.tabs = Some(vec![-1.0]);
        paras.push(p);
    }
    paras.push(rich("callout", "**Wskazówka:** prawdziwy spis treści wstawisz z karty Odwołania – zbierze nagłówki i numery stron, a przycisk „Aktualizuj wszystko” odświeży go po zmianach."));
    let mut h = para("heading1", "1. Najważniejsze liczby");
    h.fmt.break_before = Some(Break::Page);
    paras.push(h);
    let mut lead = para("normal", "Miniony rok był najlepszym w historii firmy. Zwiększyliśmy przychody, rozbudowaliśmy zespół i weszliśmy na dwa nowe rynki. W raporcie zebraliśmy najważniejsze dane, opis projektów i plany na kolejne miesiące.");
    lead.fmt.drop_cap = Some(2);
    paras.push(lead);
    paras.push(rich("heading2", "Wyniki w skrócie"));
    for t in ["przychody wyższe o **38%** rok do roku,", "**124** zrealizowane projekty, w tym 31 międzynarodowych,", "średnia ocena klientów **4,9** na 5,", "zespół powiększony z 42 do **57** osób."] {
        paras.push(bullet(ListKind::Bullet, 0, t));
    }
    paras.push(para("heading1", "2. Wyniki finansowe"));
    paras.push(para("normal", "Przychody rosły w każdym kwartale, a największy skok przyniósł czwarty kwartał – po wdrożeniu dwóch dużych projektów dla klientów z sektora publicznego."));
    let chart = Chart {
        kind: ChartKind::Area,
        title: "Przychody (mln zł)".into(),
        labels: vec!["I kw.".into(), "II kw.".into(), "III kw.".into(), "IV kw.".into()],
        series: vec![
            Series { name: "2025".into(), values: vec![4.1, 4.6, 5.0, 5.4], color: Some(Color::rgb(0xa9, 0xc8, 0xf0)) },
            Series { name: "2026".into(), values: vec![5.3, 6.1, 6.9, 8.2], color: Some(blue) },
        ],
        ..Chart::default()
    };
    let cid = add(&mut d, 2, (ml, 400.0, cw, 200.0), Shape::rect(), ObjKind::Chart { chart }, WrapMode::TopBottom, solid(Color::rgb(0xf8, 0xfb, 0xff)));
    if let Some(o) = d.object_mut(cid) {
        o.corner = 8.0;
        o.anchor_dy = 8.0;
    }
    paras.push(anchor_para(&[cid]));
    let fig = d.new_id();
    paras.push(runs("caption", vec![plain("Rysunek "), field(Field::Seq(fig, SeqKind::Figure), CharFmt::default()), plain(" – Przychody w kwartałach")]));
    paras.push(para("heading1", "3. Projekty i klienci"));
    let table = Table {
        rows: vec![
            vec!["Obszar".into(), "Projekty".into(), "Zmiana".into()],
            vec!["Administracja publiczna".into(), "38".into(), "**+52%**".into()],
            vec!["Edukacja".into(), "29".into(), "**+21%**".into()],
            vec!["Wydawnictwa".into(), "33".into(), "**+17%**".into()],
            vec!["Firmy prywatne".into(), "24".into(), "**+9%**".into()],
        ],
        header: true,
        stripes: true,
        font: "Calibri".into(),
        size: 9.5,
        accent: navy,
        line: Color::rgb(0xc8, 0xd6, 0xea),
        numbers_right: true,
        widths: Vec::new(),
        take: None,
    };
    let th = crate::paint::table_height(&table, cw);
    let tid = add(&mut d, 3, (ml, 140.0, cw, th), Shape::rect(), ObjKind::Table { table }, WrapMode::TopBottom, solid(Color::WHITE));
    if let Some(o) = d.object_mut(tid) {
        o.anchor_dy = 8.0;
    }
    paras.push(para("normal", "Najszybciej rosła współpraca z administracją publiczną. Stabilnie rozwijały się projekty edukacyjne i wydawnicze."));
    paras.push(anchor_para(&[tid]));
    paras.push(para("heading1", "4. Plany na 2027"));
    paras.push(bullet(ListKind::Number, 0, "Otwarcie biura w Gdańsku i rekrutacja 15 osób."));
    paras.push(bullet(ListKind::Number, 0, "Nowa linia usług składu i przygotowania do druku."));
    paras.push(bullet(ListKind::Number, 0, "Program szkoleń dla klientów z sektora edukacji."));
    paras.push(rich("callout", "**Wniosek:** utrzymanie tempa wzrostu wymaga inwestycji w zespół i automatyzację – oba cele mają zapewnione finansowanie."));
    d.body = Story { paras };
    d.header = Some(header_running("Raport roczny 2026", Color::rgb(0x70, 0x78, 0x88)));
    d.footer = Some(footer_of(Color::rgb(0x70, 0x78, 0x88)));
    d.page.different_first = true;
    d.header_first = Some(Story::default());
    d.footer_first = Some(Story::default());
    let _ = kpi;
    d
}

/// List firmowy: papier z logo w kole, cienka linia akcentu, dane w stopce.
fn letter() -> Document {
    let mut d = Document::new();
    d.meta.title = "List".into();
    let green = Color::rgb(0x0b, 0x7a, 0x5c);
    let gray = Color::rgb(0x6a, 0x70, 0x78);
    let (w, h) = (d.page.width, d.page.height);
    let ml = d.page.margin_left;
    let cw = d.page.content_width();
    d.page.margin_top = 150.0;
    set_style(&mut d, "normal", |s| {
        s.chars.font = Some("Cambria".into());
        s.chars.size = Some(11.0);
        s.para.line = Some(LineSpacing::Multiple(1.25));
        s.para.space_after = Some(8.0);
    });
    let mut logo = line("ZN", 20.0, Color::WHITE, true, Align::Center);
    logo.runs[0].fmt.font = Some("Segoe UI Semibold".into());
    add(&mut d, 0, (ml, 48.0, 54.0, 54.0), geom::ellipse(), text_kind(vec![logo], 2.0, VAlign::Center), WrapMode::None, gradient(green, Color::rgb(0x2b, 0xb3, 0x86), 45.0));
    let mut name = line("Zielona Nuta", 18.0, green, true, Align::Left);
    name.runs[0].fmt.font = Some("Segoe UI Semibold".into());
    let tag = line("Studio projektowe · skład i druk", 9.5, gray, false, Align::Left);
    add(&mut d, 0, (ml + 66.0, 50.0, 260.0, 52.0), Shape::rect(), text_kind(vec![name, tag], 0.0, VAlign::Center), WrapMode::None, None);
    let contact = line("ul. Ogrodowa 12, 00-001 Warszawa\nbiuro@zielonanuta.pl · +48 22 123 45 67", 8.5, gray, false, Align::Right);
    add(&mut d, 0, (w - ml - 220.0, 54.0, 220.0, 44.0), Shape::rect(), text_kind(vec![contact], 0.0, VAlign::Center), WrapMode::None, None);
    add(&mut d, 0, (ml, 116.0, cw, 1.5), Shape::rect(), ObjKind::Shape, WrapMode::None, gradient(green, Color::rgb(0x2b, 0xb3, 0x86).with_alpha(0), 0.0));
    add(&mut d, 0, (0.0, h - 18.0, w, 18.0), Shape::rect(), ObjKind::Shape, WrapMode::Behind, gradient(green, Color::rgb(0x2b, 0xb3, 0x86), 0.0));
    let mut right = para("normal", "Warszawa, 12 listopada 2026 r.");
    right.fmt.align = Some(Align::Right);
    let mut addr = rich("normal", "**Pani Anna Kowalska**\nDział Marketingu\nWydawnictwo Przykładowe sp. z o.o.\nul. Słoneczna 5, 30-001 Kraków");
    addr.fmt.indent_left = Some(cw * 0.52);
    addr.fmt.space_before = Some(22.0);
    let mut subj = rich("normal", "**Dotyczy: oferty składu i przygotowania do druku katalogu 2027**");
    subj.fmt.space_before = Some(26.0);
    let body = |t: &str| {
        let mut p = para("normal", t);
        p.fmt.align = Some(Align::Justify);
        p
    };
    let mut sign = rich("normal", "Z wyrazami szacunku\n\n\n**Tomasz Zieliński**\nKierownik studia");
    sign.fmt.indent_left = Some(cw * 0.55);
    sign.fmt.space_before = Some(20.0);
    d.body = Story {
        paras: vec![
            right,
            addr,
            subj,
            para("normal", "Szanowna Pani,"),
            body("dziękujemy za zainteresowanie naszą ofertą. W załączeniu przesyłamy wycenę składu i przygotowania do druku katalogu produktów na 2027 rok – w dwóch wariantach objętości oraz z opcją wersji elektronicznej."),
            body("Proponujemy, aby pierwsza makieta była gotowa w ciągu dwóch tygodni od otrzymania materiałów. Pliki do druku przygotujemy w CMYK, ze spadami i znacznikami cięcia, zgodnie z wymaganiami wybranej drukarni."),
            body("Chętnie omówimy szczegóły podczas spotkania lub rozmowy telefonicznej w dogodnym dla Pani terminie."),
            sign,
        ],
    };
    d
}

/// Gazetka: winieta z gradientem, numer i data, trzy łamy z linią, inicjały, cytat i ilustracja.
fn newsletter() -> Document {
    let mut d = Document::new();
    d.meta.title = "Gazetka".into();
    d.settings.hyphenate = true;
    crate::styles::apply_theme(&mut d, &crate::styles::THEMES[1]);
    let red = Color::rgb(0x9b, 0x1c, 0x2c);
    let dark = Color::rgb(0x2a, 0x0b, 0x12);
    let w = d.page.width;
    d.page.columns = 3;
    d.page.column_rule = true;
    d.page.column_gap = 14.0;
    d.page.margin_top = 150.0;
    d.page.margin_left = 44.0;
    d.page.margin_right = 44.0;
    set_style(&mut d, "normal", |s| {
        s.chars.size = Some(9.5);
        s.para.line = Some(LineSpacing::Multiple(1.18));
        s.para.space_after = Some(5.0);
        s.para.align = Some(Align::Justify);
    });
    set_style(&mut d, "heading2", |s| {
        s.chars.size = Some(15.0);
        s.chars.color = Some(red);
        s.para.space_before = Some(10.0);
    });
    // Winieta.
    add(&mut d, 0, (0.0, 0.0, w, 118.0), Shape::rect(), ObjKind::Shape, WrapMode::Behind, gradient(red, dark, 0.0));
    let mut title = line("GŁOS OSIEDLA", 46.0, Color::WHITE, true, Align::Center);
    title.runs[0].fmt.tracking = Some(5.0);
    title.runs[0].fmt.font = Some("Georgia".into());
    title.runs[0].fmt.effect = Some(TextEffect::Shadow);
    add(&mut d, 0, (0.0, 14.0, w, 70.0), Shape::rect(), text_kind(vec![title], 0.0, VAlign::Center), WrapMode::None, None);
    let mut issue = line("NR 11 / 2026   ·   BEZPŁATNY MIESIĘCZNIK MIESZKAŃCÓW   ·   LISTOPAD", 8.5, Color::rgb(0xf6, 0xd6, 0xda), true, Align::Center);
    issue.runs[0].fmt.tracking = Some(2.0);
    add(&mut d, 0, (0.0, 84.0, w, 24.0), Shape::rect(), text_kind(vec![issue], 0.0, VAlign::Center), WrapMode::None, None);
    add(&mut d, 0, (44.0, 128.0, w - 88.0, 1.2), Shape::rect(), ObjKind::Shape, WrapMode::None, solid(red));

    let arts = [
        ("Nowy park już otwarty", "Po dwóch latach prac park przy ul. Lipowej przyjął pierwszych spacerowiczów. Powstały alejki, plac zabaw, siłownia pod chmurką i staw z pomostem. Mieszkańcy posadzili ponad dwieście drzew podczas wspólnej akcji sąsiedzkiej, a rada osiedla zapowiada letnie kino plenerowe."),
        ("Kalendarz wydarzeń", "W listopadzie zapraszamy na jarmark rękodzieła, wieczór poezji w bibliotece i warsztaty fotograficzne dla młodzieży. Szczegóły i zapisy w domu kultury. Na zakończenie miesiąca – koncert chóru seniorów w kościele parafialnym."),
        ("Ludzie z sąsiedztwa", "Pani Helena od czterdziestu lat prowadzi kwiaciarnię przy rynku. W rozmowie opowiada o tym, jak zmieniało się osiedle, skąd bierze najpiękniejsze chryzantemy i dlaczego nigdy nie zamknęła sklepu w niedzielę przed Wszystkimi Świętymi."),
        ("Bezpieczna droga do szkoły", "Przy szkole podstawowej stanęły nowe progi zwalniające i przejście dla pieszych z oświetleniem. Rodzice proszą kierowców o ostrożność, szczególnie rano. Straż miejska zapowiada kontrole prędkości w pierwszych tygodniach."),
    ];
    let mut paras = Vec::new();
    for (k, (hd, body)) in arts.iter().enumerate() {
        let mut h2 = para("heading2", hd);
        if k == 0 {
            h2.fmt.space_before = Some(0.0);
        }
        paras.push(h2);
        let mut p = para("normal", body);
        p.fmt.drop_cap = Some(3);
        paras.push(p);
        paras.push(para("normal", "Masz temat do kolejnego numeru? Napisz do redakcji – najciekawsze historie i zdjęcia mieszkańców publikujemy w każdym wydaniu gazetki. Redakcja zastrzega sobie prawo do skracania tekstów."));
    }
    d.body = Story { paras };
    let mut q = line("„Osiedle to ludzie, nie bloki.”", 15.0, red, false, Align::Center);
    q.runs[0].fmt.italic = Some(true);
    q.runs[0].fmt.font = Some("Georgia".into());
    let qid = add(&mut d, 0, (w / 2.0 - 100.0, 520.0, 200.0, 86.0), Shape::rect(), text_kind(vec![q], 12.0, VAlign::Center), WrapMode::Both, solid(Color::rgb(0xfb, 0xf1, 0xf2)));
    if let Some(o) = d.object_mut(qid) {
        o.corner = 10.0;
        o.wrap.contour = false;
        o.stroke = Some(StrokeStyle { color: red, width: 1.0, dash: vec![] });
    }
    let leaf = r##"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="160" viewBox="0 0 160 160"><circle cx="80" cy="80" r="76" fill="#fbe9eb"/><path d="M80 130 C30 110 30 50 80 28 C130 50 130 110 80 130 Z" fill="#9b1c2c"/><path d="M80 130 L80 40" stroke="#fbe9eb" stroke-width="4"/><path d="M80 90 L58 70 M80 74 L100 56 M80 108 L104 88" stroke="#fbe9eb" stroke-width="3" fill="none"/></svg>"##;
    if let Some(key) = svg_asset(&mut d, leaf) {
        let id = add(&mut d, 0, (w - 44.0 - 150.0, 230.0, 120.0, 120.0), geom::ellipse(), ObjKind::Image { asset: key, fit: ImageFit::Contain, contour: None }, WrapMode::Both, None);
        if let Some(o) = d.object_mut(id) {
            o.wrap.dist = 8.0;
        }
    }
    d.footer = Some(footer_of(Color::rgb(0x88, 0x70, 0x74)));
    d
}

/// Ulotka: ciemne tło z gradientem, hasło z poświatą, tekst w kole, tekst na łuku i pasek z datą.
fn flyer() -> Document {
    let mut d = Document::new();
    d.meta.title = "Ulotka".into();
    let (w, h) = (d.page.width, d.page.height);
    let yellow = Color::rgb(0xff, 0xd6, 0x0a);
    let pink = Color::rgb(0xff, 0x3d, 0x7f);
    add(&mut d, 0, (0.0, 0.0, w, h), Shape::rect(), ObjKind::Shape, WrapMode::Behind, gradient(Color::rgb(0x12, 0x0b, 0x2a), Color::rgb(0x05, 0x05, 0x0c), 90.0));
    add(&mut d, 0, (-120.0, -120.0, 420.0, 420.0), geom::ellipse(), ObjKind::Shape, WrapMode::Behind, Some(Fill::Radial { from: pink.with_alpha(170), to: pink.with_alpha(0) }));
    add(&mut d, 0, (w - 260.0, h - 380.0, 420.0, 420.0), geom::ellipse(), ObjKind::Shape, WrapMode::Behind, Some(Fill::Radial { from: yellow.with_alpha(120), to: yellow.with_alpha(0) }));
    for (k, (x, y, r)) in [(70.0, 640.0, 26.0), (w - 110.0, 120.0, 34.0), (w - 160.0, 660.0, 16.0)].iter().enumerate() {
        add(&mut d, 0, (*x, *y, *r * 2.0, *r * 2.0), geom::star(if k == 1 { 8 } else { 5 }, 0.45), ObjKind::Shape, WrapMode::Behind, solid(yellow.with_alpha(200)));
    }
    let mut kicker = line("ZAPRASZAMY NA", 13.0, pink, true, Align::Center);
    kicker.runs[0].fmt.tracking = Some(6.0);
    let mut big = line("WIELKIE\nOTWARCIE", 64.0, yellow, true, Align::Center);
    big.runs[0].fmt.font = Some("Segoe UI Black".into());
    big.runs[0].fmt.effect = Some(TextEffect::Glow);
    big.fmt.line = Some(LineSpacing::Multiple(0.88));
    add(&mut d, 0, (40.0, 70.0, w - 80.0, 200.0), Shape::rect(), text_kind(vec![kicker, big], 0.0, VAlign::Center), WrapMode::None, None);
    let mut arc = line("★  KAWIARNIA  ·  KSIĘGARNIA  ·  GALERIA  ★", 13.0, Color::WHITE, true, Align::Center);
    arc.runs[0].fmt.tracking = Some(3.0);
    add(&mut d, 0, (w / 2.0 - 190.0, 300.0, 380.0, 80.0), geom::arc_path(), ObjKind::PathText { story: Story { paras: vec![arc] }, offset: 0.0 }, WrapMode::None, None);
    let mut inner = line("Kawa na start gratis, warsztaty dla dzieci, koncert akustyczny o 18:00 i rabat 20% na wszystkie książki przez cały weekend. Przyjdź z przyjaciółmi!", 13.5, Color::rgb(0x18, 0x10, 0x20), false, Align::Center);
    inner.fmt.line = Some(LineSpacing::Multiple(1.2));
    let cid = add(&mut d, 0, (w / 2.0 - 170.0, 360.0, 340.0, 340.0), geom::ellipse(), text_kind(vec![inner], 46.0, VAlign::Center), WrapMode::None, Some(Fill::Radial { from: Color::rgb(0xff, 0xf0, 0x8a), to: yellow }));
    if let Some(o) = d.object_mut(cid) {
        o.shadow = true;
    }
    let mut when = line("SOBOTA 14.11  ·  12:00  ·  RYNEK 7", 20.0, Color::rgb(0x12, 0x0b, 0x2a), true, Align::Center);
    when.runs[0].fmt.tracking = Some(2.5);
    let sid = add(&mut d, 0, (60.0, h - 120.0, w - 120.0, 50.0), Shape::rect(), text_kind(vec![when], 4.0, VAlign::Center), WrapMode::None, gradient(pink, yellow, 0.0));
    if let Some(o) = d.object_mut(sid) {
        o.corner = 25.0;
    }
    d
}

/// CV: kolumna boczna z gradientem (zdjęcie w kole, kontakt, umiejętności z paskami), treść po prawej.
fn cv() -> Document {
    let mut d = Document::new();
    d.meta.title = "CV".into();
    let (w, h) = (d.page.width, d.page.height);
    let teal = Color::rgb(0x0e, 0x6e, 0x7a);
    let dark = Color::rgb(0x0b, 0x2c, 0x3a);
    let side = 200.0;
    d.page.margin_left = side + 34.0;
    d.page.margin_right = 40.0;
    d.page.margin_top = 52.0;
    d.page.margin_bottom = 46.0;
    set_style(&mut d, "normal", |s| {
        s.chars.font = Some("Segoe UI".into());
        s.chars.size = Some(9.5);
        s.para.line = Some(LineSpacing::Multiple(1.2));
        s.para.space_after = Some(4.0);
    });
    set_style(&mut d, "heading1", |s| {
        s.chars.font = Some("Segoe UI Semibold".into());
        s.chars.size = Some(12.0);
        s.chars.color = Some(teal);
        s.chars.caps = Some(Caps::All);
        s.chars.tracking = Some(1.5);
        s.para.space_before = Some(16.0);
        s.para.space_after = Some(6.0);
    });
    set_style(&mut d, "heading2", |s| {
        s.chars.font = Some("Segoe UI Semibold".into());
        s.chars.size = Some(10.5);
        s.chars.color = Some(dark);
        s.para.space_before = Some(8.0);
        s.para.space_after = Some(1.0);
    });
    add(&mut d, 0, (0.0, 0.0, side, h), Shape::rect(), ObjKind::Shape, WrapMode::Behind, gradient(dark, teal, 90.0));
    // Zdjęcie: koło z inicjałami (wstaw własne zdjęcie: Wstawianie → Obraz, potem kształt koła).
    let mut ini = line("AN", 30.0, Color::WHITE, true, Align::Center);
    ini.runs[0].fmt.font = Some("Segoe UI Light".into());
    let pid = add(&mut d, 0, (side / 2.0 - 56.0, 48.0, 112.0, 112.0), geom::ellipse(), text_kind(vec![ini], 2.0, VAlign::Center), WrapMode::None, solid(Color::WHITE.with_alpha(40)));
    if let Some(o) = d.object_mut(pid) {
        o.stroke = Some(StrokeStyle { color: Color::WHITE.with_alpha(160), width: 2.0, dash: vec![] });
    }
    let white = |t: &str, size: f32, bold: bool| line(t, size, Color::WHITE, bold, Align::Left);
    let soft = |t: &str| line(t, 9.0, Color::rgb(0xcf, 0xe8, 0xec), false, Align::Left);
    let mut info = vec![white("KONTAKT", 9.0, true), soft("anna.nowak@poczta.pl"), soft("+48 600 100 200"), soft("Kraków"), soft("linkedin.com/in/annanowak")];
    info[0].runs[0].fmt.tracking = Some(2.0);
    info[0].fmt.space_after = Some(4.0);
    let mut sk = white("UMIEJĘTNOŚCI", 9.0, true);
    sk.runs[0].fmt.tracking = Some(2.0);
    sk.fmt.space_before = Some(16.0);
    info.push(sk);
    add(&mut d, 0, (26.0, 190.0, side - 44.0, 150.0), Shape::rect(), text_kind(info, 0.0, VAlign::Top), WrapMode::None, None);
    for (k, (name, lvl)) in [("Skład i DTP", 0.95), ("Typografia", 0.9), ("Projektowanie graficzne", 0.8), ("Przygotowanie do druku", 0.85), ("Język angielski", 0.75)].iter().enumerate() {
        let y = 352.0 + k as f32 * 34.0;
        add(&mut d, 0, (26.0, y, side - 44.0, 16.0), Shape::rect(), text_kind(vec![soft(name)], 0.0, VAlign::Top), WrapMode::None, None);
        let bw = side - 52.0;
        let bg = add(&mut d, 0, (26.0, y + 18.0, bw, 5.0), Shape::rect(), ObjKind::Shape, WrapMode::None, solid(Color::WHITE.with_alpha(45)));
        let fg = add(&mut d, 0, (26.0, y + 18.0, bw * *lvl as f32, 5.0), Shape::rect(), ObjKind::Shape, WrapMode::None, gradient(Color::rgb(0x7f, 0xe6, 0xd9), Color::WHITE, 0.0));
        for id in [bg, fg] {
            if let Some(o) = d.object_mut(id) {
                o.corner = 2.5;
            }
        }
    }
    let mut name = para("title", "Anna Nowak");
    name.runs[0].fmt = CharFmt { font: Some("Segoe UI Light".into()), size: Some(32.0), color: Some(dark), ..Default::default() };
    name.fmt.space_after = Some(0.0);
    let mut role = para("subtitle", "Specjalistka ds. składu i przygotowania do druku");
    role.runs[0].fmt = CharFmt { size: Some(12.0), color: Some(teal), ..Default::default() };
    let job = |title: &str, place: &str, desc: &str| {
        let mut v = vec![para("heading2", title)];
        let mut p = para("normal", place);
        p.runs[0].fmt = CharFmt { italic: Some(true), color: Some(Color::rgb(0x70, 0x7a, 0x80)), size: Some(9.0), ..Default::default() };
        v.push(p);
        v.push(para("normal", desc));
        v
    };
    let mut paras = vec![
        name,
        role,
        para("heading1", "Profil"),
        para("normal", "Od dziewięciu lat składam książki, czasopisma i materiały reklamowe. Łączę wyczucie typografii z praktyczną znajomością druku – od makiety po pliki dla drukarni. Lubię porządek w stylach, przemyślane siatki i terminy dotrzymane co do dnia."),
        para("heading1", "Doświadczenie"),
    ];
    paras.extend(job("Starsza składaczka", "Wydawnictwo Literackie · 2021 – obecnie", "Skład kilkudziesięciu tytułów rocznie, przygotowanie plików do druku offsetowego, szkolenia młodszych pracowników z typografii i stylów."));
    paras.extend(job("Grafik DTP", "Agencja Kreatywna Iskra · 2017 – 2021", "Katalogi, ulotki i raporty roczne dla klientów z branży finansowej i edukacyjnej; współpraca z drukarniami w kraju i za granicą."));
    paras.push(para("heading1", "Wykształcenie"));
    paras.extend(job("Grafika – projektowanie książki", "Akademia Sztuk Pięknych w Krakowie · 2012 – 2017", "Dyplom z wyróżnieniem za projekt serii wydawniczej poezji współczesnej."));
    paras.push(para("heading1", "Zainteresowania"));
    paras.push(para("normal", "Stare kroje drukarskie, fotografia analogowa, wędrówki po Beskidach."));
    d.body = Story { paras };
    d
}

/// Zaproszenie: kremowy papier, podwójna ramka, ornament z gwiazd, tekst na łuku i kropki dekoracyjne.
fn invitation() -> Document {
    let mut d = Document::new();
    d.meta.title = "Zaproszenie".into();
    let (w, h) = (d.page.width, d.page.height);
    let gold = Color::rgb(0xb0, 0x86, 0x2f);
    let ink = Color::rgb(0x2b, 0x23, 0x1a);
    d.page.color = Color::rgb(0xfb, 0xf6, 0xec);
    d.page.border = Some(StrokeStyle { color: gold, width: 1.2, dash: vec![] });
    d.page.border_double = true;
    d.page.border_inset = 28.0;
    add(&mut d, 0, (w / 2.0 - 180.0, 90.0, 360.0, 360.0), geom::ellipse(), ObjKind::Shape, WrapMode::Behind, Some(Fill::Radial { from: Color::rgb(0xf3, 0xe2, 0xbc).with_alpha(200), to: Color::rgb(0xf3, 0xe2, 0xbc).with_alpha(0) }));
    let mut arc = line("SERDECZNIE ZAPRASZAMY", 13.0, gold, true, Align::Center);
    arc.runs[0].fmt.tracking = Some(5.0);
    arc.runs[0].fmt.font = Some("Georgia".into());
    add(&mut d, 0, (w / 2.0 - 170.0, 120.0, 340.0, 90.0), geom::arc_path(), ObjKind::PathText { story: Story { paras: vec![arc] }, offset: 0.0 }, WrapMode::None, None);
    for (k, x) in [w / 2.0 - 60.0, w / 2.0 - 9.0, w / 2.0 + 42.0].iter().enumerate() {
        let s = if k == 1 { 18.0 } else { 12.0 };
        add(&mut d, 0, (*x + (18.0 - s) / 2.0, 222.0 + (18.0 - s) / 2.0, s, s), geom::star(4, 0.4), ObjKind::Shape, WrapMode::None, solid(gold));
    }
    let mut names = line("Ola & Michał", 46.0, ink, false, Align::Center);
    names.runs[0].fmt.font = Some("Gabriola".into());
    names.runs[0].fmt.italic = Some(true);
    let mut what = line("na uroczystość zaślubin", 14.0, ink, false, Align::Center);
    what.runs[0].fmt.font = Some("Georgia".into());
    what.runs[0].fmt.italic = Some(true);
    what.fmt.space_before = Some(6.0);
    let mut date = line("SOBOTA, 20 CZERWCA 2027 · GODZ. 15:00", 12.0, gold, true, Align::Center);
    date.runs[0].fmt.tracking = Some(2.5);
    date.fmt.space_before = Some(26.0);
    let mut place = line("Kościół św. Anny w Krakowie\nprzyjęcie: Dworek pod Lipami, Zabierzów", 12.0, ink, false, Align::Center);
    place.runs[0].fmt.font = Some("Georgia".into());
    place.fmt.space_before = Some(10.0);
    place.fmt.line = Some(LineSpacing::Multiple(1.35));
    let mut rsvp = line("Prosimy o potwierdzenie przybycia do 1 maja · tel. 600 700 800", 9.5, Color::rgb(0x80, 0x70, 0x5c), false, Align::Center);
    rsvp.fmt.space_before = Some(40.0);
    add(&mut d, 0, (70.0, 260.0, w - 140.0, 380.0), Shape::rect(), text_kind(vec![names, what, date, place, rsvp], 6.0, VAlign::Top), WrapMode::None, None);
    add(&mut d, 0, (w / 2.0 - 70.0, h - 140.0, 140.0, 1.0), Shape::rect(), ObjKind::Shape, WrapMode::None, solid(gold));
    add(&mut d, 0, (w / 2.0 - 7.0, h - 147.0, 14.0, 14.0), preset("diamond"), ObjKind::Shape, WrapMode::None, solid(gold));
    d
}

/// Dyplom: strona pozioma, ozdobna ramka, pieczęć w gwieździe z gradientem i miejsca na podpisy.
fn certificate() -> Document {
    let mut d = Document::new();
    d.meta.title = "Dyplom".into();
    let (pw, ph) = (d.page.width, d.page.height);
    d.page.width = ph;
    d.page.height = pw;
    let (w, h) = (ph, pw);
    let navy = Color::rgb(0x14, 0x2b, 0x55);
    let gold = Color::rgb(0xc8, 0x9b, 0x3c);
    d.page.color = Color::rgb(0xfd, 0xfb, 0xf5);
    d.page.border = Some(StrokeStyle { color: navy, width: 2.0, dash: vec![] });
    d.page.border_double = true;
    d.page.border_inset = 22.0;
    add(&mut d, 0, (44.0, 44.0, w - 88.0, h - 88.0), Shape::rect(), ObjKind::Shape, WrapMode::Behind, None);
    if let Some(o) = d.objects.last_mut() {
        o.stroke = Some(StrokeStyle { color: gold, width: 0.8, dash: vec![3.0, 3.0] });
        o.corner = 6.0;
    }
    for (x, y) in [(36.0, 36.0), (w - 66.0, 36.0), (36.0, h - 66.0), (w - 66.0, h - 66.0)] {
        add(&mut d, 0, (x, y, 30.0, 30.0), preset("diamond"), ObjKind::Shape, WrapMode::None, solid(gold));
    }
    let mut k = line("DYPLOM", 46.0, navy, true, Align::Center);
    k.runs[0].fmt.tracking = Some(14.0);
    k.runs[0].fmt.font = Some("Georgia".into());
    let mut sub = line("UZNANIA", 13.0, gold, true, Align::Center);
    sub.runs[0].fmt.tracking = Some(8.0);
    let mut given = line("przyznany dla", 13.0, Color::rgb(0x55, 0x5e, 0x70), false, Align::Center);
    given.runs[0].fmt.italic = Some(true);
    given.fmt.space_before = Some(22.0);
    let mut who = line("Katarzyny Wiśniewskiej", 34.0, navy, false, Align::Center);
    who.runs[0].fmt.font = Some("Gabriola".into());
    who.fmt.space_before = Some(6.0);
    let mut why = line("za wybitne osiągnięcia w Ogólnopolskim Konkursie Typograficznym\n„Litera i Forma” 2026", 13.0, Color::rgb(0x33, 0x3a, 0x48), false, Align::Center);
    why.runs[0].fmt.font = Some("Georgia".into());
    why.fmt.space_before = Some(14.0);
    why.fmt.line = Some(LineSpacing::Multiple(1.3));
    add(&mut d, 0, (100.0, 78.0, w - 200.0, 300.0), Shape::rect(), text_kind(vec![k, sub, given, who, why], 0.0, VAlign::Top), WrapMode::None, None);
    let mut seal = line("2026", 15.0, Color::WHITE, true, Align::Center);
    seal.runs[0].fmt.font = Some("Georgia".into());
    let sid = add(&mut d, 0, (w / 2.0 - 55.0, h - 190.0, 110.0, 110.0), geom::star(24, 0.86), text_kind(vec![seal], 20.0, VAlign::Center), WrapMode::None, Some(Fill::Radial { from: Color::rgb(0xf0, 0xd0, 0x80), to: gold }));
    if let Some(o) = d.object_mut(sid) {
        o.shadow = true;
    }
    for (k, (x, label)) in [(110.0, "Przewodniczący jury"), (w - 310.0, "Dyrektor konkursu")].iter().enumerate() {
        let _ = k;
        add(&mut d, 0, (*x, h - 120.0, 200.0, 0.8), Shape::rect(), ObjKind::Shape, WrapMode::None, solid(navy));
        add(&mut d, 0, (*x, h - 114.0, 200.0, 20.0), Shape::rect(), text_kind(vec![line(label, 9.5, Color::rgb(0x55, 0x5e, 0x70), false, Align::Center)], 0.0, VAlign::Top), WrapMode::None, None);
    }
    d
}

/// Menu restauracji: ciemne tło, złote akcenty, dania z cenami na tabulatorze z kropkami.
fn menu() -> Document {
    let mut d = Document::new();
    d.meta.title = "Menu".into();
    let (w, h) = (d.page.width, d.page.height);
    let gold = Color::rgb(0xd4, 0xaf, 0x6a);
    let cream = Color::rgb(0xf3, 0xea, 0xd8);
    d.page.color = Color::rgb(0x17, 0x14, 0x12);
    d.page.margin_top = 190.0;
    d.page.margin_left = 80.0;
    d.page.margin_right = 80.0;
    set_style(&mut d, "normal", |s| {
        s.chars.font = Some("Georgia".into());
        s.chars.size = Some(11.0);
        s.chars.color = Some(cream);
        s.para.space_after = Some(1.0);
    });
    set_style(&mut d, "heading2", |s| {
        s.chars.font = Some("Georgia".into());
        s.chars.size = Some(13.0);
        s.chars.color = Some(gold);
        s.chars.caps = Some(Caps::All);
        s.chars.tracking = Some(4.0);
        s.chars.bold = Some(false);
        s.para.align = Some(Align::Center);
        s.para.space_before = Some(20.0);
        s.para.space_after = Some(10.0);
    });
    d.page.border = Some(StrokeStyle { color: gold, width: 0.8, dash: vec![] });
    d.page.border_double = true;
    d.page.border_inset = 26.0;
    let mut name = line("Pod Złotą Łyżką", 38.0, gold, false, Align::Center);
    name.runs[0].fmt.font = Some("Gabriola".into());
    let mut tag = line("KUCHNIA POLSKA  ·  OD 1998", 9.0, cream, false, Align::Center);
    tag.runs[0].fmt.tracking = Some(5.0);
    add(&mut d, 0, (60.0, 70.0, w - 120.0, 90.0), Shape::rect(), text_kind(vec![name, tag], 0.0, VAlign::Center), WrapMode::None, None);
    add(&mut d, 0, (w / 2.0 - 60.0, 166.0, 120.0, 0.8), Shape::rect(), ObjKind::Shape, WrapMode::None, solid(gold));
    add(&mut d, 0, (w / 2.0 - 5.0, 161.0, 10.0, 10.0), preset("diamond"), ObjKind::Shape, WrapMode::None, solid(gold));
    let dish = |name: &str, price: &str, desc: &str| {
        let mut p = runs("normal", vec![run(name, CharFmt { bold: Some(true), ..Default::default() }), plain(&format!("\t{price}"))]);
        p.fmt.tabs = Some(vec![-1.0]);
        p.fmt.space_before = Some(6.0);
        let mut q = para("normal", desc);
        q.runs[0].fmt = CharFmt { italic: Some(true), size: Some(9.5), color: Some(Color::rgb(0xb8, 0xae, 0x9c)), ..Default::default() };
        vec![p, q]
    };
    let mut paras = Vec::new();
    for (sec, items) in [
        ("Przystawki", vec![("Tatar wołowy", "42 zł", "z marynowanym grzybem, żółtkiem i grzanką na maśle"), ("Śledź w trzech odsłonach", "34 zł", "w oleju lnianym, w śmietanie i w occie jabłkowym"), ("Pierogi z kaczką", "38 zł", "z sosem wiśniowym i prażonymi orzechami")]),
        ("Zupy", vec![("Żurek na zakwasie", "24 zł", "z białą kiełbasą, jajkiem i chrzanem"), ("Krem z dyni", "22 zł", "z olejem z pestek i grzankami ziołowymi")]),
        ("Dania główne", vec![("Polędwica z jelenia", "89 zł", "z puree z selera, borówkami i sosem z jałowca"), ("Sandacz z masłem", "72 zł", "z młodymi ziemniakami i fasolką szparagową"), ("Kaczka pieczona", "68 zł", "z jabłkami, modrą kapustą i kluskami śląskimi")]),
        ("Desery", vec![("Sernik baskijski", "26 zł", "z sosem z kwaśnych wiśni"), ("Szarlotka na ciepło", "24 zł", "z lodami waniliowymi i kruszonką")]),
    ] {
        paras.push(para("heading2", sec));
        for (n, p, ds) in items {
            paras.extend(dish(n, p, ds));
        }
    }
    let mut foot = line("Ceny zawierają podatek VAT. Zapytaj kelnera o dania bezglutenowe i wegańskie.", 8.5, Color::rgb(0x9a, 0x90, 0x80), false, Align::Center);
    foot.fmt.space_before = Some(26.0);
    paras.push(foot);
    d.body = Story { paras };
    add(&mut d, 0, (w / 2.0 - 30.0, h - 70.0, 60.0, 14.0), geom::star(4, 0.35), ObjKind::Shape, WrapMode::None, solid(gold));
    d
}

/// Faktura: nagłówek z logo, dane sprzedawcy i nabywcy w ramkach, tabela pozycji, suma w wyróżnieniu.
fn invoice() -> Document {
    let mut d = Document::new();
    d.meta.title = "Faktura".into();
    let w = d.page.width;
    let ml = d.page.margin_left;
    let cw = d.page.content_width();
    let blue = Color::rgb(0x25, 0x4e, 0xdb);
    let gray = Color::rgb(0x6b, 0x72, 0x80);
    d.page.margin_top = 250.0;
    set_style(&mut d, "normal", |s| {
        s.chars.font = Some("Segoe UI".into());
        s.chars.size = Some(9.5);
        s.para.space_after = Some(3.0);
    });
    let mut logo = line("◆ Pracownia Liter", 16.0, blue, true, Align::Left);
    logo.runs[0].fmt.font = Some("Segoe UI Semibold".into());
    add(&mut d, 0, (ml, 50.0, 260.0, 30.0), Shape::rect(), text_kind(vec![logo], 0.0, VAlign::Center), WrapMode::None, None);
    let mut title = line("FAKTURA VAT", 22.0, Color::rgb(0x11, 0x18, 0x27), true, Align::Right);
    title.runs[0].fmt.tracking = Some(2.0);
    let num = line("nr FV/2026/11/042", 10.0, gray, false, Align::Right);
    let dates = line("Data wystawienia: 14.11.2026\nTermin płatności: 28.11.2026", 9.0, gray, false, Align::Right);
    add(&mut d, 0, (w - ml - 240.0, 44.0, 240.0, 80.0), Shape::rect(), text_kind(vec![title, num, dates], 0.0, VAlign::Top), WrapMode::None, None);
    let half = (cw - 16.0) / 2.0;
    for (k, (head, body)) in [("SPRZEDAWCA", "Pracownia Liter sp. z o.o.\nul. Drukarska 8, 31-001 Kraków\nNIP 676-000-00-00\nmBank 00 1140 0000 0000 0000 0000 0000"), ("NABYWCA", "Wydawnictwo Przykładowe sp. z o.o.\nul. Słoneczna 5, 30-001 Kraków\nNIP 675-111-11-11")].iter().enumerate() {
        let mut hd = line(head, 8.5, blue, true, Align::Left);
        hd.runs[0].fmt.tracking = Some(2.0);
        hd.fmt.space_after = Some(4.0);
        let bd = line(body, 9.5, Color::rgb(0x1f, 0x29, 0x37), false, Align::Left);
        let id = add(&mut d, 0, (ml + k as f32 * (half + 16.0), 140.0, half, 92.0), Shape::rect(), text_kind(vec![hd, bd], 12.0, VAlign::Top), WrapMode::None, solid(Color::rgb(0xf4, 0xf6, 0xfb)));
        if let Some(o) = d.object_mut(id) {
            o.corner = 8.0;
        }
    }
    let table = Table {
        rows: vec![
            vec!["Lp.".into(), "Nazwa usługi".into(), "Ilość".into(), "Cena netto".into(), "VAT".into(), "Wartość brutto".into()],
            vec!["1".into(), "Skład katalogu – 64 strony".into(), "1".into(), "4 800,00".into(), "23%".into(), "5 904,00".into()],
            vec!["2".into(), "Przygotowanie plików do druku (CMYK, spady)".into(), "1".into(), "900,00".into(), "23%".into(), "1 107,00".into()],
            vec!["3".into(), "Korekta typograficzna".into(), "64".into(), "15,00".into(), "23%".into(), "1 180,80".into()],
            vec!["4".into(), "Wersja elektroniczna EPUB".into(), "1".into(), "600,00".into(), "23%".into(), "738,00".into()],
        ],
        header: true,
        stripes: true,
        font: "Segoe UI".into(),
        size: 9.0,
        accent: blue,
        line: Color::rgb(0xd6, 0xdd, 0xf0),
        numbers_right: true,
        widths: Vec::new(),
        take: None,
    };
    let th = crate::paint::table_height(&table, cw);
    let tid = add(&mut d, 0, (ml, 250.0, cw, th), Shape::rect(), ObjKind::Table { table }, WrapMode::TopBottom, solid(Color::WHITE));
    let mut sum1 = line("Razem netto: 7 260,00 zł   ·   VAT 23%: 1 669,80 zł", 9.5, gray, false, Align::Right);
    sum1.fmt.space_after = Some(4.0);
    let mut sum2 = line("DO ZAPŁATY: 8 929,80 zł", 16.0, Color::WHITE, true, Align::Right);
    sum2.runs[0].fmt.font = Some("Segoe UI Semibold".into());
    let sid = add(&mut d, 0, (w - ml - 280.0, 420.0, 280.0, 40.0), Shape::rect(), text_kind(vec![sum2], 12.0, VAlign::Center), WrapMode::TopBottom, gradient(blue, Color::rgb(0x5b, 0x3d, 0xf5), 0.0));
    if let Some(o) = d.object_mut(sid) {
        o.corner = 8.0;
        o.anchor_dy = 10.0;
    }
    if let Some(o) = d.object_mut(tid) {
        o.anchor_dy = 0.0;
    }
    let mut words = rich("normal", "Słownie: *osiem tysięcy dziewięćset dwadzieścia dziewięć złotych 80/100*");
    words.fmt.space_before = Some(16.0);
    let mut note = rich("normal", "Płatność przelewem na rachunek podany powyżej. W tytule prosimy wpisać numer faktury.");
    note.runs[0].fmt.color = Some(gray);
    let mut sign = line("…………………………………\nosoba upoważniona do wystawienia", 8.5, gray, false, Align::Right);
    sign.fmt.space_before = Some(60.0);
    d.body = Story { paras: vec![anchor_para(&[tid]), sum1, anchor_para(&[sid]), words, note, sign] };
    d
}

/// Plakat koncertowy: gradient, ogromny napis, koła w stylu winylu, tekst na łuku, lista wykonawców.
fn poster() -> Document {
    let mut d = Document::new();
    d.meta.title = "Plakat".into();
    let (w, h) = (d.page.width, d.page.height);
    let lime = Color::rgb(0xc6, 0xff, 0x3d);
    let pink = Color::rgb(0xff, 0x2e, 0x93);
    add(&mut d, 0, (0.0, 0.0, w, h), Shape::rect(), ObjKind::Shape, WrapMode::Behind, gradient(Color::rgb(0x1a, 0x05, 0x33), Color::rgb(0x00, 0x2b, 0x3d), 160.0));
    for (k, r) in [300.0f32, 230.0, 160.0, 90.0].iter().enumerate() {
        let c = if k % 2 == 0 { Color::rgb(0x0b, 0x0b, 0x12) } else { Color::rgb(0x22, 0x1c, 0x30) };
        add(&mut d, 0, (w / 2.0 - r, 330.0 - r, r * 2.0, r * 2.0), geom::ellipse(), ObjKind::Shape, WrapMode::Behind, solid(c));
    }
    add(&mut d, 0, (w / 2.0 - 40.0, 290.0, 80.0, 80.0), geom::ellipse(), ObjKind::Shape, WrapMode::Behind, Some(Fill::Radial { from: lime, to: Color::rgb(0x2e, 0xc4, 0x6a) }));
    let mut arc = line("FESTIWAL  ·  MUZYKI  ·  NOCĄ  ·", 15.0, lime, true, Align::Center);
    arc.runs[0].fmt.tracking = Some(6.0);
    add(&mut d, 0, (w / 2.0 - 230.0, 40.0, 460.0, 120.0), geom::arc_path(), ObjKind::PathText { story: Story { paras: vec![arc] }, offset: 0.0 }, WrapMode::None, None);
    let mut big = line("NOCNE\nDŹWIĘKI", 82.0, Color::WHITE, true, Align::Center);
    big.runs[0].fmt.font = Some("Segoe UI Black".into());
    big.runs[0].fmt.effect = Some(TextEffect::Glow);
    big.runs[0].fmt.tracking = Some(-2.0);
    big.fmt.line = Some(LineSpacing::Multiple(0.82));
    add(&mut d, 0, (20.0, 520.0, w - 40.0, 200.0), Shape::rect(), text_kind(vec![big], 0.0, VAlign::Center), WrapMode::None, None);
    let mut who = line("KWARTET JAZZOWY BURSZTYN  ·  DJ MORZE  ·  CHÓR MIEJSKI  ·  ORKIESTRA FALE", 10.5, pink, true, Align::Center);
    who.runs[0].fmt.tracking = Some(2.0);
    add(&mut d, 0, (40.0, 720.0, w - 80.0, 30.0), Shape::rect(), text_kind(vec![who], 0.0, VAlign::Center), WrapMode::None, None);
    let mut when = line("21–23 SIERPNIA 2027  ·  PLAŻA MIEJSKA  ·  WSTĘP WOLNY", 13.0, Color::rgb(0x1a, 0x05, 0x33), true, Align::Center);
    when.runs[0].fmt.tracking = Some(2.0);
    let sid = add(&mut d, 0, (50.0, h - 74.0, w - 100.0, 40.0), Shape::rect(), text_kind(vec![when], 4.0, VAlign::Center), WrapMode::None, gradient(lime, Color::rgb(0x3d, 0xe0, 0xff), 0.0));
    if let Some(o) = d.object_mut(sid) {
        o.corner = 20.0;
    }
    d
}

/// Kartka z życzeniami: pastelowy gradient, serca i gwiazdki, życzenia w kształcie koła.
fn card() -> Document {
    let mut d = Document::new();
    d.meta.title = "Kartka z życzeniami".into();
    let (w, h) = (d.page.width, d.page.height);
    let plum = Color::rgb(0x6b, 0x21, 0x5a);
    add(&mut d, 0, (0.0, 0.0, w, h), Shape::rect(), ObjKind::Shape, WrapMode::Behind, gradient(Color::rgb(0xff, 0xe9, 0xf1), Color::rgb(0xe7, 0xe4, 0xff), 135.0));
    for (x, y, s, c) in [(60.0, 80.0, 60.0, Color::rgb(0xff, 0x9e, 0xc0)), (w - 140.0, 120.0, 80.0, Color::rgb(0xc9, 0xb8, 0xff)), (90.0, h - 200.0, 70.0, Color::rgb(0xb8, 0xe6, 0xff)), (w - 120.0, h - 170.0, 50.0, Color::rgb(0xff, 0xc9, 0x8a))] {
        add(&mut d, 0, (x, y, s, s), preset("heart"), ObjKind::Shape, WrapMode::Behind, solid(c.with_alpha(200)));
    }
    for (x, y) in [(w / 2.0 - 150.0, 150.0), (w / 2.0 + 130.0, 230.0), (w / 2.0 - 190.0, 560.0), (w / 2.0 + 160.0, 640.0), (w / 2.0, 110.0)] {
        add(&mut d, 0, (x, y, 22.0, 22.0), geom::star(5, 0.45), ObjKind::Shape, WrapMode::Behind, solid(Color::rgb(0xff, 0xc4, 0x3d)));
    }
    let mut hi = line("Wszystkiego\nnajlepszego!", 44.0, plum, false, Align::Center);
    hi.runs[0].fmt.font = Some("Gabriola".into());
    hi.fmt.line = Some(LineSpacing::Multiple(0.9));
    add(&mut d, 0, (40.0, 200.0, w - 80.0, 150.0), Shape::rect(), text_kind(vec![hi], 0.0, VAlign::Center), WrapMode::None, None);
    let mut wish = line("Niech każdy dzień przynosi powody do uśmiechu, ludzi, z którymi chce się rozmawiać do późna, i odwagę, żeby spełniać nawet te najbardziej zwariowane marzenia. Sto lat!", 13.0, plum, false, Align::Center);
    wish.runs[0].fmt.font = Some("Georgia".into());
    wish.runs[0].fmt.italic = Some(true);
    wish.fmt.line = Some(LineSpacing::Multiple(1.25));
    let cid = add(&mut d, 0, (w / 2.0 - 150.0, 370.0, 300.0, 300.0), geom::ellipse(), text_kind(vec![wish], 40.0, VAlign::Center), WrapMode::None, solid(Color::WHITE.with_alpha(210)));
    if let Some(o) = d.object_mut(cid) {
        o.shadow = true;
        o.stroke = Some(StrokeStyle { color: Color::rgb(0xff, 0x9e, 0xc0), width: 1.5, dash: vec![4.0, 4.0] });
    }
    let mut from = line("— od całej rodziny", 12.0, plum, false, Align::Center);
    from.runs[0].fmt.font = Some("Gabriola".into());
    from.runs[0].fmt.size = Some(20.0);
    add(&mut d, 0, (40.0, 700.0, w - 80.0, 40.0), Shape::rect(), text_kind(vec![from], 0.0, VAlign::Center), WrapMode::None, None);
    d
}

/// Broszura składana na trzy: strona pozioma, trzy panele z linią, okładka w prawym panelu.
fn brochure() -> Document {
    let mut d = Document::new();
    d.meta.title = "Broszura".into();
    let (pw, ph) = (d.page.width, d.page.height);
    d.page.width = ph;
    d.page.height = pw;
    let (w, h) = (ph, pw);
    let green = Color::rgb(0x1b, 0x7f, 0x5a);
    let leaf = Color::rgb(0x8c, 0xd1, 0x6b);
    d.page.margin_left = 34.0;
    d.page.margin_right = 34.0;
    d.page.margin_top = 40.0;
    d.page.margin_bottom = 36.0;
    d.page.columns = 3;
    d.page.column_gap = 68.0;
    d.settings.hyphenate = true;
    set_style(&mut d, "normal", |s| {
        s.chars.font = Some("Segoe UI".into());
        s.chars.size = Some(9.5);
        s.para.line = Some(LineSpacing::Multiple(1.25));
        s.para.space_after = Some(5.0);
        s.para.align = Some(Align::Justify);
    });
    set_style(&mut d, "heading2", |s| {
        s.chars.font = Some("Segoe UI Semibold".into());
        s.chars.size = Some(13.0);
        s.chars.color = Some(green);
        s.para.space_before = Some(10.0);
    });
    // Okładka: prawy panel z gradientem.
    let panel = w / 3.0;
    add(&mut d, 0, (2.0 * panel, 0.0, panel, h), Shape::rect(), ObjKind::Shape, WrapMode::Behind, gradient(green, Color::rgb(0x0b, 0x3d, 0x2c), 90.0));
    add(&mut d, 0, (2.0 * panel + 40.0, 70.0, panel - 80.0, panel - 80.0), geom::ellipse(), ObjKind::Shape, WrapMode::Behind, Some(Fill::Radial { from: leaf, to: green }));
    let mut t = line("Ogród\nna balkonie", 30.0, Color::WHITE, true, Align::Center);
    t.runs[0].fmt.font = Some("Segoe UI Semibold".into());
    let s = line("Poradnik dla początkujących\nmiejskich ogrodników", 11.0, Color::rgb(0xd7, 0xf2, 0xc8), false, Align::Center);
    add(&mut d, 0, (2.0 * panel + 20.0, 330.0, panel - 40.0, 160.0), Shape::rect(), text_kind(vec![t, s], 0.0, VAlign::Top), WrapMode::None, None);
    // Tekst w dwóch pierwszych panelach; trzecia kolumna to okładka (pusta).
    let mut paras = vec![
        para("heading2", "Zacznij od słońca"),
        para("normal", "Zanim kupisz pierwszą sadzonkę, sprawdź, ile godzin słońca dostaje twój balkon. Strona południowa pokocha pomidory i zioła śródziemnomorskie, a północna – paprocie, bluszcz i mięty."),
        para("heading2", "Doniczki i ziemia"),
        para("normal", "Większa doniczka to mniej podlewania. Na dno wsyp keramzyt, a ziemię wymieszaj z kompostem. Rośliny jadalne sadź w osobnych pojemnikach, żeby łatwo je było przenosić."),
        bullet(ListKind::Check, 0, "otwory odpływowe w każdej doniczce"),
        bullet(ListKind::Check, 0, "podstawki chroniące balkon sąsiada"),
        bullet(ListKind::Check, 0, "lekka ziemia uniwersalna"),
    ];
    let mut h2 = para("heading2", "Kalendarz siewu");
    h2.fmt.break_before = Some(Break::Column);
    paras.push(h2);
    for (m, t) in [("Marzec", "rozsada pomidorów i papryki na parapecie"), ("Kwiecień", "rzodkiewka, sałata i szczypiorek w skrzynkach"), ("Maj", "wystawiamy rozsadę na balkon po zimnych ogrodnikach"), ("Czerwiec", "bazylia, nasturcja i pierwsze zbiory sałaty")] {
        paras.push(runs("normal", vec![run(m, CharFmt { bold: Some(true), color: Some(green), ..Default::default() }), plain(&format!(" – {t}"))]));
    }
    paras.push(rich("callout", "**Dobra rada:** podlewaj rano albo wieczorem – w południe woda szybko paruje, a mokre liście mogą się poparzyć."));
    let mut cover_break = Paragraph::default();
    cover_break.fmt.break_before = Some(Break::Column);
    paras.push(cover_break);
    d.body = Story { paras };
    // Linie zgięcia między panelami.
    for k in 1..3 {
        add(&mut d, 0, (panel * k as f32, 0.0, 0.6, h), Shape::rect(), ObjKind::Shape, WrapMode::Behind, solid(Color::rgb(0xd5, 0xe4, 0xdc)));
    }
    d
}

/// Rozdział książki: klasyczny układ powieściowy – numer i tytuł rozdziału, inicjał, wcięcia akapitów.
fn book() -> Document {
    let mut d = Document::new();
    d.meta.title = "Rozdział pierwszy".into();
    d.settings.hyphenate = true;
    let ink = Color::rgb(0x2a, 0x25, 0x20);
    let rust = Color::rgb(0x9a, 0x4a, 0x1c);
    d.page.width = 148.0 * MM;
    d.page.height = 210.0 * MM;
    d.page.margin_top = 54.0;
    d.page.margin_bottom = 56.0;
    d.page.margin_left = 50.0;
    d.page.margin_right = 44.0;
    set_style(&mut d, "normal", |s| {
        s.chars.font = Some("Garamond".into());
        s.chars.size = Some(11.0);
        s.chars.color = Some(ink);
        s.para.line = Some(LineSpacing::Multiple(1.3));
        s.para.space_after = Some(0.0);
        s.para.first_line = Some(16.0);
        s.para.align = Some(Align::Justify);
    });
    let mut num = line("ROZDZIAŁ PIERWSZY", 9.0, rust, false, Align::Center);
    num.runs[0].fmt.tracking = Some(4.0);
    num.fmt.space_before = Some(70.0);
    let mut title = line("Dom na końcu ulicy", 22.0, ink, false, Align::Center);
    title.runs[0].fmt.font = Some("Garamond".into());
    title.runs[0].fmt.italic = Some(true);
    title.fmt.space_before = Some(10.0);
    let mut orn = line("❦", 16.0, rust, false, Align::Center);
    orn.fmt.space_before = Some(8.0);
    orn.fmt.space_after = Some(30.0);
    let mut first = para("normal", "Deszcz zaczął padać dokładnie wtedy, gdy Marta postawiła walizkę na progu. Dom wyglądał inaczej, niż go zapamiętała: mniejszy, ciemniejszy, z okiennicami, które ktoś pomalował na kolor dojrzałych śliwek. Klucz pasował jednak do zamka i obrócił się bez oporu, jakby czekał.");
    first.fmt.drop_cap = Some(3);
    first.fmt.first_line = Some(0.0);
    let p = |t: &str| para("normal", t);
    d.body = Story {
        paras: vec![
            num,
            title,
            orn,
            first,
            p("W sieni pachniało kurzem i jabłkami. Na wieszaku wciąż wisiał płaszcz babci – szary, z mosiężnymi guzikami, których jako dziecko nie wolno jej było dotykać. Teraz dotknęła jednego z nich i poczuła chłód metalu, zupełnie zwyczajny, wcale nie magiczny."),
            p("— Jest tu kto? — zapytała, choć wiedziała, że nikogo nie ma. Głos odbił się od ścian i wrócił do niej cichszy, jakby dom odpowiadał szeptem."),
            p("Kuchnia została dokładnie taka sama. Na stole leżała serweta z haftowanymi makami, a obok cukiernicy – koperta zaadresowana starannym, pochyłym pismem. Na kopercie było tylko jedno słowo: jej imię."),
            p("Marta usiadła. Za oknem deszcz przybierał na sile, krople stukały o parapet coraz szybciej, jakby ktoś niecierpliwie bębnił palcami. Wzięła kopertę do ręki i przez długą chwilę jej nie otwierała."),
            p("Wiedziała, że gdy przeczyta list, nic już nie będzie takie jak przedtem. Wiedziała też, że po to właśnie przyjechała."),
        ],
    };
    let f = CharFmt { size: Some(8.5), color: Some(rust), font: Some("Garamond".into()), ..Default::default() };
    let mut pg = Paragraph { style: "footer".into(), fmt: ParaFmt::default(), runs: vec![run("— ", f.clone()), field(Field::PageNumber, f.clone()), run(" —", f)] };
    pg.fmt.align = Some(Align::Center);
    d.footer = Some(Story { paras: vec![pg] });
    d
}

/// Podręcznik: rozdział z numerem w ukośnym polu, dwie kolumny, przykłady w kolorze, ramki
/// wyróżnień, tabela z formatowanymi komórkami i ramka „Pamiętaj!”.
pub fn textbook() -> Document {
    let mut d = Document::new();
    d.meta.title = "Present Tenses".into();
    d.settings.lang = "en".into();
    crate::styles::apply_theme(&mut d, &crate::styles::THEMES[0]);
    let navy = Color::rgb(0x1f, 0x3a, 0x6e);
    for s in d.styles.iter_mut() {
        match s.id.as_str() {
            "normal" => {
                s.chars.size = Some(9.5);
                s.para.space_after = Some(3.0);
                s.para.line = Some(LineSpacing::Multiple(1.08));
            }
            "heading2" => {
                s.chars.color = Some(navy);
                s.chars.bold = Some(true);
                s.chars.size = Some(12.0);
                s.para.space_before = Some(8.0);
            }
            "list" => s.para.space_after = Some(1.0),
            _ => {}
        }
    }
    d.page.margin_top = 92.0;
    d.page.margin_left = 48.0;
    d.page.margin_right = 40.0;
    d.page.columns = 2;
    d.page.column_gap = 16.0;
    d.page.color = Color::rgb(0xfd, 0xfb, 0xf4);
    let bullet = |t: &str| {
        let mut p = Paragraph { style: "list".into(), fmt: ParaFmt::default(), runs: crate::io::plain::md_inline(t) };
        p.fmt.list = Some(Some(ListFmt { kind: ListKind::Bullet, level: 0, start: None }));
        p
    };
    let example = |t: &str| {
        let mut p = Paragraph { style: "normal".into(), fmt: ParaFmt::default(), runs: crate::io::plain::md_inline(t) };
        p.fmt.indent_left = Some(18.0);
        for r in p.runs.iter_mut() {
            r.fmt.italic = Some(true);
            if r.fmt.color.is_none() {
                r.fmt.color = Some(crate::io::plain::MARK_COLOR);
            }
        }
        p
    };
    let rich = |style: &str, t: &str| Paragraph { style: style.into(), fmt: ParaFmt::default(), runs: crate::io::plain::md_inline(t) };
    let gray = |t: &str| {
        let mut p = rich("normal", t);
        p.fmt.shading = Some(Color::rgb(0xe3, 0xe3, 0xe3));
        p.fmt.border = Some(Some(BoxBorder { width: 0.0, color: Color::NONE, radius: 0.0, padding: 4.0 }));
        p.fmt.space_before = Some(6.0);
        p.fmt.space_after = Some(8.0);
        p
    };
    let mut paras = vec![
        rich("heading2", "Present Simple"),
        rich("normal", "We use the **present simple** for:"),
        bullet("facts and permanent states."),
        example("Heather and Pat **live** in the centre of the city."),
        bullet("general truths and laws of nature."),
        example("The earth goes **round** the sun."),
        bullet("habits and routines (with **always**, **usually**, etc.)."),
        example("Bob always **walks** to school."),
        bullet("timetables and programmes (in the future)."),
        example("Our plane **arrives** at midnight."),
        bullet("feelings and thoughts."),
        example("I **like** James Bond films. They**’re** full of action."),
        gray("**The time expressions we use with the present simple are:** *usually, often, always, every day/week/month/year, etc., in the morning/afternoon/evening, at night/the weekend, on Fridays, etc.*"),
    ];
    let mut pc = rich("heading2", "Present Continuous");
    pc.fmt.break_before = Some(Break::Column);
    paras.push(pc);
    paras.extend([
        rich("normal", "We use the **present continuous** (**to be** + verb **-ing**):"),
        bullet("for actions taking place at or around the moment of speaking."),
        example("Tracy **is making** a lot of noise at the moment."),
        bullet("for temporary situations."),
        example("Ted **is learning** to play the drums."),
        bullet("for fixed arrangements in the near future."),
        example("Jonathan and I **are going** bowling on Saturday."),
        bullet("for currently changing and developing situations."),
        example("More and more people **are exercising** these days."),
        bullet("with adverbs such as **always** to express anger or irritation at a repeated action."),
        example("You**’re** always **forgetting** to tidy your room."),
        gray("**The time expressions we use with the present continuous are:** *now, at the moment, at present, these days, nowadays, still, today, tonight, etc.*"),
    ]);
    paras.push(rich("heading2", "Stative Verbs"));
    paras.push(rich("normal", "Stative verbs are verbs which describe a state rather than an action, and so do not usually have a continuous tense. These verbs are:"));
    paras.push(bullet("verbs of the senses (**see, hear, smell, taste, feel, look, sound, seem, appear**, etc.). ==David **looks** tired.=="));
    paras.push(bullet("verbs of perception (**know, believe, understand, realise, remember, forget**, etc.). ==I **think** History is interesting.=="));
    paras.push(bullet("verbs which express feelings and emotions (**like, love, hate, enjoy, prefer, detest, desire, want**, etc.)."));
    d.body = Story { paras };

    let w = d.page.width;
    // Nagłówek rozdziału: numer w ukośnym polu i tytuł.
    let mut num = Paragraph::with_text("normal", "1");
    num.fmt.align = Some(Align::Center);
    num.runs[0].fmt = CharFmt { size: Some(34.0), bold: Some(true), italic: Some(true), color: Some(Color::WHITE), font: Some("Calibri".into()), ..Default::default() };
    let id = d.new_id();
    d.objects.push(shape_obj(id, 0, (40.0, 22.0, 62.0, 52.0), Shape::polygon(&[(0.2, 0.0), (1.0, 0.0), (0.8, 1.0), (0.0, 1.0)]), frame(Story { paras: vec![num] }, 2.0, VAlign::Center), WrapMode::None, Some(Fill::Solid { color: navy })));
    let mut title = Paragraph::with_text("normal", "Present Tenses");
    title.runs[0].fmt = CharFmt { size: Some(28.0), color: Some(navy), font: Some("Calibri".into()), ..Default::default() };
    let id = d.new_id();
    d.objects.push(shape_obj(id, 0, (100.0, 26.0, 360.0, 44.0), Shape::rect(), frame(Story { paras: vec![title] }, 2.0, VAlign::Center), WrapMode::None, None));
    let id = d.new_id();
    let mut bar = shape_obj(id, 0, (40.0, 76.0, w - 80.0, 3.0), Shape::rect(), ObjKind::Shape, WrapMode::None, Some(Fill::Solid { color: navy }));
    bar.corner = 1.0;
    d.objects.push(bar);

    // Tabela czasowników statycznych pod tekstem.
    let table = Table {
        rows: vec![
            vec!["Verb".into(), "Present Simple".into(), "Present Continuous".into()],
            vec!["**THINK:**".into(), "==I **think** our new Science teacher is very nice.== (= believe)".into(), "==Mum and Dad **are thinking** of buying a new car.== (= are considering)".into()],
            vec!["**HAVE:**".into(), "==He **has** a pet iguana.== (= own, possess)".into(), "==They **are having** fun ice-skating.== (= are experiencing)".into()],
            vec!["**SEE:**".into(), "==I **can see** John and Tina in the park.== (= are visible)".into(), "==I**’m seeing** Patrick outside the cinema tonight.== (= am meeting)".into()],
            vec!["**TASTE:**".into(), "==The meat **tastes** very salty.== (= it is, has the flavour of)".into(), "==Harry **is tasting** the new Chinese dish.== (= is testing)".into()],
            vec!["**LOOK:**".into(), "==Julia **looks** very pleased.== (= appears)".into(), "==Stephen **is looking** at an advert in the newspaper.== (= is taking a look at)".into()],
            vec!["**SMELL:**".into(), "==These flowers **smell** great.== (= have the aroma of)".into(), "==Why **are** you **smelling** the milk?== (= are sniffing)".into()],
            vec!["**APPEAR:**".into(), "==Anna **appears** to be ill.== (= seems to)".into(), "==He **is appearing** at the theatre this week.== (= is performing)".into()],
            vec!["**FIT:**".into(), "==These jeans are comfortable and **fit** me perfectly.== (= are the right size)".into(), "==My dad **is fitting** security locks on the windows.== (= is attaching)".into()],
            vec!["**WEIGH:**".into(), "==The parcel **weighs** two kilos.== (= has the weight of)".into(), "==The shop assistant **is weighing** the apples.== (= is measuring the weight)".into()],
            vec!["**FEEL:**".into(), "==This fabric **feels** soft.== (= has the texture of)".into(), "==I**’m feeling** much better today.== (= am experiencing)".into()],
        ],
        header: true,
        stripes: false,
        font: "Calibri".into(),
        size: 9.0,
        accent: navy,
        line: Color::rgb(0x60, 0x70, 0x90),
        numbers_right: false,
        widths: Vec::new(),
        take: None,
    };
    let tw = w - d.page.margin_left - d.page.margin_right;
    let th = crate::paint::table_height(&table, tw);
    let id = d.new_id();
    let table_id = id;
    d.objects.push(shape_obj(id, 0, (d.page.margin_left, 560.0, tw, th), Shape::rect(), ObjKind::Table { table }, WrapMode::TopBottom, Some(Fill::Solid { color: Color::WHITE })));

    // Ramka „Remember!” z etykietą.
    let mut rem = Paragraph { style: "normal".into(), fmt: ParaFmt::default(), runs: crate::io::plain::md_inline("The verb **enjoy** can be used in continuous tenses to express a specific preference. ==My brother really **enjoys** skateboarding.== (general preference) **BUT** ==They **are really enjoying** the concert.== (specific preference)") };
    rem.fmt.list = Some(Some(ListFmt { kind: ListKind::Bullet, level: 0, start: None }));
    let ry = 560.0 + th + 26.0;
    let id = d.new_id();
    let box_id = id;
    let mut boxo = shape_obj(id, 0, (d.page.margin_left, ry, tw, 70.0), Shape::rect(), frame(Story { paras: vec![rem] }, 12.0, VAlign::Center), WrapMode::TopBottom, Some(Fill::Solid { color: Color::WHITE }));
    boxo.corner = 12.0;
    boxo.stroke = Some(StrokeStyle { color: navy, width: 1.5, dash: vec![] });
    d.objects.push(boxo);
    let mut lab = Paragraph::with_text("normal", "Remember!");
    lab.fmt.align = Some(Align::Center);
    lab.runs[0].fmt = CharFmt { italic: Some(true), bold: Some(true), color: Some(navy), size: Some(9.0), ..Default::default() };
    let id = d.new_id();
    let mut tag = shape_obj(id, 0, (d.page.margin_left + 8.0, ry - 10.0, 76.0, 20.0), Shape::rect(), frame(Story { paras: vec![lab] }, 2.0, VAlign::Center), WrapMode::None, Some(Fill::Solid { color: Color::rgb(0xfd, 0xfb, 0xf4) }));
    tag.corner = 10.0;
    tag.stroke = Some(StrokeStyle { color: navy, width: 1.2, dash: vec![] });
    d.objects.push(tag);
    // Tabela płynie z tekstem: kotwica w pustym akapicie na końcu treści.
    d.body.paras.push(Paragraph::default());
    let last = d.body.paras.len() - 1;
    d.body.paras[last].runs = vec![Run { text: FIELD_CHAR.to_string(), field: Some(Field::Anchor(table_id)), ..Default::default() }];
    // Ramka „Remember!” za tabelą (etykieta zostaje przy ramce – oba obiekty w jednym akapicie).
    let mut after = Paragraph::default();
    let tag_id = d.objects.last().map(|o| o.id).unwrap_or(0);
    after.runs = vec![
        Run { text: FIELD_CHAR.to_string(), field: Some(Field::Anchor(box_id)), ..Default::default() },
        Run { text: FIELD_CHAR.to_string(), field: Some(Field::Anchor(tag_id)), ..Default::default() },
    ];
    d.body.paras.push(after);
    if let Some(b) = d.object_mut(box_id) {
        b.anchor_dy = 18.0;
    }
    if let Some(t) = d.object_mut(tag_id) {
        t.anchor_dy = 8.0;
        t.wrap.mode = WrapMode::None;
    }
    d.page.border = Some(StrokeStyle { color: navy, width: 1.0, dash: vec![] });
    d.page.border_inset = 30.0;
    d
}
