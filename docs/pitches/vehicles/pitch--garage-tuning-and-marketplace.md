# Problem

Der [Coins-Pitch](../coins/pitch--coins-and-player-profile.md) gibt gefahrenen
Metern einen dauerhaften Wert. Es fehlt aber ein Ziel, für das Spieler ihre
Münzen ausgeben können. Das eigene Fahrzeug lässt sich weder auswählen noch
gestalten, und verschiedene Fahrstile haben keinen Platz in der Garage.

Fahrzeuge sollen Sammelziele und spielbare Alternativen werden. Wer auf ein
anderes Modell umsteigt, soll sein bisheriges Auto samt Tuning verkaufen können.
Ein Handelsplatz soll diese Fahrzeuge anderen Spielern anbieten, ohne dass
Verkäufer Preise aushandeln oder auf eine Auszahlung warten müssen.

**Status: vorgeschlagen.** Baut auf Coins und Spielerprofil auf. Garage, Tuning
und Handelsplatz sind neue Arbeit, keine bereits implementierten Funktionen.

# Solution

Eine Garage verbindet Fahrzeugkatalog, Konfiguration, Werkstatt und Handelsplatz.
Alle zehn Modelle sind fahrbar, sehen unterschiedlich aus und besitzen ein
eigenes Fahrprofil. Käufe und Tuning kosten ausschließlich erspielte Coins.

## Ablauf und Startauto

```text
Fahrt beenden → Coins gutschreiben → Garage
                                     ├─ Fahrzeug auswählen → nächste Fahrt
                                     ├─ Katalog → Fahrzeug kaufen
                                     ├─ Werkstatt → Farbe, Reifen und Tuning
                                     └─ Handelsplatz
                                          ├─ einstellen → sofort Coins erhalten
                                          └─ angebotenes Fahrzeug kaufen → Garage
```

Die Garage ist vor einer Fahrt und nach deren Ende erreichbar. Sie zeigt
Kontostand, eigene Fahrzeuge und das aktive Fahrzeug. Die letzte Auswahl bleibt
gespeichert; ein Schnellstart beginnt die nächste Fahrt damit. Während einer
Fahrt sind Kauf, Verkauf und Konfigurationsänderungen gesperrt. Noch nicht
bestätigte Fahrtgutschriften können nicht ausgegeben werden.

Jedes Profil erhält einmalig einen **Opel Corsa als Startauto**, ohne Coin-Abzug.
Auch bestehende Profile ohne Garage bekommen diesen Corsa genau einmal. Weitere
Corsas kosten den Katalogpreis. Ein Neustart oder Verkauf erzeugt kein neues
Startauto. Mindestens ein fahrbares Fahrzeug muss im Besitz bleiben; vor dem
Verkauf des aktiven Autos wird ein anderes ausgewählt.

## Fahrzeugangebot und Preise

Die Preise orientieren sich an realen Größenordnungen. Als erste Skala entspricht
**ein Euro Referenzwert einem Coin Katalogpreis**. Das ist eine Spielpreisregel,
keine Möglichkeit zum Umtausch in Geld.

Die Tabelle enthält gewählte Varianten und gerundete Spielpreise. Historische
Listenpreise, Sammlerwerte und ausländische Preise sind gekennzeichnet; sie bilden
keinen einheitlichen aktuellen Neuwagenmarkt ab. Recherche: 12. November 2026.
Die Referenzen werden als feste Katalogwerte übernommen, nicht während des Spiels
aus dem Internet geladen.

| Fahrzeug | Reale Preisorientierung | Katalogpreis in Coins | Aussehen und vorgeschlagenes Fahrprofil |
| --- | --- | ---: | --- |
| Bobby-Car | BIG Classic: rund 55 € ([BIG](https://www.big.de/big_de/kategorien/kinderfahrzeuge/big-bobby-car/classic/big-bobby-car-classic-800001303-de.html)) | 55 | Kleine offene Kunststoffkarosserie, sichtbares Lenkrad; sehr wendig, niedriges Tempo, kurzer Boost. |
| Opel Corsa — Startauto | Einstieg 2025: 22.890 € ([Opel](https://www.media.stellantis.com/de-de/opel/press/opel-corsa-jetzt-auch-als-ultimate-und-mit-neuem-winter-paket)) | 23.000; erstes Exemplar gratis | Kompakter Kleinwagen; ausgewogene Referenz für Tempo, Lenkung und Boost. |
| Audi RS6 | Avant performance, Modelljahr 2025: 138.500 € ([Audi-Preisliste](https://media.audi.com/is/content/audi/country/de/de/neuwagen/preislisten/my-2025/preislisten/preisliste_rs-6-avant_rs-6-avant-performance.pdf)) | 140.000 | Langer, sportlicher Kombi; hohes Tempo und kräftiger Boost, größere Kollisionsfläche als der Corsa. |
| Ford F150 | F-150 XL 2025 ab 38.810 US-Dollar, ohne US-Steuern und Überführung ([Ford](https://www.fordpro.com/en-us/fleet-vehicles/f150-2025/f150-xl/)) | 40.000 | Breiter Pickup mit Ladefläche; lange Boostdauer, langsameres seitliches Ausweichen. |
| Koenigsegg Jesko | Attack: rund 2,8 Mio. € ursprünglicher Preis ([AUTO BILD, 2021](https://www.autobild.de/artikel/koenigsegg-jesko-2022-preis-kaufen-ps-v8-absolut-hypersportwagen-20379511.html)) | 2.800.000 | Flache, breite Karosserie mit Heckflügel; höchstes Tempo, hoher Boostverbrauch und wenig Reaktionszeit. |
| BMW M1 | Straßenmodell: Auktionsschätzung 2025 von 425.000–525.000 € für ein Exemplar ([radical, mit Losbeschreibung](https://radical-mag.com/en/2025/04/25/bmw-m1-1179-2/)) | 500.000 | Kantige Keilform und Hecklamellen; sportlich und direkt lenkbar, kürzerer Boost als der RS6. |
| Mazda5 | 1.8 Center-Line: historischer Neupreis 22.790 € ([Motor1-Test, 2011](https://de.motor1.com/reviews/160019/mazda-5-18-im-test-urbane-familie/)) | 23.000 | Langer Van mit erkennbaren Schiebetüren; moderates Tempo und schnelle Boostregeneration, größerer Platzbedarf. |
| Fendt 1050 | Vario PowerPlus: 357.620 € ohne MwSt., Referenz 2018; rechnerisch rund 425.568 € mit 19 % ([profi/DLG](https://pruefberichte.dlg.org/filestorage/0318_Fendt_1050_Vario.pdf)) | 425.000 | Traktor mit Kabine und großen freistehenden Rädern; niedriges Tempo, ausdauernder Boost, sehr breite Kollisionsfläche. |
| Seat Ateca | Style 1.5 TSI DSG: 33.320 €, historische Liste Januar 2023 ([SEAT-Preisliste beim Händler](https://schuler-eisner.de/files/content/fahrzeuge/seat-ateca/Preisliste_SEAT_Ateca.pdf)) | 33.000 | Kompaktes SUV; etwas höheres Tempo als der Corsa, etwas langsamere Lenkung. |
| Volvo XC60 | Mild-Hybrid ab 59.990 € UVP ([Volvo](https://www.volvocars.com/de/cars/xc60/)) | 60.000 | Größeres SUV mit markanten Leuchten; mittleres Tempo, ausdauernder Boost, breiter als der Ateca. |

Beim Ford sind 40.000 Coins eine grobe Spielpreisannahme auf Basis des
US-Einstiegsmodells, kein deutscher Importpreis und keine genaue Umrechnung.
Der M1 nutzt einen Sammlerwert statt seines historischen Neupreises; der Mazda5
dagegen einen historischen Listenpreis. Diese Auswahl bleibt nachvollziehbar und
kann beim Balancing angepasst werden.

**Preis ist kein Leistungsrang:** Ein Sammlerauto oder Traktor darf teurer sein
als ein schnelleres Auto. Fahrprofile sind Arcade-Entwürfe, keine Aussagen über
reale Fahreigenschaften. Auch Bobby-Car und Fendt fahren automatisch mit
spielbaren Geschwindigkeiten.

Bei einem Coin je zehn Meter entsprechen 23.000 Coins bereits 230 km und der
Jesko 28.000 km Spielstrecke, jeweils ohne weitere Ausgaben. Das ist ein
wesentliches Risiko fürs Spielgefühl. Bobby-Car und kleine Upgrades sollen früh
erreichbar sein; teure Fahrzeuge bleiben langfristige Ziele. Vor Umsetzung die
Sparzeiten erproben und bei Bedarf die allgemeine Verdienstregel des Coins-Pitches
ausdrücklich weiterentwickeln. Testkontostände machen alle Modelle ohne Wartezeit
erprobbar.

## Unterschiede während der Fahrt

Die Modelle unterscheiden sich über Reise- und Boosttempo, seitliche
Lenkgeschwindigkeit, Boostdauer und Regeneration sowie Breite und Länge.
Die Garage vergleicht diese Werte mit dem aktiven Auto, einschließlich Reifen
und Tuning. Drei Herzen, Herz-Pickups und Schilde gelten für alle; große Fahrzeuge
erhalten keine versteckte Kollisionsimmunität.

Silhouette, Fenster, Leuchten und charakteristische Teile unterscheiden die
Modelle auch bei gleicher Farbe. Die Draufsicht vereinfacht ihre Proportionen.
Sichtbare Größe und Kollisionsfläche passen zusammen; auch der Fendt muss in
einer Spur fahrbar bleiben. Verkehrsabstände, Pickup-Freiräume und Kollisionen
berücksichtigen das gewählte Fahrzeug. Die zehn Schwierigkeitsstufen wirken auf
dessen Grundwerte innerhalb erprobter Grenzen für Tempo und Ausweichmöglichkeiten.

## Farbe, Bereifung und Tuning

Die Werkstatt zeigt vor jedem Kauf Vorschau, Kosten, verbleibenden Kontostand
und den Vergleich der Fahrwerte. Erst die Bestätigung kauft die Änderung.
Jede Konfiguration gehört zu einem konkreten Fahrzeug und wird mitverkauft.

| Option | Wirkung | Vorgeschlagene Kosten |
| --- | --- | --- |
| Farbe | Grundfarbe aus einer Palette; ohne Fahrwertänderung. | Auswahl und spätere Farbwechsel kostenlos. |
| Serienreifen | Ausgewogenes Ausgangsprofil, modelltypisches Aussehen. | Enthalten. |
| Sportreifen | Sichtbar anderes Profil; +10 % seitliche Lenkgeschwindigkeit, −5 % Boostdauer. | Einmalig 2 % des Fahrzeug-Katalogpreises. |
| Effizienzreifen | +10 % Boostdauer, −5 % seitliche Lenkgeschwindigkeit. | Einmalig 2 % des Fahrzeug-Katalogpreises. |
| Motor Stufe 1 | +5 % Reise- und Boosttempo. | 5 % des Fahrzeug-Katalogpreises. |
| Motor Stufe 2 | Insgesamt +10 % Reise- und Boosttempo; benötigt Stufe 1. | Weitere 10 % des Fahrzeug-Katalogpreises. |
| Carbon-Body-Kit | Sichtbare Carbonteile und modellspezifische Anbauteile; +5 % seitliche Lenkgeschwindigkeit. | Einmalig 8 % des Fahrzeug-Katalogpreises. |

Kosten werden auf ganze Coins aufgerundet, mindestens ein Coin. Beim kostenlosen
Start-Corsa zählt der reguläre Katalogpreis. Gekaufte Reifensätze, Motorstufen und
Body-Kit bleiben am Fahrzeug freigeschaltet; der Wechsel zwischen ihnen und dem
Serienzustand ist kostenlos. Aktiv sind jeweils ein Reifensatz, eine Motorstufe
und höchstens ein Body-Kit. Boni werden aus Grundwerten neu berechnet und können
sich durch wiederholtes Ausrüsten nicht aufschaukeln. Lenkboni werden addiert;
anschließend gelten die Modellgrenzen.

Alle zehn Modelle erhalten diese Kategorien mit passenden Darstellungen. Der
bessere Motor des Bobby-Cars ist eine spielerische Fantasie. Carbonteile lassen
die Lackfarbe weiter erkennen und vergrößern die Kollisionsfläche im ersten
Experiment nicht. Alle Prozentwerte sind Spieltestvorschläge.

## Handelsplatz mit garantiertem Sofortankauf

**Beim Einstellen kauft der Handelsplatz das Fahrzeug sofort an.** Nach
Bestätigung werden Besitzübergang und Coin-Auszahlung gemeinsam gespeichert.
Der Verkäufer wartet auf keinen anderen Spieler. Danach bietet der Handelsplatz
genau dieses Fahrzeug mit Farbe und allen gekauften Upgrades anderen Spielern an.
Der spätere Kauf zahlt den ursprünglichen Verkäufer nicht ein zweites Mal aus;
die Zahlung geht an den Handelsplatz.

Einstellen ist ein Verkauf: Die Vorschau nennt den garantierten Auszahlungsbetrag
und den Besitzverlust. Vor Bestätigung kann man abbrechen; danach gibt es kein
kostenloses Zurückziehen. Solange verfügbar, darf auch der frühere Besitzer das
Angebot zum angezeigten Preis zurückkaufen. Preise werden automatisch berechnet,
ohne eigene Preisangabe oder Verhandlung.

Erste Preisregel:

```text
B = Katalogpreis des Grundfahrzeugs
U = Summe der Katalogkosten aller daran gekauften Upgrades
    (beide Motorstufen zählen mit ihren jeweiligen Kosten; Farbe zählt nicht)
Ankauf/Auszahlung = abrunden(0,70 × B + 0,50 × U)
Marktplatz-Kaufpreis = aufrunden(0,85 × B + 0,75 × U)
```

Deaktivierte, aber mitverkaufte Upgrades zählen ebenfalls genau einmal.
Ein Corsa mit Motor Stufe 1 für 1.150 Coins und Carbon-Kit für 1.840 Coins hat
`B = 23.000` und `U = 2.990`: Er bringt sofort **17.595 Coins** ein und wird
für **21.793 Coins** angeboten. Gebrauchtkauf spart gegenüber denselben Neukäufen;
sofortiger Wiederverkauf bringt weniger als der Kauf gekostet hat.

Die Formel ignoriert den tatsächlich gezahlten Kaufpreis, Fahrstrecke und Anzahl
der Vorbesitzer. Weitere Besitzer mindern den Wert nicht erneut. Einstellen,
Zurückkaufen, Upgrade-Wechsel und Wiederverkaufen dürfen keinen Coin-Gewinn
erzeugen. Die Differenz zwischen An- und Verkauf bleibt beim Markt; weitere
Gebühren gibt es nicht. Der garantierte Ankauf benötigt kein begrenztes Händlerbudget.

Katalog und Preisregel sind versioniert und bleiben im ersten Experiment fest.
Bei späteren Änderungen müssen Angebote und Wiederverkaufswerte gemeinsam
migriert werden, damit keine Gewinnschleife zwischen alten Kauf- und neuen
Ankaufspreisen entsteht. Veraltete Preisvorschauen verlangen eine neue Bestätigung.

Die Liste zeigt Modell, Farbe, Reifen, Tuning, Fahrwerte und Kaufpreis.
Verkaufte Angebote verschwinden. Wenn zwei Spieler dasselbe Auto kaufen, gewinnt
genau eine Transaktion; der andere behält seine Coins und erhält einen Hinweis.
Ein leerer Markt ist erlaubt, der Fahrzeugkatalog bleibt verfügbar.

## Speicher und Umfang des Experiments

Jedes Fahrzeug besitzt eine stabile ID, Modell-ID, Besitzer, Farbe, gekaufte und
aktive Upgrades sowie eine Datenversion. Das Profil speichert aktive Fahrzeug-ID
und einmalige Startauto-Vergabe. Kontostand, Eigentum und Angebote bleiben durch
gemeinsame Transaktionen konsistent. Bei Fehlern wird kein Erfolg vorgetäuscht;
nach Verbindungsabbruch wird derselbe Transaktionsstatus erneut abgefragt.

Das lokale Spielerprofil allein ermöglicht keinen geräteübergreifenden Markt.
Ein gemeinsamer Dienst mit authentifizierten Spieleridentitäten und verbindlichen
Kontoständen gehört deshalb zum vollständigen Zielbild. Lokal bearbeitbare Coins
dürfen nicht ungeprüft in diesen Markt gelangen. Kontoverknüpfung und eine
vertrauenswürdige Vergabe von Fahrt-Coins müssen vor Online-Handel geklärt sein;
ein frei vom Client gemeldeter Meterwert reicht als Nachweis nicht aus.

Zwei prüfbare Schritte begrenzen das Experiment: zuerst Garage, alle zehn Modelle,
Tuning und Ankaufregeln mit lokalen Testprofilen; anschließend ein gemeinsamer
Testmarkt mit zwei getrennten Spielclients und verbindlicher Verbuchung. Der
erste Schritt erprobt Bedienung und Wirtschaft, erfüllt aber noch nicht den
Handel zwischen Spielern auf verschiedenen Geräten. Der vollständige Pitch ist
erst mit dem zweiten Schritt erfüllt.

Fahrparameter bleiben Teil der headless testbaren Simulation; Darstellung liest
die Konfiguration. Speicher und Marktanfragen warten nicht im Fixed-Step.
Konkrete Speicherformate und die Vertrauensgrenze zum Marktdienst werden vor
Implementierung in vorgeschlagenen [ADRs](../../architecture/decisions/README.md)
festgehalten. Die [Architekturdokumentation](../../architecture/README.md) wird
mit der Umsetzung ergänzt.

Erfolgskriterium: Ein Spieler kauft und konfiguriert ein Fahrzeug, fährt mit
verändertem Fahrgefühl und erhält beim Einstellen sofort Coins. Ein zweiter
Spieler kauft genau dieses Fahrzeug mit allen Änderungen. Nach Programmneustart
sehen beide den korrekten Besitz und Kontostand.

Gezielte Prüfungen: einmalige Corsa-Vergabe, Schutz des letzten Fahrzeugs, zu wenig
Coins, Upgrade-Wechsel ohne Doppelkauf oder gestapelte Boni, Rundung, gewinnfreie
Kauf-Verkauf-Zyklen, keine zweite Auszahlung beim späteren Kauf, konkurrierende
Käufe und Wiederholung nach Abbruch ohne doppelte Buchung. Spielproben prüfen
Silhouetten und Konfigurationen, insbesondere die kleinsten, größten und
schnellsten Varianten bei höchster Schwierigkeit. Menüs müssen mit Tastatur und
Controller bedienbar sein.

# Security

Der gemeinsame Markt führt Netzwerkzugriffe und fremde Besitzdaten ein. Der
Dienst prüft Identität, Eigentum, Kontostand, Modell- und Upgrade-IDs und berechnet
Preise selbst. Der Client gibt weder Preise noch Eigentümer verbindlich vor.
Coin-Beträge sind begrenzte ganze Zahlen; ungültige oder überlaufende Werte
werden abgewiesen.

Kauf und Ankauf erhalten eindeutige Transaktions-IDs und werden atomar und bei
Wiederholung nur einmal verbucht. Gleichzeitige Anfragen dürfen dasselbe Auto
nicht mehrfach verkaufen oder denselben Kontostand mehrfach ausgeben. Ein
Profil-Reset setzt die Startauto-Vergabe nicht zurück. Das Farmen kostenloser
Startautos über viele Konten ist vor öffentlichem Handel gesondert zu begrenzen,
ebenso manipulierte Fahrtgutschriften.

Angebote enthalten geprüfte Katalogdaten und Konfigurationen, keine hochgeladenen
Bilder, Freitexte oder Kontaktdaten. Zugangsdaten gehören nicht in Angebote oder
Logs. Offline-Spielstände und Testkontostände sind vom verbindlichen Online-Markt
getrennt; Offline-Anfragen erzeugen keine vorläufig ausgebbaren Verkaufserlöse.

# Rabbit Holes

- **Reale Preise und Sparzeiten:** Vom Spielzeug bis zum Millionenfahrzeug reichen
  die Ziele. Frühe Upgrades und Verdienst zuerst erproben, bevor ein langfristiger
  Grind festgeschrieben wird.
- **Zehn realistische Simulationen:** Unterschiede auf wenige Arcade-Parameter
  begrenzen, ohne Getriebe-, Drehmoment- oder Reifenphysikmodell.
- **Zu große oder schnelle Fahrzeuge:** Größen und Parametergrenzen gemeinsam
  mit Verkehr und Pickups testen; teuer darf nicht unspielbar bedeuten.
- **Sofortankauf erzeugt Coins ohne Käufer:** Feste Abschläge, konsistente
  Upgrade-Bewertung und Schutz vor duplizierten Fahrzeugen sind erforderlich.
  Eine Nachfragebörse ist nicht erforderlich.
- **Online-Handel als Nebenprojekt:** Identität, Coin-Vergabe und Transaktionen
  sind echte Voraussetzungen. Einen lokalen Testmarkt nicht als fertigen
  gemeinsamen Handelsplatz ausgeben.
- **Unbegrenzte Varianten:** Pro Modell eine Grundform, Farbpalette, drei
  Reifensätze und ein Carbon-Kit; keine vollständigen Herstellerkonfiguratoren.

# No-Gos

Kein Echtgeld, Coin-Kauf, Geld-Auszahlung, Kredit oder Leasing. Keine Auktionen,
freien Verkaufspreise, Verhandlungen, Spieler-Chats oder direkter Tausch.
Kein Warten auf einen Käufer vor Auszahlung: Der Markt garantiert den Ankauf.
Keine künstlichen Käufer, die einen erfolgreichen Spielerverkauf vortäuschen.

Keine neuen Strecken, Wettereffekte, Driftphysik, Reparaturen, Reifenabnutzung
oder Treibstoffkosten. Keine zufälligen Tuningteile, Lootboxen oder verschlechterten
Gebrauchtfahrzeuge. Keine zusätzliche Währung und kein Coin-Multiplikator allein
für den Besitz eines teuren Autos. Kein Fahrzeugwechsel mitten in der Fahrt.
