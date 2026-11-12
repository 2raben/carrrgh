# Problem

In [Car Drive](../car-drive/pitch--car-movements.md) kennt eine Fahrt nur eine
Richtung: Jeder Zusammenstoß kostet ein Herz, und verlorene Herzen kommen nie
zurück. Nach dem ersten Fehler wird jede Fahrt deshalb nur noch vorsichtiger;
es gibt keinen Anlass, für eine Belohnung ein Risiko einzugehen. Lange Fahrten
enden oft an einem einzelnen frühen Treffer statt an der wachsenden
Schwierigkeit.

Seltene Fundstücke auf der Straße sollen kleine Entscheidungen schaffen:
Lohnt sich der Spurwechsel durch den Verkehr, um ein Herz oder ein Schild
einzusammeln? Dieser Pitch ergänzt Car Drive; der ursprüngliche Pitch schloss
Power-ups für den ersten Prototyp bewusst aus.

**Status: umgesetzt.** Die [Architekturdokumentation](../../architecture/README.md#hearts-and-shields)
beschreibt die konkreten Regeln und Anfangswerte; Seltenheit und Lesbarkeit bleiben
Spieltestaufgaben.

# Solution

Zwei Arten von Fundstücken erscheinen zufällig und selten auf der Straße. Man
sammelt sie ein, indem man darüberfährt. Sie bewegen sich wie die Fahrbahn, also
mit der eigenen Geschwindigkeit nach unten, und verschwinden unten aus dem Bild,
wenn man sie verpasst.

## Power-Up: ein Herz zurück

Ein Power-Up gibt beim Einsammeln sofort ein verlorenes Herz zurück. Mehr als
die drei Startherzen gibt es nicht. Ein Power-Up erscheint deshalb nur, solange
mindestens ein Herz fehlt; mit vollen Herzen bleibt die Straße frei davon. Ist
man inzwischen wieder bei drei Herzen, wenn man ein bereits sichtbares Power-Up
erreicht, fährt man ohne Wirkung darüber und es bleibt liegen.

## Schild: der nächste Treffer ist frei

Ein eingesammeltes Schild wird aufbewahrt, bis es gebraucht wird. Bei der
nächsten Kollision verbraucht es sich statt eines Herzens. Der übliche kurze
Trefferschutz beginnt trotzdem, damit dieselbe Berührung nicht unmittelbar
danach doch ein Herz kostet. Das Schild wirkt automatisch; es gibt keine
zusätzliche Taste und keine Zeitbegrenzung. Es schützt auch beim letzten Herzen
vor dem Spielende.

Man kann höchstens ein Schild aufbewahren. Solange eines gehalten wird, erscheint
kein neues. Erreicht man ein bereits sichtbares Schild mit vollem Speicher,
fährt man ohne Wirkung darüber und es bleibt liegen. Ein Zusammenstoß während
des laufenden Trefferschutzes verbraucht das Schild nicht.

## Erscheinen

Fundstücke sind selten: im Mittel etwa eines auf mehrere hundert Meter, mit
zufälligem Abstand. Der genaue Wert ist ein Spieltestwert; ein guter Startpunkt
ist, dass man in einer typischen Fahrt nur eine Handvoll sieht. Es ist immer
höchstens ein Fundstück gleichzeitig auf der Straße. Welche Art erscheint,
entscheidet der Zufall unter den gerade erlaubten Arten; ist keine erlaubt
(volle Herzen und ein gehaltenes Schild), erscheint nichts.

Ein Fundstück erscheint wie der Verkehr außerhalb des sichtbaren Bereichs auf
einer Spurmitte, aber nicht auf oder direkt neben einem Auto derselben Spur.
Es muss erreichbar sein, ohne dass ein Treffer unvermeidlich ist; Risiko entsteht
durch den Weg dorthin, nicht durch eine Falle. Der Zufall verwendet einen
setzbaren Startwert wie der Verkehr, damit Tests reproduzierbar bleiben.

Das Einsammeln nutzt dieselbe überstrichene Kollisionsprüfung wie der Verkehr,
damit auch mit Boost kein Fundstück zwischen zwei Simulationsschritten
übersprungen wird. Fundstücke selbst verursachen nie Schaden. Ein Neustart
entfernt alle Fundstücke und das gehaltene Schild.

## Darstellung

Power-Up und Schild sind auf einen Blick voneinander und von Autos
unterscheidbar, zum Beispiel ein rotes Herz und ein blaues Schildsymbol. Das
HUD zeigt neben den Herzen, ob ein Schild bereit ist. Am Auto zeigt eine eigene
Markierung das gehaltene Schild; sie unterscheidet sich vom goldenen Umriss des
Trefferschutzes. Das Verbrauchen des Schilds ist als kurzer, deutlicher Moment
erkennbar.

## Umfang des Experiments

Das Experiment ist spielbar, wenn man Fundstücke früh genug erkennt, bewusst
entscheiden kann, ob man sie holt, und die Wirkung von Herz und Schild im HUD
nachvollziehen kann. Automatisierte Simulationstests sollen prüfen: Ein
Power-Up gibt genau ein Herz zurück und nie mehr als drei; ein Schild verhindert
genau einen Herzverlust und startet den Trefferschutz; höchstens ein Schild wird
gehalten; gesperrte Arten erscheinen nicht; Einsammeln funktioniert auch bei
vollem Boost; Fundstücke erscheinen nicht auf Autos; der Neustart setzt alles
zurück; derselbe Startwert erzeugt dieselben Fundstücke. Eine Spielprobe prüft
Seltenheit, Lesbarkeit und ob sich das Einsammeln als lohnende Entscheidung
anfühlt.

Die [Architekturdokumentation](../../architecture/README.md) wird bei der
Umsetzung um die tatsächlichen Regeln und Anfangswerte ergänzt.

# Security

Fundstücke entstehen lokal aus dem spieleigenen Zufallsgenerator. Es gibt kein
Netzwerk, keine gespeicherten Daten und keine externen Inhalte. Dieser Pitch
führt keine Verarbeitung nicht vertrauenswürdiger Eingaben ein.

# Rabbit Holes

- **Unfaire Platzierung:** Ein Fundstück zwischen dicht folgenden Autos lockt in
  einen unvermeidlichen Treffer. Nur Positionen mit Abstand zum Verkehr derselben
  Spur zulassen und mit den bestehenden Verkehrsregeln testen, statt eine
  allgemeine Erreichbarkeitsplanung zu bauen.
- **Seltenheit und Balance:** Zu häufige Herzen machen die Fahrt endlos, zu
  seltene machen sie bedeutungslos. Mit einem einfachen zufälligen
  Streckenabstand beginnen und im Spiel justieren; keine Abhängigkeit von
  Schwierigkeitsstufen oder Leistung im ersten Schritt.
- **Schild und Trefferschutz:** Die Reihenfolge von Schild, Schutzzeit und
  „jedes Auto trifft nur einmal“ muss eindeutig sein. Ein Treffer bleibt ein
  Treffer; das Schild ersetzt nur den Herzverlust.
- **Zufall in Tests:** Ein eigener oder geteilter Zufallsgenerator darf
  bestehende Verkehrstests nicht unbemerkt verändern. Startwerte in Tests
  explizit setzen.

# No-Gos

Keine weiteren Power-up-Arten wie Magnet, Zeitlupe, Unverwundbarkeit,
Boost-Auffüllung oder Punktebonus. Keine Herzen über drei, kein Stapeln von
Schilden, keine aktive Schild-Taste und keine zeitlich begrenzten Effekte. Kein
Inventar, Shop, Upgrades oder freischaltbare Inhalte. Keine Fundstücke, die
Schaden verursachen oder den Verkehr beeinflussen.
