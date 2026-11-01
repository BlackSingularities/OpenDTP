# Zmiany

## Niewydane

- Rdzeń podzielony na osobne moduły (`crates/`): podstawa, dokument, tekst, format, skład, wymiana plików, AI, aktualizacje.
- Bezpieczny zapis wszystkich plików (dokument, eksport DOCX/PDF/EPUB/HTML/MD/TXT/PNG, kopie odzyskiwania, ustawienia, słownik): plik tymczasowy, zapis na dysk, podmiana z ponawianiem, gdy plik chwilowo blokuje antywirus albo OneDrive. Przerwany zapis nie psuje już istniejącego pliku.
- Dziennik programu i zapis awarii (bez treści dokumentu); po awarii dokument zostaje zachowany i otwiera się przy następnym uruchomieniu, a program pokazuje komunikat z miejscem dziennika.
- Odzyskiwanie nie usuwa już kopii z innego otwartego okna OpenDTP ani starszych kopii z kilku awarii.
- Szybsze cofanie przy długiej historii zmian.
- Korespondencja seryjna: pola «Nazwa» albo {{Nazwa}} w treści, dane z CSV (średnik, przecinek albo tabulator; UTF-8 lub Windows-1250) – jeden dokument z rekordem na każdej stronie albo osobny plik dla każdego wiersza.
- Porównywanie dokumentów: różnice względem wcześniejszej wersji (ODTP, DOCX, ODT, RTF, TXT, MD) jako śledzone zmiany na poziomie akapitów i słów.
- Inspektor dokumentu: komentarze, zmiany, historia wersji i dane autora w pliku – z usuwaniem jednym poleceniem.
- Kontrola dostępności w panelu Preflight: tekst alternatywny obrazów i wykresów, poziomy nagłówków, wiersz nagłówka tabel, kontrast tekstu (WCAG), język i tytuł dokumentu, łącza opisane adresem.
- Tabele: sortowanie według kolumny (liczby jako liczby, polski porządek alfabetyczny), suma kolumny, zamiana tekstu w tabelę i tabeli w tekst.
- Otwieranie plików ODT (LibreOffice), RTF i HTML.
- PDF: zakładki z nagłówków i klikalne łącza.
- Przyjęcie albo odrzucenie zmian usuwa akapity, które w całości były zmianą (bez pustych wierszy).
- Cytowania i bibliografia w stylach APA 7, MLA 9, Chicago (autor–data) i IEEE: źródła z formularza albo z plików BibTeX i RIS (Zotero, Mendeley, JabRef, Google Scholar), cytowanie kliknięciem, bibliografia ze stylem z wysunięciem; zmiana stylu przelicza wszystkie cytowania (IEEE – numery według kolejności użycia).
- Spis ilustracji, tabel i wzorów z numerami stron, odświeżany razem ze spisem treści.
- Eksport do OpenDocument (ODT) i RTF.
- Etykiety adresowe z CSV na arkuszach A4 (3 × 8, 3 × 7, 2 × 7, 2 × 4), z wzorem z treści dokumentu albo ze wszystkich kolumn.
- „Aktualizuj wszystko” odświeża też cytowania, bibliografię i spisy ilustracji.

## 0.7.0

- Przebudowana wstążka: przyciski ustawień w nagłówkach grup, równe nagłówki u dołu, zwężanie do szerokości okna jak w Wordzie, przycisk zwijania w prawym dolnym rogu.
- Nowe narzędzia: wklejanie z zachowaniem formatowania z Worda, przeglądarek i Outlooka, efekty tekstu (cień, poświata, kontur), lista wielopoziomowa, sortowanie akapitów, obramowanie akapitu, metatagi, strona tytułowa, kody QR i kreskowe, zamiana tekstu w kształt i z powrotem.
- Karta Układ: miniatura marginesów, okno Ustawienia strony z własnym rozmiarem papieru, tło i rozbudowane obramowanie strony (podwójne, kreskowane, kropkowane), typografia; karta Projekt połączona z Układem.
- Widok: płynne przewijanie i powiększanie, rzeczywisty rozmiar przy 100% (fizyczny rozmiar ekranu), linijka pionowa, numeracja wierszy (także w PDF i DOCX).
- Uporządkowane karty Wstawianie, Kształty tekstu, Odwołania i Recenzja; „Aktualizuj wszystko” dla spisu treści, indeksu i odsyłaczy.
- Ustawienia w kartach z bocznym spisem sekcji; paleta poleceń przy polu wyszukiwania.
- Import DOCX: style z Worda z dziedziczeniem, pola tekstowe, kształty i grupy w swoich miejscach, kolory motywu, przypisy, tabele z kolumnami, wzory WMF jako grafika wektorowa.
- Nowy przewodnik po możliwościach programu i nowe szablony: CV, zaproszenie, dyplom, menu, faktura, plakat, kartka z życzeniami, broszura, rozdział książki; odświeżone raport, list, gazetka i ulotka.

## 0.6.0

- Śledzenie zmian: oznaczanie wstawień i usunięć, akceptowanie i odrzucanie pojedynczo albo wszystkich naraz, eksport do DOCX jako zmiany Worda.
- Komentarze z odpowiedziami i oznaczaniem jako rozwiązane.
- Historia wersji zapisywana w pliku dokumentu, z przywracaniem.
- Eksport do EPUB 3: rozdziały z nagłówków, spis treści, obrazy.
- Preflight ostrzega o niezaakceptowanych zmianach i otwartych komentarzach.

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
