//! Generowanie dokumentów przez modele językowe: wielu dostawców (OpenRouter, OpenAI, Anthropic,
//! Google Gemini, Mistral, Groq, DeepSeek, xAI, Ollama, własny adres zgodny z OpenAI), lista modeli
//! pobierana od dostawcy, a odpowiedź – opis dokumentu w JSON – zamieniana na sformatowany dokument
//! (style, listy, wyróżnienia, kolumny, wykresy, motyw).

use serde_json::{json, Value};

use crate::model::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Api {
    /// Zgodne z OpenAI `/chat/completions`.
    OpenAi,
    Anthropic,
    Gemini,
}

pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub base: &'static str,
    pub api: Api,
    pub needs_key: bool,
    pub fallback: &'static [&'static str],
}

pub const PROVIDERS: &[Provider] = &[
    Provider { id: "openrouter", name: "OpenRouter", base: "https://openrouter.ai/api/v1", api: Api::OpenAi, needs_key: true, fallback: &["openrouter/auto", "anthropic/claude-sonnet-5.5", "openai/gpt-4o", "google/gemini-2.5-pro", "meta-llama/llama-3.3-70b-instruct"] },
    Provider { id: "openai", name: "OpenAI", base: "https://api.openai.com/v1", api: Api::OpenAi, needs_key: true, fallback: &["gpt-4o", "gpt-4o-mini", "gpt-4.1", "gpt-4.1-mini"] },
    Provider { id: "anthropic", name: "Anthropic", base: "https://api.anthropic.com/v1", api: Api::Anthropic, needs_key: true, fallback: &["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"] },
    Provider { id: "gemini", name: "Google Gemini", base: "https://generativelanguage.googleapis.com/v1beta", api: Api::Gemini, needs_key: true, fallback: &["gemini-2.5-pro", "gemini-2.5-flash"] },
    Provider { id: "mistral", name: "Mistral", base: "https://api.mistral.ai/v1", api: Api::OpenAi, needs_key: true, fallback: &["mistral-large-latest", "mistral-small-latest"] },
    Provider { id: "groq", name: "Groq", base: "https://api.groq.com/openai/v1", api: Api::OpenAi, needs_key: true, fallback: &["llama-3.3-70b-versatile"] },
    Provider { id: "deepseek", name: "DeepSeek", base: "https://api.deepseek.com/v1", api: Api::OpenAi, needs_key: true, fallback: &["deepseek-chat", "deepseek-reasoner"] },
    Provider { id: "xai", name: "xAI (Grok)", base: "https://api.x.ai/v1", api: Api::OpenAi, needs_key: true, fallback: &["grok-3", "grok-3-mini"] },
    Provider { id: "ollama", name: "Ollama (lokalnie)", base: "http://localhost:11434/v1", api: Api::OpenAi, needs_key: false, fallback: &["llama3.1", "qwen2.5", "mistral"] },
    Provider { id: "custom", name: "Własny (zgodny z OpenAI)", base: "", api: Api::OpenAi, needs_key: false, fallback: &[] },
];

pub fn provider(id: &str) -> &'static Provider {
    PROVIDERS.iter().find(|p| p.id == id).unwrap_or(&PROVIDERS[0])
}

pub struct Config {
    pub provider: String,
    pub key: String,
    pub model: String,
    pub temperature: f32,
    /// Własny adres API (dla „custom” albo innego serwera Ollama).
    pub base: String,
}

impl Config {
    fn base(&self) -> String {
        let p = provider(&self.provider);
        let b = if self.base.trim().is_empty() { p.base.to_string() } else { self.base.trim().to_string() };
        b.trim_end_matches('/').to_string()
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_read(std::time::Duration::from_secs(300)).timeout_connect(std::time::Duration::from_secs(20)).build()
}

fn err(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, r) => {
            let body = r.into_string().unwrap_or_default();
            let msg = serde_json::from_str::<Value>(&body).ok().and_then(|v| v["error"]["message"].as_str().or(v["error"].as_str()).or(v["message"].as_str()).map(str::to_string)).unwrap_or(body);
            format!("HTTP {code}: {}", msg.chars().take(300).collect::<String>())
        }
        e => e.to_string(),
    }
}

/// Modele dostępne u dostawcy (posortowane); przy błędzie – lista zapasowa.
pub fn list_models(cfg: &Config) -> Result<Vec<String>, String> {
    let p = provider(&cfg.provider);
    let base = cfg.base();
    if base.is_empty() {
        return Err(crate::lang::tr("Podaj adres API").into());
    }
    let a = agent();
    let v: Value = match p.api {
        Api::OpenAi => {
            let mut r = a.get(&format!("{base}/models"));
            if !cfg.key.is_empty() {
                r = r.set("Authorization", &format!("Bearer {}", cfg.key));
            }
            r.call().map_err(err)?.into_json().map_err(|e| e.to_string())?
        }
        Api::Anthropic => a.get(&format!("{base}/models?limit=100")).set("x-api-key", &cfg.key).set("anthropic-version", "2023-06-01").call().map_err(err)?.into_json().map_err(|e| e.to_string())?,
        Api::Gemini => a.get(&format!("{base}/models?key={}", cfg.key)).call().map_err(err)?.into_json().map_err(|e| e.to_string())?,
    };
    let mut out: Vec<String> = match p.api {
        Api::Gemini => v["models"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|m| m["supportedGenerationMethods"].as_array().is_none_or(|s| s.iter().any(|x| x == "generateContent")))
                    .filter_map(|m| m["name"].as_str().map(|n| n.trim_start_matches("models/").to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        _ => v["data"].as_array().map(|a| a.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect()).unwrap_or_default(),
    };
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Ok(p.fallback.iter().map(|s| s.to_string()).collect());
    }
    Ok(out)
}

/// Instrukcja dla modelu. Lista możliwości jest budowana z faktycznych list programu (style,
/// motywy, kształty), więc zawsze odpowiada temu, co OpenDTP potrafi.
pub fn system() -> String {
    let themes: Vec<String> = crate::styles::THEMES.iter().map(|t| format!("{} ({}: {} / {})", t.id, t.name, t.heading, t.body)).collect();
    let styles: Vec<String> = crate::styles::defaults().iter().map(|s| format!("{} ({})", s.id, s.name)).collect();
    let shapes = crate::geom::PRESETS.join("|");
    format!(
        r##"Jesteś redaktorem i składaczem dokumentów w programie OpenDTP – edytorze tekstu i programie do składu (DTP) dla Windows. Na prośbę użytkownika przygotowujesz kompletny, merytoryczny, świetnie napisany i efektownie złożony dokument. Zwracasz WYŁĄCZNIE obiekt JSON (bez komentarzy i bez bloku ```).

MOŻLIWOŚCI OPENDTP, z których korzystasz:
- Strony A4, A5 albo Letter, pionowo lub poziomo, kolor tła strony, 1–4 kolumny treści z linią między nimi.
- Motywy dokumentu (kroje i kolory wszystkich stylów): {themes}.
- Style akapitów: {styles}. Nagłówki tworzą konspekt i spis treści z numerami stron.
- Akapity: wyrównanie, cieniowanie tła, kolor i rozmiar tekstu, inicjał (duża pierwsza litera na 3 wiersze).
- Wyróżnienia w tekście: **pogrubienie**, *kursywa*, ==wyróżnienie kolorem== (np. zdania przykładowe), [łącze](https://adres) – działają też w komórkach tabel i w punktach list.
- Listy punktowane i numerowane.
- Tabele z nagłówkiem w kolorze motywu, pasami i obramowaniem; komórki mają formatowany tekst i zawijają się, wysokość wierszy dopasowuje się do treści.
- Wykresy: column, bar, line, area, pie, doughnut – z tytułem, etykietami i seriami danych.
- Kształty tekstu: ramka o dowolnym kształcie ({shapes}); tekst wypełnia jej wnętrze, a treść strony opływa kształt. Kolor wypełnienia i obrysu.
- Ramki tekstowe (np. ramka boczna, ramka z kolumnami) przy lewej albo prawej krawędzi lub na całą szerokość; treść opływa je.
- Tekst na ścieżce: krótki napis biegnący po łuku (arc) albo fali (wave).
- Nagłówek i stopka strony, numery stron.

SCHEMAT ODPOWIEDZI:
{{
  "title": "tytuł", "subtitle": "podtytuł albo pusty",
  "theme": "jeden z identyfikatorów motywów",
  "page": "A4|A5|Letter", "landscape": false, "page_color": "#ffffff", "columns": 1,
  "toc": false, "header": "tekst nagłówka strony albo pusty", "footer_page_numbers": true,
  "blocks": [
    {{"type": "heading1|heading2|heading3", "text": "..."}},
    {{"type": "paragraph", "text": "...", "align": "left|center|right|justify", "drop_cap": false, "shading": "#rrggbb albo pusty", "color": "#rrggbb albo pusty", "size": 0}},
    {{"type": "bullets|numbered", "items": ["...", "..."]}},
    {{"type": "quote|callout|caption", "text": "..."}},
    {{"type": "code", "text": "..."}},
    {{"type": "table", "rows": [["nagłówek 1", "nagłówek 2"], ["a", "b"]]}},
    {{"type": "chart", "kind": "column|bar|line|area|pie|doughnut", "title": "...", "labels": ["..."], "series": [{{"name": "...", "values": [1, 2]}}]}},
    {{"type": "shape_text", "shape": "jeden z kształtów", "text": "krótki tekst", "fill": "#rrggbb", "side": "left|right"}},
    {{"type": "frame", "text": "tekst ramki (akapity rozdziel znakiem nowego wiersza)", "side": "left|right|full", "fill": "#rrggbb", "columns": 1}},
    {{"type": "path_text", "text": "krótki napis", "path": "arc|wave"}},
    {{"type": "pagebreak"}}
  ]
}}

ZASADY: Pisz w języku prośby (domyślnie po polsku, z polską typografią: „cudzysłowy”, półpauzy –). Dobieraj motyw, kolumny i elementy do charakteru dokumentu: raport – spis treści, nagłówki, wykresy, wyróżnienia; ulotka lub plakat – kolor strony, duże shape_text, path_text, mało tekstu; gazetka – 2–3 kolumny, inicjały, ramki; list – zwykłe akapity. Kolory dobieraj harmonijnie do motywu i dbaj o kontrast tekstu z tłem. Wykresy tylko z sensownymi danymi. Nie wymyślaj faktów, których nie znasz – miejsca do uzupełnienia oznacz [w nawiasach kwadratowych]."##,
        themes = themes.join("; "),
        styles = styles.join(", "),
        shapes = shapes
    )
}

/// Wysyła prośbę do modelu i zwraca tekst odpowiedzi.
pub fn complete(cfg: &Config, user: &str) -> Result<String, String> {
    let p = provider(&cfg.provider);
    let base = cfg.base();
    if base.is_empty() {
        return Err(crate::lang::tr("Podaj adres API").into());
    }
    if p.needs_key && cfg.key.trim().is_empty() {
        return Err(crate::lang::tr("Brak klucza API – wpisz go w Ustawieniach → Sztuczna inteligencja").into());
    }
    if cfg.model.trim().is_empty() {
        return Err(crate::lang::tr("Wybierz model w Ustawieniach").into());
    }
    let a = agent();
    let t = cfg.temperature.clamp(0.0, 2.0);
    match p.api {
        Api::OpenAi => {
            let mut body = json!({
                "model": cfg.model,
                "temperature": t,
                "messages": [{"role": "system", "content": system()}, {"role": "user", "content": user}],
            });
            if matches!(p.id, "openai" | "openrouter" | "mistral" | "groq" | "deepseek") {
                body["response_format"] = json!({"type": "json_object"});
            }
            let mut r = a.post(&format!("{base}/chat/completions")).set("Content-Type", "application/json");
            if !cfg.key.is_empty() {
                r = r.set("Authorization", &format!("Bearer {}", cfg.key));
            }
            if p.id == "openrouter" {
                r = r.set("HTTP-Referer", "https://redzikowski.com").set("X-Title", "OpenDTP");
            }
            let mut v: Value = match r.send_json(body.clone()) {
                Ok(resp) => resp.into_json().map_err(|e| e.to_string())?,
                // Część modeli nie zna response_format – ponawiamy bez niego.
                Err(ureq::Error::Status(400, _)) if body.get("response_format").is_some() => {
                    body.as_object_mut().unwrap().remove("response_format");
                    let mut r = a.post(&format!("{base}/chat/completions")).set("Content-Type", "application/json");
                    if !cfg.key.is_empty() {
                        r = r.set("Authorization", &format!("Bearer {}", cfg.key));
                    }
                    r.send_json(body).map_err(err)?.into_json().map_err(|e| e.to_string())?
                }
                Err(e) => return Err(err(e)),
            };
            Ok(v["choices"][0]["message"]["content"].take().as_str().unwrap_or_default().to_string())
        }
        Api::Anthropic => {
            let body = json!({
                "model": cfg.model,
                "max_tokens": 16000,
                "temperature": t.min(1.0),
                "system": system(),
                "messages": [{"role": "user", "content": user}],
            });
            let v: Value = a.post(&format!("{base}/messages")).set("x-api-key", &cfg.key).set("anthropic-version", "2023-06-01").send_json(body).map_err(err)?.into_json().map_err(|e| e.to_string())?;
            Ok(v["content"].as_array().map(|c| c.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("")).unwrap_or_default())
        }
        Api::Gemini => {
            let body = json!({
                "systemInstruction": {"parts": [{"text": system()}]},
                "contents": [{"role": "user", "parts": [{"text": user}]}],
                "generationConfig": {"temperature": t, "responseMimeType": "application/json"},
            });
            let v: Value = a.post(&format!("{base}/models/{}:generateContent?key={}", cfg.model, cfg.key)).send_json(body).map_err(err)?.into_json().map_err(|e| e.to_string())?;
            Ok(v["candidates"][0]["content"]["parts"].as_array().map(|p| p.iter().filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join("")).unwrap_or_default())
        }
    }
}

/// Wycina obiekt JSON z odpowiedzi (modele czasem dodają ``` albo zdanie wstępu).
fn extract_json(s: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(s.trim()) {
        return Some(v);
    }
    let a = s.find('{')?;
    let b = s.rfind('}')?;
    serde_json::from_str(&s[a..=b]).ok()
}

fn runs(text: &str) -> Vec<Run> {
    crate::io::plain::md_inline(text)
}

fn para(style: &str, text: &str) -> Paragraph {
    Paragraph { style: style.into(), fmt: ParaFmt::default(), runs: runs(text) }
}

fn obj(id: u64, page: usize, x: f32, y: f32, w: f32, h: f32, shape: Shape, kind: ObjKind, wrap: WrapMode) -> Object {
    Object { id, name: String::new(), kind, page, x, y, w, h, shape, wrap: Wrap { mode: wrap, dist: 10.0, contour: true }, fill: None, stroke: None, opacity: 1.0, corner: 0.0, shadow: false, locked: false }
}

struct Extra {
    at: usize,
    kind: String,
    shape: String,
    text: String,
    fill: Option<Color>,
    side: String,
    columns: u8,
}

/// Dokument z odpowiedzi modelu. Bez poprawnego JSON – tekst jako Markdown.
pub fn build(answer: &str, template: &Document) -> Document {
    let Some(v) = extract_json(answer) else {
        return crate::io::plain::from_markdown(answer);
    };
    let mut d = template.clone();
    d.objects.clear();
    d.assets.clear();
    let s = |k: &str| v[k].as_str().unwrap_or("").trim().to_string();
    if let Some(t) = crate::styles::THEMES.iter().find(|t| t.id == s("theme")) {
        crate::styles::apply_theme(&mut d, t);
    }
    match s("page").as_str() {
        "A5" => {
            d.page.width = 148.0 * MM;
            d.page.height = 210.0 * MM;
        }
        "Letter" => {
            d.page.width = 215.9 * MM;
            d.page.height = 279.4 * MM;
        }
        _ => {}
    }
    if v["landscape"].as_bool() == Some(true) {
        std::mem::swap(&mut d.page.width, &mut d.page.height);
    }
    if let Some(c) = Color::parse(&s("page_color")) {
        d.page.color = c;
    }
    let cols = v["columns"].as_u64().unwrap_or(1).clamp(1, 4) as u8;
    d.page.columns = cols;
    d.page.column_rule = cols > 1;
    d.meta.title = s("title");
    let mut paras = Vec::new();
    if !s("title").is_empty() {
        paras.push(para("title", &s("title")));
    }
    if !s("subtitle").is_empty() {
        paras.push(para("subtitle", &s("subtitle")));
    }
    let toc_at = if v["toc"].as_bool() == Some(true) { Some(paras.len()) } else { None };
    let mut charts: Vec<(usize, Chart)> = Vec::new();
    let mut extras: Vec<Extra> = Vec::new();
    let mut tables: Vec<(usize, Vec<Vec<String>>)> = Vec::new();
    let mut page_break = false;
    for b in v["blocks"].as_array().cloned().unwrap_or_default() {
        let typ = b["type"].as_str().unwrap_or("paragraph");
        let text = b["text"].as_str().unwrap_or("").to_string();
        let mut add: Vec<Paragraph> = match typ {
            "heading1" | "h1" => vec![para("heading1", &text)],
            "heading2" | "h2" => vec![para("heading2", &text)],
            "heading3" | "h3" => vec![para("heading3", &text)],
            "quote" => vec![para("quote", &text)],
            "caption" => vec![para("caption", &text)],
            "callout" => vec![para("callout", &text)],
            "code" => text.lines().map(|l| Paragraph::with_text("code", l)).collect(),
            "bullets" | "numbered" => {
                let kind = if typ == "numbered" { ListKind::Number } else { ListKind::Bullet };
                b["items"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|i| i.as_str())
                            .map(|i| {
                                let mut p = para("list", i);
                                p.fmt.list = Some(Some(ListFmt { kind, level: 0, start: None }));
                                p
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
            "table" => {
                let rows: Vec<Vec<String>> = b["rows"].as_array().map(|rows| rows.iter().map(|r| r.as_array().map(|c| c.iter().map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).collect()).unwrap_or_default()).collect()).unwrap_or_default();
                if !rows.is_empty() {
                    tables.push((paras.len(), rows));
                }
                Vec::new()
            }
            "chart" => {
                let kind = match b["kind"].as_str().unwrap_or("column") {
                    "bar" => ChartKind::Bar,
                    "line" => ChartKind::Line,
                    "area" => ChartKind::Area,
                    "pie" => ChartKind::Pie,
                    "doughnut" => ChartKind::Doughnut,
                    _ => ChartKind::Column,
                };
                let labels: Vec<String> = b["labels"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
                let series: Vec<Series> = b["series"]
                    .as_array()
                    .map(|a| a.iter().map(|s| Series { name: s["name"].as_str().unwrap_or("").into(), values: s["values"].as_array().map(|v| v.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()).unwrap_or_default(), color: None }).collect())
                    .unwrap_or_default();
                if !labels.is_empty() && !series.is_empty() {
                    charts.push((paras.len(), Chart { kind, title: b["title"].as_str().unwrap_or("").into(), labels, series, ..Chart::default() }));
                }
                Vec::new()
            }
            "shape_text" | "frame" | "path_text" => {
                extras.push(Extra {
                    at: paras.len(),
                    kind: typ.to_string(),
                    shape: b["shape"].as_str().or(b["path"].as_str()).unwrap_or("").into(),
                    text,
                    fill: b["fill"].as_str().and_then(Color::parse),
                    side: b["side"].as_str().unwrap_or("").into(),
                    columns: b["columns"].as_u64().unwrap_or(1).clamp(1, 4) as u8,
                });
                Vec::new()
            }
            "pagebreak" => {
                page_break = true;
                Vec::new()
            }
            _ => {
                let mut p = para("normal", &text);
                p.fmt.align = match b["align"].as_str() {
                    Some("center") => Some(Align::Center),
                    Some("right") => Some(Align::Right),
                    Some("justify") => Some(Align::Justify),
                    _ => None,
                };
                if b["drop_cap"].as_bool() == Some(true) {
                    p.fmt.drop_cap = Some(3);
                }
                if let Some(c) = b["shading"].as_str().and_then(Color::parse) {
                    p.fmt.shading = Some(c);
                }
                let color = b["color"].as_str().and_then(Color::parse);
                let size = b["size"].as_f64().filter(|v| *v >= 6.0 && *v <= 200.0).map(|v| v as f32);
                for r in p.runs.iter_mut() {
                    if color.is_some() && r.fmt.link.is_none() {
                        r.fmt.color = color;
                    }
                    if size.is_some() {
                        r.fmt.size = size;
                    }
                }
                vec![p]
            }
        };
        if page_break {
            if let Some(f) = add.first_mut() {
                f.fmt.break_before = Some(Break::Page);
                page_break = false;
            }
        }
        paras.append(&mut add);
    }
    if paras.is_empty() {
        paras.push(Paragraph::default());
    }
    if let Some(at) = toc_at {
        let mut h = para("heading1", &crate::lang::tr("Spis treści"));
        h.fmt.break_before = None;
        paras.insert(at, h);
    }
    d.body = Story { paras };
    if !s("header").is_empty() {
        d.header = Some(Story { paras: vec![para("header", &s("header"))] });
    }
    if v["footer_page_numbers"].as_bool() != Some(false) {
        d.footer = Some(Story { paras: vec![Paragraph { style: "footer".into(), fmt: ParaFmt::default(), runs: vec![Run { text: FIELD_CHAR.to_string(), field: Some(Field::PageNumber), ..Default::default() }] }] });
    }
    // Obiekty w miejscu swoich akapitów (po pierwszym składzie).
    let mut eng = crate::layout::Engine::new();
    if toc_at.is_some() {
        let lay = eng.layout(&d);
        let toc = crate::layout::toc(&d, &lay);
        let at = toc_at.unwrap() + 1;
        let n = toc.len();
        d.body.paras.splice(at..at, toc);
        // Indeksy obiektów za spisem przesuwają się.
        for c in charts.iter_mut() {
            if c.0 >= at {
                c.0 += n + 1;
            }
        }
        for e in extras.iter_mut() {
            if e.at >= at {
                e.at += n + 1;
            }
        }
        for t in tables.iter_mut() {
            if t.0 >= at {
                t.0 += n + 1;
            }
        }
    }
    for (pi, chart) in charts {
        let lay = eng.layout(&d);
        let (page, top) = lay.story_lines(StoryRef::Body).iter().map(|&i| &lay.lines[i]).find(|l| l.para >= pi).map(|l| (l.page, l.top)).unwrap_or((0, d.page.margin_top));
        let (x0, x1) = d.page.column_spans()[0];
        let w = if cols > 1 { x1 - x0 } else { (x1 - x0) * 0.8 };
        let mut o = obj(d.new_id(), page, if cols > 1 { x0 } else { x0 + (x1 - x0 - w) / 2.0 }, top, w, w * 0.6, Shape::rect(), ObjKind::Chart { chart }, WrapMode::TopBottom);
        o.fill = Some(Fill::Solid { color: Color::rgb(0xfa, 0xfa, 0xfb) });
        o.corner = 6.0;
        d.objects.push(o);
    }
    let accent = d.style("heading1").and_then(|s| s.chars.color).unwrap_or(Color::rgb(0x1f, 0x4e, 0x79));
    for (pi, rows) in tables {
        let lay = eng.layout(&d);
        let (page, top) = lay.story_lines(StoryRef::Body).iter().map(|&i| &lay.lines[i]).find(|l| l.para >= pi).map(|l| (l.page, l.top)).unwrap_or((0, d.page.margin_top));
        let (x0, x1) = d.page.column_spans()[0];
        let w = if cols > 1 { x1 - x0 } else { d.page.content_width() };
        let table = Table { rows, accent, font: d.style("normal").and_then(|s| s.chars.font.clone()).unwrap_or_else(|| "Calibri".into()), ..Table::default() };
        let h = crate::paint::table_height(&table, w);
        let id = d.new_id();
        d.objects.push(obj(id, page, x0, top + 4.0, w, h, Shape::rect(), ObjKind::Table { table }, WrapMode::TopBottom));
    }
    let soft = |c: Color| {
        let mix = |v: u8| (v as f32 * 0.14 + 255.0 * 0.86) as u8;
        Color::rgb(mix(c.r), mix(c.g), mix(c.b))
    };
    for (k, e) in extras.into_iter().enumerate() {
        let lay = eng.layout(&d);
        let (page, top) = lay.story_lines(StoryRef::Body).iter().map(|&i| &lay.lines[i]).find(|l| l.para >= e.at).map(|l| (l.page, l.top)).unwrap_or((0, d.page.margin_top));
        let ps = d.page.clone();
        let cw = ps.content_width();
        let right = match e.side.as_str() {
            "left" => false,
            "right" => true,
            _ => k % 2 == 0,
        };
        let id = d.new_id();
        let o = match e.kind.as_str() {
            "frame" => {
                let full = e.side == "full";
                let w = if full { cw } else { (cw * 0.38).max(140.0) };
                let x = if full { ps.margin_left } else if right { ps.width - ps.margin_right - w } else { ps.margin_left };
                let paras: Vec<Paragraph> = e.text.split('\n').map(|t| para("normal", t)).collect();
                let lines = e.text.len() as f32 / (w / 6.0) + paras.len() as f32;
                let h = (lines * 15.0 / e.columns as f32 + 30.0).clamp(60.0, ps.height * 0.6);
                let mut o = obj(id, page, x, top, w, h, Shape::rect(), ObjKind::Text { story: Story { paras }, padding: 12.0, columns: e.columns, gap: 12.0, valign: VAlign::Top, next: None }, if full { WrapMode::TopBottom } else { WrapMode::Both });
                o.fill = Some(Fill::Solid { color: e.fill.unwrap_or(soft(accent)) });
                o.corner = 8.0;
                o
            }
            "path_text" => {
                let w = cw * 0.7;
                let mut sp = para("normal", &e.text);
                sp.fmt.align = Some(Align::Center);
                for r in sp.runs.iter_mut() {
                    r.fmt.size = Some(20.0);
                    r.fmt.tracking = Some(2.0);
                    r.fmt.color = Some(accent);
                }
                let shape = if e.shape == "wave" {
                    let mut s = crate::geom::wave();
                    s.contours[0].nodes.truncate(3);
                    s.contours[0].closed = false;
                    s
                } else {
                    crate::geom::arc_path()
                };
                obj(id, page, ps.margin_left + (cw - w) / 2.0, top, w, w * 0.35, shape, ObjKind::PathText { story: Story { paras: vec![sp] }, offset: 0.0 }, WrapMode::TopBottom)
            }
            _ => {
                let size = 170.0;
                let x = if right { ps.width - ps.margin_right - size } else { ps.margin_left };
                let mut sp = para("normal", &e.text);
                sp.fmt.align = Some(Align::Center);
                for r in sp.runs.iter_mut() {
                    r.fmt.size = Some(12.0);
                    r.fmt.bold = Some(true);
                }
                let sh = crate::geom::preset(&e.shape).filter(|s| s.contours.iter().any(|c| c.closed)).unwrap_or_else(crate::geom::ellipse);
                let mut o = obj(id, page, x, top, size, size, sh, ObjKind::Text { story: Story { paras: vec![sp] }, padding: 14.0, columns: 1, gap: 12.0, valign: VAlign::Top, next: None }, WrapMode::Both);
                o.fill = Some(Fill::Solid { color: e.fill.unwrap_or(soft(accent)) });
                o.stroke = Some(StrokeStyle { color: accent, width: 1.0, dash: vec![] });
                o
            }
        };
        d.objects.push(o);
    }
    d
}

/// Prośba użytkownika z dopiskami o długości i stylu.
pub fn prompt(user: &str, length: i32) -> String {
    let len = match length {
        0 => "Dokument krótki (ok. 1 strony).",
        2 => "Dokument obszerny (kilka stron, wiele sekcji).",
        _ => "Dokument średniej długości (2–3 strony).",
    };
    format!("{user}\n\n{len}")
}
