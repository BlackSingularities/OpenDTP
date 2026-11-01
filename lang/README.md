# Tłumaczenia OpenDTP

Tekstem źródłowym jest polski. Każdy inny język to jeden plik gettext:

```
lang/<kod>/LC_MESSAGES/opendtp.po
```

Ten sam plik czyta Slint (teksty `@tr("…")` w `app/ui/*.slint`) i program w Ruście (`tr("…")`, `trf!("…")` – `core/src/lang.rs`). Budowa sama znajduje wszystkie foldery w `lang/`.

## Nowy język

1. Skopiuj `lang/opendtp.pot` do `lang/<kod>/LC_MESSAGES/opendtp.po` (kod jak w Windows: `de`, `fr`, `uk`, `cs`…).
2. W nagłówku wpisz `Language: <kod>` i `X-Language-Name: <nazwa języka w tym języku>`.
3. Uzupełnij `msgstr` (np. w Poedit). Puste `msgstr` – zostaje tekst polski.
4. Dopisz język do przełącznika w Ustawieniach (`app/ui/backstage.slint`, `s-lang`) i wypchnij tag – GitHub Actions zbuduje program z nowym językiem.

Miejsca `{nazwa}` i `{}` zostaw bez zmian (kolejność można zmienić).
