# Zmiany

## 0.5.0

- Obrazy osadzone i linkowane: łącze do pliku na dysku z automatycznym odświeżaniem po zmianie, przełączanie w obie strony, panel Obrazy z rozdzielczością i ostrzeżeniami, pakowanie dokumentu z obrazami.
- Nawigator: przesuwanie rozdziałów razem z podrozdziałami.
- Znajdź i zamień: wyrażenia regularne i wyszukiwanie w akapitach wybranego stylu.
- Biblioteka elementów: zapisane fragmenty tekstu i obiekty do wstawiania w dowolnym dokumencie.
- PDF do druku: kolory CMYK i dodatkowe (Pantone), spady, znaczniki cięcia.
- Kolory dokumentu: próbki z dokładnymi wartościami CMYK.
- Preflight: kontrola przed drukiem (nadmiar tekstu, brakujące pliki i czcionki, rozdzielczość, spady).

## 0.4.0

- Strony wzorcowe: osobne nagłówki i stopki stron parzystych (układ lustrzany) oraz obrazy, kształty i wykresy powtarzane na wszystkich, nieparzystych albo parzystych stronach.
- Żywa pagina: tytuł bieżącego rozdziału, podrozdziału albo hasła w nagłówku strony.
- Zakładki i odsyłacze („zob. s. 12”, „rys. 3”), które same się aktualizują.
- Podpisy rysunków, tabel i wzorów z automatyczną numeracją.
- Indeks (skorowidz): oznaczanie haseł i podhaseł, oznaczanie wszystkich wystąpień, polski porządek alfabetyczny i zakresy stron.
- Eksport tych elementów do DOCX jako pól Worda.

## 0.3.0

- Opcjonalne karty skrótów (do trzech, z własną nazwą) w miejsce stałej „Mojej karty”.
- Wyraźniejsze sekcje wstążki; podpisy grup nie są już ucinane.
- Jedno ustawienie przezroczystości dla wszystkich pasków, paneli, menu i okien; akryl Windows 11.
- Właściwości obiektu oraz Znajdź i zamień w zadokowanym pasku bocznym ze zmianą szerokości; ikony zamiast długich napisów.
- Podpowiedzi przy wszystkich kontrolkach, wyświetlane tuż przy kursorze.
- Rozbudowane menu kontekstowe dla tekstu, pisowni, łączy, obiektów i tabel.
- Start od ekranu powitalnego zamiast ostatniego dokumentu.

## 0.2.0

- Tabele płynące z tekstem: podział na strony z powtórzonym nagłówkiem, scalanie komórek, szerokość kolumn myszą, eksport jako tabela Worda.
- Obiekty zakotwiczone w tekście, grafika SVG, znak wodny i obramowanie strony.
- Sekcje (kolumny i orientacja), przypisy dolne i końcowe, eksport do DOCX.
- Typografia: dzielenie wyrazów, kontrola wdów, funkcje OpenType, linijka, okna Czcionka i Akapit.
- Nowy wygląd: pasek tytułu scalony z górnym paskiem, renderer Skia, chipy i segmenty z ikonami, ustawienia z ikonami.
- Szablon Podręcznik, tłumaczenie angielskie.

## 0.1.0

Pierwsze wydanie OpenDTP – edytora tekstu i programu do składu dokumentów dla Windows 10 i 11.

- **Kształty tekstu** – ramka o dowolnym kształcie z węzłów i krzywych Béziera (rysowanie piórem, 20 gotowych kształtów, edycja węzłów, operacje suma / różnica / część wspólna / wykluczenie). Tekst wypełnia wnętrze, a treść strony opływa kształt.
- **Opływanie** po konturze albo ramce: z obu stron, z lewej, z prawej, po większej stronie, nad i pod, za tekstem; odstęp; kontur obrazu wykrywany z przezroczystości lub tła.
- **Skład**: kolumny z linią, łańcuchy ramek (tekst przepływa między ramkami), kolumny w ramkach, inicjały, siatka bazowa, polskie sierotki, tabulatory z kropkami, nagłówki i stopki z numerami stron, inna pierwsza strona.
- **Tekst na ścieżce**, wykresy (kolumnowy, słupkowy, liniowy, warstwowy, kołowy, pierścieniowy), obrazy z dopasowaniem i maską kształtu.
- **Formatowanie**: style akapitów z galerią i panelem, motywy dokumentu, listy, wersaliki i kapitaliki, indeksy, podkreślenia, światło, cieniowanie i ramki akapitów, malarz formatów.
- **Pisownia** polska i angielska na bieżąco, podpowiedzi, słownik użytkownika; autokorekta, inteligentne cudzysłowy, automatyczne listy.
- **Generowanie dokumentów przez AI**: OpenRouter, OpenAI, Anthropic, Google Gemini, Mistral, Groq, DeepSeek, xAI, Ollama i własny serwer; lista modeli od dostawcy, temperatura, klucz szyfrowany DPAPI. AI zna wszystkie możliwości programu i składa dokument z pełnym formatowaniem.
- **Pliki**: własny format `.odtp`, eksport DOCX (style, listy, obrazy i kształty w tych samych miejscach, opływanie wielokątem; kształty tekstu jako dokładne wiersze albo edytowalne pole), PDF 1:1 z osadzonymi czcionkami, HTML, Markdown, TXT, PNG; import DOCX, TXT i Markdown; kopie do odzyskania po awarii.
- **Interfejs**: wstążka z 9 kartami, paleta poleceń (Ctrl+K), pasek szybkiego dostępu i „Moja karta” do dostosowania, panele wysuwane, motywy AMOLED / grafitowy / jasny, własny kolor akcentu, skala interfejsu; polski i angielski.
- **Aktualizacje** przez redzikowski.com: sprawdzanie w tle, pobieranie i instalacja (Program Files, uprawnienia administratora) w tym samym folderze; wersja przenośna aktualizuje się sama.
