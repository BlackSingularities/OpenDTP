# OpenDTP

Edytor tekstu i program do składu dokumentów dla Windows 10 i 11. Natywny program w Rust ze Slint i własnym silnikiem składu: tekst w dowolnych kształtach, opływanie, kolumny, wykresy, generowanie dokumentów przez AI i eksport 1:1 do Worda i PDF.

## Najważniejsze

- **Kształty tekstu** – rysujesz kontur węzłami (proste i krzywe) albo wybierasz gotowy kształt; tekst wypełnia jego wnętrze, a treść strony opływa go z ustawionym odstępem. Otwory w kształcie (np. pierścień) – tekst przeskakuje na drugą stronę.
- **Skład** – kolumny, łańcuchy ramek, inicjały, siatka bazowa, polskie sierotki, nagłówki i stopki z polami.
- **Obiekty** – obrazy (kontur z przezroczystości), kształty z gradientem i cieniem, wykresy z edytowalnymi danymi, tekst na ścieżce, operacje logiczne na kształtach, przyciąganie do marginesów i innych obiektów.
- **AI** – opisujesz dokument, a wybrany model (OpenRouter, OpenAI, Anthropic, Gemini, Mistral, Groq, DeepSeek, xAI, Ollama, własny serwer) pisze go i składa z formatowaniem.
- **Pisownia** – polski (SJP.PL) i angielski, na bieżąco, ze słownikiem użytkownika.
- **Pliki** – własny `.odtp`; eksport DOCX, PDF, HTML, Markdown, TXT, PNG; import DOCX, TXT, Markdown.
- **Interfejs** – wstążka, paleta poleceń, motywy AMOLED / grafit / jasny, własny akcent, polski i angielski (kolejne języki to pliki `.po` w `lang/`).

## Instalacja

Program pobierzesz z [redzikowski.com](https://redzikowski.com): instalator (Program Files) albo wersja przenośna ZIP. Kolejne wersje program pobiera i instaluje sam.

## Budowanie

Wydania buduje GitHub Actions po wypchnięciu tagu `v*` (np. `v0.1.0`): instalator Inno Setup i wersja przenośna.

---

Pomysł i wykonanie: Adam Rędzikowski · [redzikowski.com](https://redzikowski.com) · [Postaw kawę](https://buycoffee.to/adamredzikowski)
