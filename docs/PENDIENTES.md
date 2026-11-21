# Pendientes

Lo que falta para dar el emulador por terminado, en tres frentes: las paradas
que el núcleo todavía no hace, el pupitre web con todos sus controles
funcionando y más programas para correr. Escrito el 20/11/2026. Cada punto
dice qué se verificó en el código y qué falta averiguar en las fuentes. La
regla de siempre: lo que el manual no respalda se marca `Supuesto:`.

## 1. Paradas del núcleo

CS 225 A §7.5 enumera siete motivos de parada. Estado en
`crates/mercury-core/src/cpu.rs`:

| Parada (§7.5) | Estado | Qué hacer |
|---|---|---|
| (i) instrucción 99 o parada prepulsable | ✅ `Stop::Stop99`, `Stop::PrepulseStop` | `Machine::prepulse` continúa en la instrucción siguiente. |
| (ii) falla de paridad | ✅ `Stop::Paridad` | Núcleos y tambor. Se comprueba al buscar una instrucción, leer un operando o transferir un bloque. Supuesto: queda pendiente la operación fallida y el bloque se copia entero. |
| (iii) desborde del acumulador | ✅ `Stop::Overflow` | Enciende STOP FLIP FLOP y `Machine::prepulse` sigue en la instrucción siguiente. |
| (iv) parada opcional | ✅ `Stop::OptionalStop` | Con `Io::handswitch_stop` en ON, para tras obedecer la instrucción en `Io::address_keys`. Excepciones de §7.5: `60`/`62`/`63` paran dos veces (`Supuesto:` antes y al completar la E/S); `68`/`69` paran sin transferir el tambor (`Supuesto:` 60 µs). |
| (v) sector inexistente | ✅ `Stop::NonExistentSector` | `Drum::get`/`get_mut` verifican rango en `68`/`69` sin envolver módulo `len`. Sólo `Machine::initial_transfer` la levanta, no `PREPULSE`. |
| (vi) parada por bucle | ✅ `Stop::LoopStop` | Detecta saltos puros tomados hacia sí mismos (`59`, `08`, `09`, `28`, `29`, `48`, `49`, `180`); excluye `380 *` y `18b *` (`b ≠ 0`) porque modifican estado y el *Input Routine* los usa como espera. Sólo `Machine::initial_transfer` la levanta. |
| (vii) SINGLE | ✅ `Stop::Single` | Con `Io::single_mode`, detiene la unidad de control y obedece una instrucción por `PREPULSE` (dos prepulsos en `60`, `62` y `63`). |

Otras cosas del núcleo:

* **Seguir después de una parada.** ✅ Implementado en el núcleo, la consola
  de texto y el pupitre web. Se puede cambiar de modo, mover las llaves,
  pulsar PREPULSE o INITIAL TRANSFER y poner cinta en el lector durante
  una parada. Cubierto por `paradas.rs`, `sesion.rs` y los tests de WASM.
* **Instrucciones 80–89** (tarjetas, impresora de líneas, cinta magnética):
  devuelven `Stop::Unimplemented` (`cpu.rs`). No se sabe si Clementina
  tenía esos equipos.
* **Función `31b`** del manual *Convencional*: `probar31 B N` permite probar
  la operación documentada. Faltan el código de siete bits y su tiempo.
* **Modificación de la UBA**. Implementada en **Clementina 1966**
  (`--clementina-1966` en el CLI) con cuatro tambores. `65` y `66` eligen
  la mitad para `00`–`07`, `20`–`27`, `60` y `61`. Durán documenta la
  mitad baja al comenzar un programa. Siguen pendientes la confirmación
  de `63`, los cruces por modificación B y el efecto de las maniobras
  del pupitre sobre la selección. Ver [Fuentes](FUENTES.md).

Cada parada tiene su test en `crates/mercury-core/tests/paradas.rs`, y `cargo test --workspace` verifica que el *Input Routine*, TINY y los Quickies siguen andando.

## 2. El pupitre: que funcione todo

### Para qué: en el modo 1961 se puede meter la pata

El objetivo es que en el modo «Como en 1961» el operador pueda hacerlo mal y
que cada error tenga la consecuencia que habría tenido en la máquina, sin
carteles que lo frenen. Por eso cada control tiene que funcionar de verdad.

✅ Resuelto compilando [`crates/mercury-wasm`](../crates/mercury-wasm) a
WebAssembly (sin dependencias externas, con `mercury-core`, `mercury-autocode` y
los 62 sectores no vacíos del *Input Routine* de 1959 embebidos) y conectando
la consola unificada [`web/index.html`](../web/index.html) +
[`web/servidor.py`](../web/servidor.py) directamente al núcleo en vivo:

* **INITIAL TRANSFER sin cinta:** queda esperando en la primera lectura con la
  instrucción retenida (`Stop::InputExhausted`); al poner la cinta después,
  reanuda.
* **MANUAL, SINGLE, HANDSWITCH STOP, WRITE CURRENT y llaves J/D/F:** todos
  cambian en vivo en plena corrida y tienen efecto inmediato (`WRITE CURRENT`
  en OFF hace caer el *tele-input* en el bucle `280 60`; las llaves `INPUT` en
  0 paran el *Input Routine*; `HANDSWITCH STOP` en `ADDRESS` detiene con parada
  opcional).
* **La hoja de operación no bloquea.** Muestra los pasos de §7.4 y §7.5 según
  el programa y explica qué ocurrió ante cualquier parada o error, sin impedir
  ninguna maniobra.

### Control por control

Fuente: Apéndice VII y §7.4–§7.5 de CS 225 A. ✅ anda · ❌ fuera de alcance / sin fuente.

| Control | Qué hace según el manual | Estado | Detalles / supuestos |
|---|---|---|---|
| INITIAL TRANSFER | Sector 0 a la página 0, arranca en 0 (§7.4) | ✅ | En núcleo (`Machine::initial_transfer`), `--interactivo` y web WASM; levanta paradas (v) y (vi). |
| PREPULSE | Sigue después de una parada; en SINGLE, una instrucción; en MANUAL, obedece las llaves | ✅ | En núcleo (`Machine::prepulse`), `--interactivo` y botón `PREPULSE` en `web/index.html`. |
| CLEAR TAPE | El Apéndice VII lo nombra pero no lo explica | ❌ | Sin explicación en CS 225 A ni en su índice; botón inactivo con nota en pantalla. |
| MANUAL/AUTO | En MANUAL se obedece la instrucción armada en FUNCTION y ADDRESS con PREPULSE (§7.5) | ✅ | `Stop::Manual`, `Machine::manual_instruction` y palanca en vivo en `web/index.html`. |
| HANDSWITCH STOP | Parada opcional (iv) en `ADDRESS` | ✅ | `Stop::OptionalStop` (incluye excepciones en `60/62/63` y `68/69`) en núcleo, CLI y web. |
| SINGLE/CONTINUOUS | Parada (vii), paso a paso con PREPULSE | ✅ | `Stop::Single` (dos prepulsos en `60/62/63`; retoma al volver a `CONTINUOUS`). |
| Llaves FUNCTION (J) | Mitad alta de la instrucción manual | ✅ | `Io::function_keys`, CLI `--interactivo` y fila `J` en vivo en `web/index.html`. |
| Llaves ADDRESS (D) | Mitad baja de la instrucción manual y dirección de parada opcional | ✅ | `Io::address_keys`, `--parada-opcional` / `--interactivo` y fila `D` en vivo. |
| Llaves INPUT (F) | Las lee la instrucción 61 | ✅ | Cambian en vivo en `--interactivo` y en `web/index.html` (Primos, *tele-input*, Nim). |
| WRITE CURRENT | Permite escribir el tambor con la 69 | ✅ | `Io::write_current` en vivo (`Supuesto:` tarda igual sin alterar el sector). |
| HOOT y VOLUME | Parlante de la instrucción 58 | ✅ | En CLI (`--wav`) y en web con WebAudio + palanca `HOOT` y perilla `Volume` interactiva. |
| Selectores DISPLAY | Eligen qué registro muestra cada tubo (García Camarero 1962, §2.5) | ✅ | Perillas interactivas en `web/index.html`: ciclan `B1–B7`, `B1–B4`, `Display A, E` (orden `64`) y `A, E, I, C` (`Supuesto:` orden de las posiciones). |
| Visores | Muestran registros y la salida de `64` | ✅ | Traza de pulsos rectangulares hacia arriba por cada 1 (Durán 2018, págs. 35 y 106; TEC cap. 1 los nombra como puntos brillantes). |
| INHIBIT PARITIES, PARITY STOP | Paridad | ✅ | La llave habilita el reintento con PREPULSE. El diagnóstico en **Por dentro** invierte un bit de paridad. |
| FUSE ALARM B.U., MAG. | Alarma de fusibles, cinta magnética | ❌ | Fuera de alcance; documentado en el panel. |
| SAC TEST, B TEST, SHIFT ≥31 | Lámparas de prueba | ✅ | En núcleo y web (`Supuesto:` el sentido desde CS 225 A cap. 3). |
| ACC/MULT. | Alarma | ✅ | Enciende con `Stop::Overflow` (`Supuesto:`). |
| STOP FLIP FLOP | Paradas (i) a (iv) | ✅ | `Machine::stop_flip_flop()` y lámpara `lStop` en `web/index.html`. |
| Las cuatro lámparas sin rótulo del Apéndice VII | Sobre MANUAL/AUTO y WRITE CURRENT, y entre la 2.ª y la 3.ª hilera de esas dos columnas | ❌ | Sin documentación en CS 225 A; no se dibujan. |
| OFF | Apaga | ✅ | Corta la corrida, silencia WebAudio y conserva núcleos y tambor (`Supuesto:` limpia registros volátiles al cortar alimentación). |

## 3. Más programas para Clementina

### Consola web unificada

✅ Reemplazado `ports/tiny/en_vivo.html` por [`web/index.html`](../web/index.html),
[`web/servidor.py`](../web/servidor.py) y [`web/catalogo.json`](../web/catalogo.json):

* **Catálogo JSON.** Define carga (`escrito`, `autocode`, `input_routine`,
  `tele_input`, `midi`, `escrito_tambor`), código de salida (`ferranti`,
  `ita2` o `longitudinal`), llaves,
  hoja de operación y salidas esperadas, validadas automáticamente por
  `crates/mercury-wasm/tests/catalogo.rs`.
* **«Por dentro» genérico.** Muestra los 512 o 1024 sectores del equipo
  elegido y las 32 páginas de núcleos activas por lote. Para TINY 2.1
  conserva además el guion y las etiquetas de sus bancos y overlays.
* **Selector de programa.** Sincronizado entre «Modo fácil» y «Como en 1961»,
  con controles adaptados (llaves `F`, cinta de datos, partida de Nim o TINY).

### Programas que ya están en el catálogo

| Programa | Archivo / cinta | Estado |
|---|---|---|
| Fibonacci | `examples/fibonacci.mer` | ✅ en catálogo y verificado en WASM |
| Calculando primos (Ferranti e ITA2) | `examples/primos_ferranti.mer`, `examples/primos.mer` | ✅ llaves `F` en vivo y decodificación Ferranti/ITA2 |
| Un nombre en la cinta (ejemplo) | `examples/nombre_ejemplo.mer` | ✅ perfora `CLEMENTINA` (sin datos personales de la cinta 3) |
| *My Darling Clementine* | `examples/clementine.mer` | ✅ corre a `×1` sonando por WebAudio (`HOOT`) |
| Conversor MIDI | Melodía de prueba, `.mid` + guion y variantes locales | ✅ selección de pista y canal, transposición, separación, HOOT y perforación |
| Fibonacci, integral de Sievert y contornos | `samples/autocode/fib2.mer`, `int.mer`, `contour.mer` | ✅ compilados por `mercury-autocode` en WASM (`contour.salida.txt` verificado) |
| *Quickies* por el *Input Routine* (1959) | `quicky_raiz` (Q12), `quicky_seno` (Q6), `quicky_reciproco` (Q1) | ✅ cargan y corren desde el tambor por `INITIAL TRANSFER` real |
| TINY 2.1 (`compacto`, `ternario`, `3bit`) | variantes opcionales en `ports/tiny/` | ✅ detectadas si están los checkpoints/cintas y corridas en WASM |

### Programas para reconstruir de las fuentes

Son reconstrucciones nuevas y hay que decirlo así: no se conserva el código
original de ninguno.

* **El juego del Nim (Marienbad).** ✅ Reconstrucción nueva en
  [`examples/nim.mer`](../examples/nim.mer) a partir de Durán (2018, págs. 33–36)
  con tests en `crates/mercury-core/tests/nim.rs` e interfaz interactiva en la
  consola web:
  * lee palillos iniciales (0–10) y jugadas desde las llaves `INPUT (F)` tras cada `990`;
  * dibuja las 4 filas como pulsos en `B1–B4` y con `64` (`Display A, E`);
  * perfora mensajes longitudinales de 5 canales de alto (`JUEGUE QUE YO LE GANARE`, `MI POSICION ES MALA PERO CUIDESE`, `QUITE ... DE LA FILA ...`, `NO JUEGO MAS PORQUE HIZO TRAMPA`, `GANE ! QUIERO OTRO` y `Supuesto:` derrota `USTED GANO ! QUIERO REVANCHA`);
  * con posición segura la mantiene por `nim-sum`; con posición insegura quita un único palillo.
* **Balística, tabulación 37** (trabajo n.º 66 del *Informativo del Instituto
  de Cálculo*). Sólo se conoce el encabezado impreso que se ve en el video.
  Haría falta el problema para reconstruirlo.
* **Ejemplos de los manuales.** Los de García Camarero (*AUTOCODE*, 1961, y
  *Convencional*, 1962) y los del *Mercury Autocode Manual*. Las pruebas de
  Autocode cubren la cinta de dirección, un tramo del ejemplo de Runge-Kutta
  y mínimos cuadrados. Quedan por incorporar otros ejemplos como programas
  listos para correr en el catálogo.
* **R4440, ecuaciones diferenciales** (Gibbons, 1961,
  [ficha en Fuentes](FUENTES.md#autocode)). Es un programa de biblioteca y la entrada se
  parece al Autocode.
* **El intérprete de la CEUNS** (serie de TEC, cap. 3). No hay fuente; sólo
  se sabe que existió.

## 4. Diseño: un poco de relieve

El pupitre plano quedó bien, pero a algunas piezas les vendría un toque de
volumen: el vidrio de los tubos, los botones, las palancas y las lámparas.
Hay que hacerlo dentro de Hallmark, sin volver a la imitación de la versión
anterior:

* Profundidad **por escalones de luminosidad y sombras con desplazamiento y
  desenfoque** (regla 29). Nada de halos de color ni brillos.
* Nada de biseles ni cromados en degradé que finjan metal (regla 47c).
* Un token `--shadow-*` por nivel de relieve. Usar los mismos en todo el
  pupitre.
* Candidatos:
  * los tubos, con una sombra interior que dé la curva del vidrio;
  * los botones y la perilla, con una sombra corta abajo;
  * las palancas, con una sombra que siga la posición arriba o abajo;
  * el borde de la cuña, con un escalón más oscuro.
* Al terminar, `sloplint` tiene que seguir en 0 FAIL sin eximir la 47c.
* ✅ **Elegido (21/11/2026): luz cenital.** Tokens `--shadow-1`,
  `--shadow-2`, `--shadow-palanca`, `--shadow-hundido` y `--borde-cuna` en
  `web/index.html`. La sombra de las palancas es un `drop-shadow` sobre la
  palanca ya girada, así que sigue a la tapa arriba o abajo. Falta correr
  `sloplint`, que no se corrió.
* **Excepción: los tubos.** Para darles personalidad se admite un
  resplandor leve del fósforo (3 px), una curvatura suave de la traza y el
  borde del vidrio más oscuro, aunque la regla general pide no usar
  brillos. Sin líneas de barrido: los visores trazaban los pulsos, no eran
  de TV.

## 5. Otros pendientes

* **Sitio estático.** ✅ `tools/armar_web.py` arma `dist/` y está publicado
  en <https://clementina.pages.dev/> (Cloudflare Pages).
* **WASM.** ✅ Resuelto en [`crates/mercury-wasm`](../crates/mercury-wasm) (sin
  dependencias externas, compilado con `cargo build -p mercury-wasm --target wasm32-unknown-unknown --release`).
* **El *Input Routine* bit a bit.** Del Dago cotejó los sectores 0 y 1
  contra la cinta de COMIC. Conseguir esa digitalización permitiría repetir
  el cotejo en este emulador. Los demás sectores siguen pendientes.
* **COMIC y AIC.** La cinta maestra de COMIC se cargó por *Tele-input* en
  el emulador de Del Dago con cuatro tambores (1024 sectores). El CLI puede
  arrancar en esa configuración y espera un byte de 0 a 31 por fila. Si
  llega el archivo de SAMCA, hay que identificar su formato, preservar los
  metadatos y comprobar los 176 bloques contra el tambor. El CLI reconoce
  el aviso `30` al entrar en la bocina del sector 1 y puede guardar el
  tambor con `--guardar-tambor`. La cinta original sigue pendiente. El
  ejemplar del manual de 1966 catalogado en la UADER
  puede ayudar a reproducir el lenguaje y preparar una cinta fuente de
  prueba. La copia de julio del COMIC y la del AIC son otros dos rollos
  entregados a Exactas, con digitalización pendiente de confirmar.
* **Biblioteca histórica de Autocode.** Las operaciones de los manuales ya
  se traducen. Queda incorporar fuentes de los subprogramas de biblioteca
  que se conserven.
* **TINY.** Los modelos están fuera del repositorio (`~/Downloads/tiny68k`);
  las cintas armadas quedan en `target/tiny/`.
* **Revisión de Hallmark.** Falta la revisión final en contexto limpio, con
  capturas (`sloplint --render` necesita puppeteer).
