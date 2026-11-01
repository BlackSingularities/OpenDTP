//! Rejestr poleceń: identyfikator, nazwa, ikona i skrót. Z niego budujemy paletę poleceń (Ctrl+K),
//! pasek szybkiego dostępu i kartę „Moja karta”. Polecenie z argumentem zapisujemy jako `id:arg`.

use opendtp_core::lang::tr;
use slint::Image;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub struct Cmd {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub key: &'static str,
}

const fn c(id: &'static str, label: &'static str, icon: &'static str, key: &'static str) -> Cmd {
    Cmd { id, label, icon, key }
}

pub const ALL: &[Cmd] = &[
    c("ai", "Generuj dokument z AI", "wand-sparkles", ""),
    c("new", "Nowy dokument", "file-plus", "Ctrl+N"),
    c("open", "Otwórz…", "folder-open", "Ctrl+O"),
    c("save", "Zapisz", "save", "Ctrl+S"),
    c("save-as", "Zapisz jako…", "save-all", "Ctrl+Shift+S"),
    c("export-docx", "Eksportuj do Worda (DOCX)", "file-type-2", ""),
    c("export-pdf", "Eksportuj do PDF", "file-output", ""),
    c("export-html", "Eksportuj do HTML", "globe", ""),
    c("export-md", "Eksportuj do Markdown", "hash", ""),
    c("export-txt", "Eksportuj do tekstu", "file-text", ""),
    c("export-png", "Eksportuj stronę jako obraz", "image", ""),
    c("print", "Drukuj (przez PDF)", "printer", "Ctrl+P"),
    c("undo", "Cofnij", "undo-2", "Ctrl+Z"),
    c("redo", "Ponów", "redo-2", "Ctrl+Y"),
    c("cut", "Wytnij", "scissors", "Ctrl+X"),
    c("copy", "Kopiuj", "copy", "Ctrl+C"),
    c("paste", "Wklej", "clipboard-paste", "Ctrl+V"),
    c("paste-plain", "Wklej jako tekst", "clipboard-paste", "Ctrl+Shift+V"),
    c("format-painter", "Malarz formatów", "paintbrush", ""),
    c("select-all", "Zaznacz wszystko", "text-select", "Ctrl+A"),
    c("find", "Znajdź", "search", "Ctrl+F"),
    c("replace", "Zamień", "replace", "Ctrl+H"),
    c("bold", "Pogrubienie", "bold", "Ctrl+B"),
    c("italic", "Kursywa", "italic", "Ctrl+I"),
    c("underline", "Podkreślenie", "underline", "Ctrl+U"),
    c("strike", "Przekreślenie", "strikethrough", ""),
    c("sup", "Indeks górny", "superscript", "Ctrl+Shift+="),
    c("sub", "Indeks dolny", "subscript", "Ctrl+="),
    c("grow", "Powiększ czcionkę", "a-arrow-up", "Ctrl+]"),
    c("shrink", "Pomniejsz czcionkę", "a-arrow-down", "Ctrl+["),
    c("clear-format", "Wyczyść formatowanie", "eraser", "Ctrl+Spacja"),
    c("caps:1", "Wersaliki", "case-sensitive", ""),
    c("caps:2", "Kapitaliki", "case-sensitive", ""),
    c("align:0", "Wyrównaj do lewej", "align-left", "Ctrl+L"),
    c("align:1", "Wyśrodkuj", "align-center", "Ctrl+E"),
    c("align:2", "Wyrównaj do prawej", "align-right", "Ctrl+R"),
    c("align:3", "Wyjustuj", "align-justify", "Ctrl+J"),
    c("list:bullet", "Lista punktowana", "list", ""),
    c("list:number", "Lista numerowana", "list-ordered", ""),
    c("indent", "Zwiększ wcięcie", "list-indent-increase", "Tab"),
    c("outdent", "Zmniejsz wcięcie", "list-indent-decrease", "Shift+Tab"),
    c("style:title", "Styl: Tytuł", "heading", ""),
    c("style:heading1", "Styl: Nagłówek 1", "heading-1", "Ctrl+Alt+1"),
    c("style:heading2", "Styl: Nagłówek 2", "heading-2", "Ctrl+Alt+2"),
    c("style:normal", "Styl: Normalny", "letter-text", "Ctrl+Shift+N"),
    c("style:quote", "Styl: Cytat", "quote", ""),
    c("marks", "Pokaż znaki formatowania", "pilcrow", "Ctrl+*"),
    c("page-break", "Podział strony", "separator-horizontal", "Ctrl+Enter"),
    c("column-break", "Podział kolumny", "columns-2", "Ctrl+Shift+Enter"),
    c("page-blank", "Pusta strona", "file-plus", ""),
    c("image", "Wstaw obraz", "image", ""),
    c("text-frame", "Ramka tekstowa", "type", ""),
    c("draw-shape", "Rysuj kształt tekstu węzłami", "pen-tool", ""),
    c("text-shape-star", "Kształt tekstu: gwiazda", "star", ""),
    c("text-shape-ellipse", "Kształt tekstu: koło", "circle", ""),
    c("text-shape-heart", "Kształt tekstu: serce", "heart", ""),
    c("text-shape-wave", "Kształt tekstu: fala", "waves", ""),
    c("shape-rect", "Kształt: prostokąt", "square", ""),
    c("shape-ellipse", "Kształt: elipsa", "circle", ""),
    c("shape-line", "Kształt: linia", "minus", ""),
    c("font-dialog", "Czcionka – wszystkie ustawienia", "sliders-horizontal", ""),
    c("para-dialog", "Akapit – wszystkie ustawienia", "sliders-vertical", ""),
    c("hyphenate", "Dzielenie wyrazów", "minus", ""),
    c("ruler-toggle", "Linijka", "ruler", ""),
    c("footnote", "Wstaw przypis dolny", "superscript", ""),
    c("endnote", "Wstaw przypis końcowy", "superscript", ""),
    c("section-new", "Nowa sekcja od nowej strony", "file-plus", ""),
    c("section-new-cont", "Nowa sekcja ciągła", "separator-horizontal", ""),
    c("table", "Wstaw tabelę", "layout-template", ""),
    c("page-border", "Obramowanie strony", "square-dashed", ""),
    c("chart-column", "Wykres kolumnowy", "chart-column", ""),
    c("chart-pie", "Wykres kołowy", "chart-pie", ""),
    c("chart-line", "Wykres liniowy", "chart-line", ""),
    c("path-text", "Tekst na ścieżce", "spline", ""),
    c("drop-cap", "Inicjał", "heading", ""),
    c("link", "Hiperłącze", "link", "Ctrl+K"),
    c("field-page", "Numer strony", "hash", ""),
    c("field-date", "Data", "calendar", ""),
    c("header-edit", "Nagłówek strony", "panel-top", ""),
    c("footer-edit", "Stopka strony", "panel-bottom", ""),
    c("toc", "Wstaw spis treści", "table-of-contents", ""),
    c("toc-update", "Aktualizuj spis treści", "refresh-cw", ""),
    c("caption", "Podpis rysunku", "captions", ""),
    c("bookmark", "Wstaw zakładkę (cel odsyłaczy)", "pin", ""),
    c("image-link", "Wstaw obraz jako łącze do pliku", "link", ""),
    c("image-embed", "Osadź obraz w dokumencie", "save", ""),
    c("image-to-link", "Zamień obraz na łącze do pliku", "link", ""),
    c("links-refresh", "Sprawdź obrazy linkowane", "refresh-cw", ""),
    c("links-embed-all", "Osadź wszystkie obrazy linkowane", "save-all", ""),
    c("package", "Spakuj dokument z obrazami", "folder-open", ""),
    c("lib-save", "Zapisz zaznaczenie w bibliotece", "book-a", ""),
    c("preflight", "Preflight – kontrola przed drukiem", "badge-info", ""),
    c("export-pdf-print", "Eksportuj PDF do druku", "printer", ""),
    c("swatch-add-used", "Pobierz kolory użyte w dokumencie", "palette", ""),
    c("track-toggle", "Śledź zmiany", "pen-line", ""),
    c("rev-next", "Następna zmiana", "chevron-right", ""),
    c("rev-accept-all", "Akceptuj wszystkie zmiany", "list-checks", ""),
    c("rev-reject-all", "Odrzuć wszystkie zmiany", "eraser", ""),
    c("comment", "Nowy komentarz", "message-square-quote", ""),
    c("version-save", "Zapisz wersję", "history", ""),
    c("export-epub", "Eksportuj do EPUB", "book-a", ""),
    c("index-mark", "Oznacz hasło indeksu", "hash", ""),
    c("index-mark-all", "Oznacz wszystkie wystąpienia w indeksie", "whole-word", ""),
    c("index", "Wstaw albo odśwież indeks", "book-a", ""),
    c("odd-even", "Inne nagłówki stron parzystych", "book-a", ""),
    c("running-head", "Żywa pagina w nagłówku", "panel-top", ""),
    c("columns-1", "Jedna kolumna", "file-text", ""),
    c("columns-2", "Dwie kolumny", "columns-2", ""),
    c("columns-3", "Trzy kolumny", "columns-3", ""),
    c("orientation", "Zmień orientację strony", "rectangle-horizontal", ""),
    c("node-mode", "Edytuj węzły kształtu", "spline-pointer", ""),
    c("link-frame", "Połącz ramki tekstowe", "link-2", ""),
    c("unlink-frame", "Rozłącz ramki", "unlink", ""),
    c("bool-union", "Suma kształtów", "squares-unite", ""),
    c("bool-difference", "Różnica kształtów", "squares-subtract", ""),
    c("wrap:1", "Opływanie z obu stron", "wrap-text", ""),
    c("wrap:5", "Opływanie: nad i pod", "between-horizontal-start", ""),
    c("wrap:6", "Za tekstem", "layers", ""),
    c("z-front", "Na wierzch", "bring-to-front", ""),
    c("z-back", "Na spód", "send-to-back", ""),
    c("duplicate", "Duplikuj obiekt", "copy", "Ctrl+D"),
    c("delete-obj", "Usuń obiekt", "trash-2", "Delete"),
    c("detect-contour", "Wykryj kontur obrazu", "scan", ""),
    c("spell-next", "Następny błąd pisowni", "spell-check", "F7"),
    c("stats", "Statystyka dokumentu", "sigma", ""),
    c("zoom-in", "Powiększ", "zoom-in", "Ctrl++"),
    c("zoom-out", "Pomniejsz", "zoom-out", "Ctrl+−"),
    c("zoom:width", "Szerokość strony", "move-horizontal", ""),
    c("zoom:page", "Cała strona", "file", ""),
    c("panel:nav", "Nawigator", "list-tree", ""),
    c("panel:styles", "Panel stylów", "letter-text", ""),
    c("guides", "Linie pomocnicze", "ruler", ""),
    c("theme:0", "Motyw AMOLED", "moon", ""),
    c("theme:1", "Motyw grafitowy", "contrast", ""),
    c("theme:2", "Motyw jasny", "sun", ""),
    c("fullscreen", "Pełny ekran", "maximize", "F11"),
    c("ribbon-collapse", "Zwiń / rozwiń wstążkę", "chevron-up", "Ctrl+F1"),
    c("settings", "Ustawienia", "settings", ""),
    c("check-update", "Sprawdź aktualizacje", "refresh-cw", ""),
    c("about", "O programie", "badge-info", ""),
    c("support", "Postaw kawę autorowi", "coffee", ""),
];

pub fn find(id: &str) -> Option<&'static Cmd> {
    ALL.iter().find(|c| c.id == id)
}

pub fn icon(name: &str) -> Image {
    ICONS.iter().find(|(n, _)| *n == name).and_then(|(_, data)| Image::load_from_svg_data(data).ok()).unwrap_or_default()
}

pub fn label(c: &Cmd) -> String {
    tr(c.label).to_string()
}

/// Proste dopasowanie do palety: wszystkie słowa zapytania w nazwie (bez polskich znaków i wielkości liter).
pub fn matches(c: &Cmd, q: &str) -> bool {
    let fold = |s: &str| -> String {
        s.to_lowercase().chars().map(|ch| match ch {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            o => o,
        }).collect()
    };
    let hay = fold(&format!("{} {} {}", label(c), c.label, c.id));
    fold(q).split_whitespace().all(|w| hay.contains(w))
}
