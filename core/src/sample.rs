//! Dokument powitalny – pokazuje możliwości programu (kształty tekstu, opływanie, kolumny,
//! wykres, tekst na ścieżce) i służy testom składu.

use crate::geom;
use crate::model::*;

const LOREM: &str = "Typografia to sztuka układania tekstu tak, aby czytało się go lekko i przyjemnie. OpenDTP składa tekst w dowolnych kształtach: ramka może być kołem, falą, gwiazdą albo własnym konturem z węzłów i krzywych, a treść płynie wewnątrz niej i opływa obrazy, wykresy oraz inne kształty z ustawionym odstępem. Wiersze same omijają przeszkody, a gdy w środku kształtu jest otwór, tekst przeskakuje na jego drugą stronę.";

pub fn welcome() -> Document {
    let mut d = Document::new();
    d.meta.title = "Witaj w OpenDTP".into();
    let p = |style: &str, t: &str| Paragraph::with_text(style, t);
    let mut paras = vec![p("title", "Witaj w OpenDTP"), p("subtitle", "Edytor tekstu i skład dokumentów"), p("heading1", "Tekst, który płynie")];
    for _ in 0..2 {
        paras.push(p("normal", LOREM));
    }
    let mut q = p("quote", "„Dobry układ strony jest niewidoczny – widać tylko treść.”");
    q.fmt.space_before = Some(6.0);
    paras.push(q);
    paras.push(p("heading2", "Listy i wyróżnienia"));
    for t in ["Kształty tekstu z węzłami i krzywymi", "Opływanie konturem, ramką albo z jednej strony", "Kolumny, łańcuchy ramek i tekst na ścieżce"] {
        let mut lp = p("list", t);
        lp.fmt.list = Some(Some(ListFmt { kind: ListKind::Bullet, level: 0, start: None }));
        paras.push(lp);
    }
    let mut c = p("callout", "Wskazówka: kliknij dwukrotnie ramkę, aby edytować jej węzły. Shift dodaje węzeł, Alt zmienia odcinek w krzywą.");
    c.runs[0].fmt.bold = None;
    paras.push(c);
    for _ in 0..3 {
        paras.push(p("normal", LOREM));
    }
    d.body = Story { paras };
    let star = Object {
        id: d.new_id(),
        name: "Kształt tekstu".into(),
        kind: ObjKind::Text {
            story: Story { paras: vec![{
                let mut sp = p("normal", "Tekst wpisany w gwiazdę wypełnia jej wnętrze wiersz po wierszu, a cały kształt opływa treść strony. Węzły można przesuwać i zmieniać w krzywe.");
                sp.fmt.align = Some(Align::Center);
                sp.runs[0].fmt.color = Some(Color::rgb(0x12, 0x3a, 0x66));
                sp.runs[0].fmt.size = Some(10.0);
                sp
            }] },
            padding: 4.0,
            columns: 1,
            gap: 12.0,
            valign: VAlign::Top,
            next: None,
        },
        page: 0,
        x: 330.0,
        y: 190.0,
        w: 190.0,
        h: 190.0,
        shape: geom::star(5, 0.55),
        wrap: Wrap { mode: WrapMode::Both, dist: 10.0, contour: true },
        fill: Some(Fill::Radial { from: Color::rgb(0xff, 0xf3, 0xc4), to: Color::rgb(0xff, 0xd8, 0x6b) }),
        stroke: Some(StrokeStyle { color: Color::rgb(0xd9, 0x9a, 0x00), width: 1.0, dash: vec![] }),
        opacity: 1.0,
        corner: 0.0,
        shadow: true,
        locked: false,
    };
    d.objects.push(star);
    let chart = Object {
        id: d.new_id(),
        name: "Wykres".into(),
        kind: ObjKind::Chart { chart: Chart { title: "Sprzedaż".into(), ..Chart::default() } },
        page: 1,
        x: 72.0,
        y: 300.0,
        w: 240.0,
        h: 170.0,
        shape: Shape::rect(),
        wrap: Wrap { mode: WrapMode::Right, dist: 12.0, contour: false },
        fill: Some(Fill::Solid { color: Color::rgb(0xfa, 0xfa, 0xfa) }),
        stroke: Some(StrokeStyle { color: Color::rgb(0xdd, 0xdd, 0xdd), width: 0.5, dash: vec![] }),
        opacity: 1.0,
        corner: 6.0,
        shadow: false,
        locked: false,
    };
    d.objects.push(chart);
    let arc = Object {
        id: d.new_id(),
        name: "Tekst na łuku".into(),
        kind: ObjKind::PathText { story: Story { paras: vec![{
            let mut a = p("normal", "TEKST BIEGNĄCY PO KRZYWEJ");
            a.fmt.align = Some(Align::Center);
            a.runs[0].fmt.size = Some(16.0);
            a.runs[0].fmt.tracking = Some(2.0);
            a.runs[0].fmt.color = Some(Color::rgb(0x8a, 0x1c, 0x1c));
            a
        }] }, offset: 0.0 },
        page: 1,
        x: 330.0,
        y: 520.0,
        w: 200.0,
        h: 90.0,
        shape: geom::arc_path(),
        wrap: Wrap { mode: WrapMode::Both, dist: 6.0, contour: false },
        fill: None,
        stroke: None,
        opacity: 1.0,
        corner: 0.0,
        shadow: false,
        locked: false,
    };
    d.objects.push(arc);
    d.footer = Some(Story { paras: vec![Paragraph {
        style: "footer".into(),
        fmt: ParaFmt::default(),
        runs: vec![
            Run { text: "Strona ".into(), ..Default::default() },
            Run { text: FIELD_CHAR.to_string(), field: Some(Field::PageNumber), ..Default::default() },
            Run { text: " z ".into(), ..Default::default() },
            Run { text: FIELD_CHAR.to_string(), field: Some(Field::PageCount), ..Default::default() },
        ],
    }] });
    d
}

fn para(style: &str, t: &str) -> Paragraph {
    Paragraph::with_text(style, t)
}

fn footer_numbers() -> Story {
    Story { paras: vec![Paragraph {
        style: "footer".into(),
        fmt: ParaFmt::default(),
        runs: vec![Run { text: FIELD_CHAR.to_string(), field: Some(Field::PageNumber), ..Default::default() }],
    }] }
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
    }
}

fn frame(story: Story, padding: f32, valign: VAlign) -> ObjKind {
    ObjKind::Text { story, padding, columns: 1, gap: 12.0, valign, next: None }
}

/// Dokument z szablonu (karta „Nowy” w menu Plik).
pub fn template(id: &str) -> Document {
    match id {
        "welcome" => welcome(),
        "textbook" => textbook(),
        "report" => {
            let mut d = Document::new();
            d.meta.title = "Raport".into();
            let mut t = para("title", "Tytuł raportu");
            t.fmt.space_before = Some(160.0);
            let mut paras = vec![t, para("subtitle", "Podtytuł · autor · data")];
            let mut toc = para("heading1", "Spis treści");
            toc.fmt.break_before = Some(Break::Page);
            paras.push(toc);
            paras.push(para("toc1", "Wprowadzenie\t3"));
            paras.push(para("toc1", "Wyniki\t3"));
            paras.push(para("toc1", "Podsumowanie\t3"));
            let mut h = para("heading1", "Wprowadzenie");
            h.fmt.break_before = Some(Break::Page);
            paras.push(h);
            paras.push(para("normal", "Opisz cel i zakres dokumentu. Kliknij dowolny tekst szablonu, aby go zastąpić własnym."));
            paras.push(para("heading1", "Wyniki"));
            paras.push(para("normal", "Najważniejsze ustalenia przedstaw w punktach albo na wykresie (Wstawianie, Wykres)."));
            paras.push(para("heading2", "Szczegóły"));
            paras.push(para("normal", "Tekst raportu…"));
            paras.push(para("heading1", "Podsumowanie"));
            paras.push(para("callout", "Najważniejszy wniosek w wyróżnionej ramce."));
            d.body = Story { paras };
            d.footer = Some(footer_numbers());
            d.page.different_first = true;
            d.footer_first = Some(Story::default());
            d
        }
        "letter" => {
            let mut d = Document::new();
            d.meta.title = "List".into();
            let mut right = para("normal", "Miejscowość, dnia …");
            right.fmt.align = Some(Align::Right);
            let mut paras = vec![para("normal", "Imię i nazwisko nadawcy\nUlica i numer\nKod pocztowy, miejscowość"), right];
            let mut addr = para("normal", "Adresat\nUlica i numer\nKod pocztowy, miejscowość");
            addr.fmt.indent_left = Some(260.0);
            addr.fmt.space_before = Some(24.0);
            paras.push(addr);
            let mut subj = para("normal", "Dotyczy: …");
            subj.runs[0].fmt.bold = Some(true);
            subj.fmt.space_before = Some(24.0);
            paras.push(subj);
            paras.push(para("normal", "Szanowni Państwo,"));
            let mut body = para("normal", "treść listu…");
            body.fmt.align = Some(Align::Justify);
            body.fmt.first_line = Some(18.0);
            paras.push(body);
            let mut sign = para("normal", "Z poważaniem\n\n…");
            sign.fmt.indent_left = Some(300.0);
            sign.fmt.space_before = Some(24.0);
            paras.push(sign);
            d.body = Story { paras };
            d
        }
        "newsletter" => {
            let mut d = Document::new();
            d.meta.title = "Gazetka".into();
            d.page.columns = 3;
            d.page.column_rule = true;
            d.page.column_gap = 14.0;
            d.page.margin_top = 120.0;
            crate::styles::apply_theme(&mut d, &crate::styles::THEMES[1]);
            let lorem = "Skład w trzech kolumnach z linią pomiędzy nimi. Tekst płynie z kolumny do kolumny i ze strony na stronę, opływając obrazy i kształty. Zastąp ten tekst własnym artykułem.";
            let mut paras = Vec::new();
            for h in ["Wiadomości", "Wydarzenia", "Ludzie", "Kalendarz"] {
                paras.push(para("heading2", h));
                for _ in 0..3 {
                    let mut p = para("normal", lorem);
                    p.fmt.align = Some(Align::Justify);
                    paras.push(p);
                }
            }
            if let Some(f) = paras.get_mut(1) {
                f.fmt.drop_cap = Some(3);
            }
            d.body = Story { paras };
            let mut title = para("title", "GAZETKA");
            title.runs[0].fmt.tracking = Some(6.0);
            title.runs[0].fmt.color = Some(Color::WHITE);
            title.fmt.align = Some(Align::Center);
            let id = d.new_id();
            let w = d.page.width;
            d.objects.push(shape_obj(
                id,
                0,
                (0.0, 0.0, w, 100.0),
                Shape::rect(),
                frame(Story { paras: vec![title] }, 22.0, VAlign::Center),
                WrapMode::TopBottom,
                Some(Fill::Linear { from: Color::rgb(0x8a, 0x1c, 0x1c), to: Color::rgb(0x3a, 0x0c, 0x0c), angle: 0.0 }),
            ));
            let id = d.new_id();
            d.objects.push(shape_obj(id, 0, (230.0, 300.0, 140.0, 140.0), geom::ellipse(), ObjKind::Shape, WrapMode::Both, Some(Fill::Solid { color: Color::rgb(0xf2, 0xc1, 0x4e) })));
            d.footer = Some(footer_numbers());
            d
        }
        "flyer" => {
            let mut d = Document::new();
            d.meta.title = "Ulotka".into();
            d.page.color = Color::rgb(0x0e, 0x0e, 0x12);
            let w = d.page.width;
            let mut big = para("title", "WIELKIE\nOTWARCIE");
            big.runs[0].fmt.color = Some(Color::rgb(0xff, 0xd6, 0x0a));
            big.runs[0].fmt.size = Some(54.0);
            big.runs[0].fmt.bold = Some(true);
            big.fmt.align = Some(Align::Center);
            big.fmt.line = Some(LineSpacing::Multiple(0.9));
            let id = d.new_id();
            d.objects.push(shape_obj(id, 0, (60.0, 70.0, w - 120.0, 160.0), Shape::rect(), frame(Story { paras: vec![big] }, 4.0, VAlign::Center), WrapMode::None, None));
            let mut inner = para("normal", "Tekst wpisany w koło – zmień kształt węzłami albo wybierz inny z karty Kształty tekstu. Wiersze dopasowują się do konturu.");
            inner.fmt.align = Some(Align::Center);
            inner.runs[0].fmt.size = Some(13.0);
            inner.runs[0].fmt.color = Some(Color::rgb(0x10, 0x10, 0x10));
            let id = d.new_id();
            d.objects.push(shape_obj(id, 0, (120.0, 280.0, 355.0, 355.0), geom::ellipse(), frame(Story { paras: vec![inner] }, 30.0, VAlign::Top), WrapMode::Both, Some(Fill::Solid { color: Color::rgb(0xff, 0xd6, 0x0a) })));
            let mut when = para("normal", "SOBOTA · 12:00 · RYNEK");
            when.fmt.align = Some(Align::Center);
            when.runs[0].fmt.color = Some(Color::WHITE);
            when.runs[0].fmt.size = Some(20.0);
            when.runs[0].fmt.tracking = Some(3.0);
            let id = d.new_id();
            d.objects.push(shape_obj(id, 0, (60.0, 690.0, w - 120.0, 60.0), Shape::rect(), frame(Story { paras: vec![when] }, 4.0, VAlign::Center), WrapMode::None, None));
            d
        }
        _ => Document::new(),
    }
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
        ],
        header: true,
        stripes: false,
        font: "Calibri".into(),
        size: 9.0,
        accent: navy,
        line: Color::rgb(0x60, 0x70, 0x90),
        numbers_right: false,
    };
    let tw = w - d.page.margin_left - d.page.margin_right;
    let th = crate::paint::table_height(&table, tw);
    let id = d.new_id();
    d.objects.push(shape_obj(id, 0, (d.page.margin_left, 560.0, tw, th), Shape::rect(), ObjKind::Table { table }, WrapMode::TopBottom, Some(Fill::Solid { color: Color::WHITE })));

    // Ramka „Remember!” z etykietą.
    let mut rem = Paragraph { style: "normal".into(), fmt: ParaFmt::default(), runs: crate::io::plain::md_inline("The verb **enjoy** can be used in continuous tenses to express a specific preference. ==My brother really **enjoys** skateboarding.== (general preference) **BUT** ==They **are really enjoying** the concert.== (specific preference)") };
    rem.fmt.list = Some(Some(ListFmt { kind: ListKind::Bullet, level: 0, start: None }));
    let ry = 560.0 + th + 26.0;
    let id = d.new_id();
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
    d.page.border = Some(StrokeStyle { color: navy, width: 1.0, dash: vec![] });
    d.page.border_inset = 30.0;
    d
}
