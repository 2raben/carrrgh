# Problem

In [Car Drive](../car-drive/pitch--car-movements.md) zählt nur die laufende Fahrt.
Nach dem Game-over zeigt das Spiel die erreichten Meter, ein Neustart setzt alles
auf null, und beim Schließen des Fensters ist jede Spur der bisherigen Fahrten
verloren. Es gibt nichts, das über mehrere Fahrten wächst, und das Spiel weiß
nicht, wer spielt.

Eine Währung soll Fortschritt über einzelne Fahrten hinaus sichtbar machen: Jeder
gefahrene Meter bringt etwas ein, auch wenn die Fahrt früh endet. Damit das
funktioniert, braucht das Spiel einen Spieler, dem die Coins gehören, und einen
Speicher, der Neustarts und Programmende übersteht. Der ursprüngliche Pitch und
die [Spielidee](../../idea.md) schlossen gespeicherte Daten bisher aus.

Das Spiel läuft heute nur nativ. Ein späteres Browserspiel ist denkbar; der
Speicher soll so geschnitten sein, dass es dann nicht an ihm scheitert.

**Status: umgesetzt.** Die [Architekturdokumentation](../../architecture/README.md#profiles-and-coins)
beschreibt die konkreten Regeln, [ADR 0007](../../architecture/decisions/0007-persist-profiles-behind-a-store-interface.md)
die Wahl der Speicherung. Die Coin-Rate bleibt ein Spieltestwert.

# Solution

Drei Teile, die zusammen ein spielbares Experiment ergeben: Coins aus Metern, ein
lokales Spielerprofil und eine austauschbare Persistenzschicht mit SQLite als
erster Umsetzung.

## Ablauf

```text
Programmstart
  └─ Profil laden ──┬─ vorhanden ──────────────────────────┐
                    └─ keines → „Spieler anlegen“ → speichern ┤
                                                           ▼
                         Fahrt ── Game-over oder Neustart ──► Coins gutschreiben
                           ▲                                     │
                           └──────────── nächste Fahrt ◄─────────┘
```

## Coins aus Metern

Eine Fahrt verdient Coins proportional zur gefahrenen Strecke: als Startwert
**ein Coin pro 10 m**, abgerundet auf ganze Coins. Die Rate ist ein Spieltestwert
und steht an einer Stelle. Boost, Schwierigkeitsstufe oder Treffer ändern die
Rate im ersten Schritt nicht; wer weiter fährt, verdient mehr, und Boost lohnt
sich nur über die schneller zurückgelegte Strecke.

Die Coins einer Fahrt werden gutgeschrieben, wenn die Fahrt endet:

- beim Game-over, also nach dem dritten Treffer;
- beim Neustart mitten in einer Fahrt, denn auch eine abgebrochene Fahrt ist
  gefahren;
- beim regulären Schließen des Programms während einer Fahrt, soweit der
  Speicher das noch abschließen kann.

Jede Fahrt wird genau einmal gutgeschrieben. Ein Neustart direkt nach dem
Game-over bucht nichts ein zweites Mal. Der Kontostand selbst geht durch
Game-over, Neustart oder Programmende nie verloren; nur bei einem Absturz kann
die gerade laufende Fahrt fehlen.

Die Umrechnung ist reine Simulationslogik: Die Simulation kennt die Meter der
Fahrt, berechnet daraus die verdienten Coins und meldet das Fahrtende als
Nachricht. Sie liest und schreibt dabei nichts selbst; das übernimmt die
Persistenzschicht.

## Spielerprofil

Findet das Spiel beim Start kein Profil, zeigt es vor der ersten Fahrt einen
einfachen Bildschirm „Spieler anlegen“: ein Namensfeld, vorbelegt mit einem
Standardnamen wie „Fahrer“ (im englischsprachigen Spiel „Driver“), und eine
Bestätigung. Tastatur kann den Namen
ändern; mit Controller lässt sich der Standardname direkt bestätigen. Erst nach
erfolgreichem Speichern beginnt die Fahrt. Findet das Spiel ein Profil, beginnt
die Fahrt wie heute sofort.

Ein Profil besteht aus einer stabilen ID, dem Namen, dem Coin-Kontostand und dem
Erstellungszeitpunkt. Namen sind 1–16 Zeichen lang, werden an den Rändern
gekürzt und dürfen keine Steuerzeichen enthalten. Der Datenbestand erlaubt
mehrere Profile, das Spiel legt in diesem Pitch aber genau eines an und
verwendet es.

## Darstellung

Das HUD zeigt zusätzlich den Spielernamen, den Kontostand und die Coins der
laufenden Fahrt, zum Beispiel `+123`. Der Game-over-Bildschirm nennt die
verdienten Coins und den neuen Kontostand, sobald die Gutschrift bestätigt
gespeichert ist. Schlägt das Speichern fehl, steht dort ein deutlicher Hinweis
statt eines falschen Kontostands.

## Persistenzschicht

Das Spiel spricht nicht mit SQLite, sondern mit einer kleinen, spieleigenen
Schnittstelle für Profile. Sie bietet nur, was dieses Experiment braucht:

- vorhandenes Profil laden;
- neues Profil anlegen;
- die Coins einer Fahrt einem Profil gutschreiben.

Die Schnittstelle verwendet eigene, versionierte Datentypen statt ECS-Komponenten,
SQL-Typen oder Zeilen. Alle Operationen sind asynchron gedacht: Spielsysteme
stoßen sie an und erhalten das Ergebnis später als Nachricht oder Ressource. Kein
System im Fixed-Step wartet auf den Speicher. Damit passt dieselbe Schnittstelle
zu nativer Datei-I/O und zu den asynchronen Speichern eines Browsers.

Umsetzungen:

1. **SQLite (nativ, dieser Pitch):** eine Datenbankdatei im Datenverzeichnis des
   Benutzers, mit Tabellen für Profile und verbuchte Fahrten. Eine Gutschrift
   ist eine Transaktion: Fahrt eintragen und Kontostand erhöhen gelingen
   gemeinsam oder gar nicht. Eine eindeutige Fahrt-ID macht doppelte
   Gutschriften wirkungslos. Die Schemaversion steht in der Datenbank; Änderungen
   laufen als nummerierte Migrationen. Blockierende Zugriffe laufen im
   `IoTaskPool`, nicht im Spielablauf.
2. **Arbeitsspeicher (Tests):** dieselbe Schnittstelle ohne Datei, damit die
   Simulationstests headless und deterministisch bleiben.
3. **Browser (später, nicht Teil dieses Pitches):** eine Umsetzung auf IndexedDB
   oder `localStorage` hinter derselben Schnittstelle. Der Pitch muss nur
   sicherstellen, dass nichts außerhalb der SQLite-Umsetzung SQLite voraussetzt.

Der Speicherort ist für Entwicklung und Tests überschreibbar, damit
Spielproben nicht das eigene Profil verändern. Die Wahl von SQLite, die neue
Abhängigkeit und das gespeicherte Format brauchen eine ADR; die
[Architekturdokumentation](../../architecture/README.md) wird bei der Umsetzung
ergänzt.

## Umfang des Experiments

Das Experiment ist spielbar, wenn man beim ersten Start einen Spieler anlegt,
fährt, nach dem Game-over die Gutschrift sieht und nach einem Programmneustart
denselben Spieler mit demselben Kontostand vorfindet.

Automatisierte Tests sollen prüfen: Die Umrechnung von Metern in Coins rundet
ab und hängt nicht von Boost oder Stufe ab; Game-over, Neustart mitten in der
Fahrt und Neustart nach dem Game-over buchen jede Fahrt genau einmal; ohne
Profil startet keine Fahrt; die Simulation bleibt mit der Arbeitsspeicher-Umsetzung
headless lauffähig. Für SQLite in einer temporären Datei: Profil anlegen und
wieder laden, Gutschrift ist atomar und bei Wiederholung idempotent, eine
leere Datei wird initialisiert, eine Datei mit unbekannter, neuerer
Schemaversion wird nicht überschrieben, ungültige Namen werden abgelehnt.
Eine Spielprobe prüft, ob die Coin-Rate sich lohnend anfühlt und ob der
Anlegen-Bildschirm den Einstieg nicht bremst.

# Security

Der Pitch führt erstmals dauerhaft gespeicherte Daten und freie Texteingabe ein.

- **Spielername:** Er ist nicht vertrauenswürdige Eingabe. Länge und erlaubte
  Zeichen werden vor dem Speichern geprüft, und beim Laden erneut, weil die Datei
  von außen verändert sein kann. Der Name wird nur als Text dargestellt.
- **SQL:** Alle Abfragen verwenden Parameter; Eingaben werden nie in SQL-Text
  eingesetzt.
- **Speicherdatei:** Sie liegt lokal im Benutzerverzeichnis und enthält keine
  Geheimnisse, keine E-Mail-Adresse und keine Zugangsdaten. Wer die Datei
  bearbeitet, kann seinen Kontostand ändern; für ein lokales Einzelspielerspiel
  ohne Bestenliste ist das hingenommen. Beschädigte, unbekannte oder zu neue
  Dateien werden nicht stillschweigend überschrieben; das Spiel bleibt dann ohne
  Speichern spielbar und sagt das.
- **Netzwerk:** Es gibt keines. Profile werden nicht synchronisiert oder geteilt.
- **Abhängigkeit:** Eine SQLite-Bibliothek kommt hinzu. Sie wird gepinnt, in der
  ADR begründet und über `Cargo.lock` nachvollziehbar gehalten.

# Rabbit Holes

- **Texteingabe in Bevy:** Ein vollständiges Eingabefeld mit Cursor, Auswahl,
  IME und Bildschirmtastatur kann den ganzen Pitch verschlingen. Mit einem
  minimalen Feld beginnen: Zeichen anhängen, Rücktaste, Enter; ein vorbelegter
  Name macht das Feld für Controller optional.
- **Wann eine Fahrt endet:** Game-over, Neustart, Schließen und Absturz haben
  unterschiedliche Garantien. Gutschrift nur an klaren Fahrtenden mit
  eindeutiger Fahrt-ID; keine laufenden Zwischenstände während der Fahrt. Ein
  Absturz kostet höchstens die laufende Fahrt.
- **Asynchroner Speicher und Spielzustand:** Laden, Anlegen und Gutschreiben
  dauern unbestimmt lange. Einen einfachen App-Zustand (Profil laden → Spieler
  anlegen → Fahren) verwenden, statt Fahrlogik auf Speicherergebnisse warten zu
  lassen. Eine noch nicht bestätigte Gutschrift bleibt im Speicher und wird bei
  der nächsten Gelegenheit erneut versucht.
- **SQLite im Browser:** SQLite nach WebAssembly zu bringen (OPFS, sql.js) ist
  ein eigenes Projekt. Nicht versuchen; die Schnittstelle ist der Austauschpunkt.
- **Abstraktion zu groß schneiden:** Kein generisches Repository, keine
  allgemeine Spielstandverwaltung und kein ORM. Nur die drei Operationen, die
  dieses Experiment braucht; weitere kommen mit ihren eigenen Pitches.
- **Native Build-Abhängigkeiten:** Eine gebündelte SQLite braucht einen
  C-Compiler. Im Projekt-`shell.nix` und mit der vorhandenen Clang-Konfiguration
  prüfen, bevor die Abhängigkeit festgelegt wird.
- **Schemaentwicklung:** Mit Version 1 und einer Migrationsfunktion beginnen;
  keine Migrationsbibliothek, bevor es eine zweite Version gibt.

# No-Gos

Kein Shop, keine Ausgaben von Coins, keine Upgrades oder freischaltbaren Inhalte.
Keine Coins als Fundstücke auf der Straße und keine Multiplikatoren für Boost,
Stufe oder fehlerfreie Fahrt. Keine Profilauswahl, kein Umbenennen oder Löschen
im Spiel und keine Profilbilder. Keine Online-Konten, Synchronisation,
Cloud-Speicher oder Bestenlisten. Kein Schutz gegen das Bearbeiten der
Speicherdatei. Kein Browser-Build und keine Browser-Umsetzung des Speichers in
diesem Pitch. Keine gespeicherten Einstellungen, Rekorde oder Fahrtverläufe
außer der für die einmalige Gutschrift nötigen Fahrt-ID.
