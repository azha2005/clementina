# Clementina

Emulador de **Clementina**, la primera computadora científica de la Argentina:
una **Ferranti Mercury** instalada en el Instituto de Cálculo de la FCEN-UBA,
en servicio desde el 15 de mayo de 1961 hasta 1970 (según la serie documental
*Clementina* de TEC; otras fuentes dicen 1971).

Hay al menos dos proyectos independientes de emulación de Mercury. Gustavo
del Dago documentó la carga de una cinta histórica de COMIC y el comienzo de
la traducción en el emulador de SAMCA. Julián A. Lell publicó una
implementación parcial en Python. La Computer Conservation Society también
describió un emulador en desarrollo para la réplica del Museo de Informática.
Las referencias y el estado de acceso están en [Fuentes](docs/FUENTES.md).

## Estado

| Parte | Estado |
|---|---|
| Almacén de núcleos (2048 × 20 bits) y tambor (512 sectores por omisión, como en la configuración de 1962) | ✅ |
| Paridad par de núcleos y tambor, parada, PREPULSE e INHIBIT PARITIES | ✅ |
| Tabla decimal 00–99 y códigos de función documentados en el Apéndice V | ✅ |
| Aritmética de punto flotante bit a bit (redondeo "forzar un 1", estandarización, dígito extra) | ✅ verificada con los ejemplos del Apéndice I |
| Registros B, sac, saltos, exponente, E/S multicanal 90–97 | ✅ (no se sabe si Clementina tenía 90–97) |
| Cinta de papel (lector, perforadora, código de 5 canales con cifras/letras) | ✅ |
| Parlante (instrucción 58) exportado a WAV | ✅ |
| Tiempos de instrucción, rotación del tambor, velocidad de lector y perforadora | ✅ (velocidad de la perforadora: supuesta) |
| Ensamblador de la *forma escrita* (`420 32`, `017 32+`, `(1`, `v1`, `+1,12`, `=n`) | ✅ (en el anfitrión) |
| *Input Routine* original corriendo en el emulador | ✅ Imagen reconstruida de los sectores 0–63. Traduce cintas en forma escrita, con etiquetas, saltos entre capítulos y Quickies. |
| Compilador de Autocode (en el anfitrión, rutinas en código Mercury) | ✅ capítulos y programas numerados, memoria auxiliar, complejos, doble longitud, integración y matrices |
| Tarjetas, impresora de líneas, cinta magnética (80–89) | ❌ no emulados |
| Consola gráfica (pupitre con monitores y llaves), versión web | ✅ pupitre completo del Apéndice VII en [`web/index.html`](web/index.html) corriendo en WASM ([`crates/mercury-wasm`](crates/mercury-wasm)), con controles en vivo (§7.4–§7.5), parlante WebAudio, catálogo de programas y modos «fácil» y «como en 1961». |

Lo que falta para terminarlo (las paradas de §7.5, cada control del pupitre
y más programas) está en [`docs/PENDIENTES.md`](docs/PENDIENTES.md).

## Uso

Hace falta Rust estable y Cargo.

```powershell
cargo build --release
```

```powershell
.\target\release\clementina correr examples\fibonacci.mer
```

```powershell
.\target\release\clementina correr examples\clementine.mer --wav clementine.wav
```

```powershell
.\target\release\clementina listar examples\fibonacci.mer
```

Opciones de `correr`: `--cinta ARCHIVO` (texto que se perfora en la cinta de
entrada), `--llaves N` (llaves de mano F), `--traza`, `--max N`, `--wav ARCHIVO`,
`--html ARCHIVO` (página que toca el WAV e imprime lo perforado en sincronía),
`--sectores N`, `--interactivo` (sesión de pupitre en texto con `PREPULSE`,
`SINGLE`, `MANUAL`, `--parada-opcional` y `--sin-escritura`).

### Consola web en vivo (WASM)

```powershell
rustup target add wasm32-unknown-unknown
cargo build -p mercury-wasm --target wasm32-unknown-unknown --release
python web/servidor.py
```

Abre `http://127.0.0.1:7661/` con el pupitre completo conectado en tiempo real al
núcleo WebAssembly y al compilador de Autocode (`web/catalogo.json`), soporte
de código Ferranti/ITA2/longitudinal, parlante `HOOT` por WebAudio y los
visores con traza de pulsos.

En **Conversor MIDI** podés arrancar con la melodía de prueba o abrir un
`.mid` y un guion `.txt` opcional en UTF-8. Elegí pista, canal, transposición
en semitonos y separación entre notas. El guion usa una línea `@SEGUNDOS`
o `@MM:SS` para indicar cuándo empieza a perforarse cada bloque. El conversor
trabaja en el navegador. Elegí **×1** y activá **HOOT** para escuchar la
reproducción. A velocidad máxima se omite la síntesis en vivo.

El catálogo también muestra los pares `.mer` + `.tambor` disponibles en
`target/still_alive/` y `target/demo_midi/`. El servidor admite sólo los
nombres `still_alive`, `still_alive_m12` y las variantes `sa_*` del conversor,
con extensiones `.mer`, `.tambor`, `.mid` y `.txt`.
Si está generado el par `still_alive.mer` y `still_alive.tambor` con la
portada mejorada, el catálogo lo muestra como **Still Alive · emblema de
Aperture**. La cinta carga el reproductor, la partitura y el guion. El
reproductor lee la partitura y el guion del tambor una vez cargados.

Los programas precargados tienen una cinta visible en el lector. El modo
fácil monta la cinta al elegir el programa y se encarga del arranque.
**Correr en Clementina** usa la velocidad seleccionada. **Acelerar carga ·
arrancar a ×1** lee la cinta a Máx y pasa a ×1 al entrar al programa. En
**Como en 1961**, usá **Poner cinta del programa**. Para las cintas
generadas, poné la llave 2 de INPUT y WRITE CURRENT en ON, pasá a AUTO
y CONTINUOUS y pulsá **INITIAL TRANSFER**. La máquina se detiene al terminar
la carga. Ajustá las llaves para el programa y pulsá **PREPULSE**. Para
volver a cargar, montá la cinta de nuevo.

En **Elecciones argentinas** podés elegir el catálogo, el total del país,
los distritos, una ficha, las diferencias o el mapa de 1958, 1962 y 1965. La
consola arma la cinta y el programa Autocode perfora el resultado.
**Acelerar consulta · volver a ×1** mantiene Máx durante la lectura de
los datos y la perforación. La velocidad vuelve a ×1 en la parada final.

En **Mapas provinciales** elegí uno de los 24 distritos de cada elección
para perforar los porcentajes provinciales y el mapa por departamento,
partido bonaerense o circunscripción de Capital Federal.

Los Quickies y TINY conservan sus cintas del Input Routine. Para forma
escrita, Autocode y MIDI, la consola perfora una cinta tele-input con el
código Mercury y las páginas y sectores necesarios. En Autocode, la
traducción se hace al preparar la cinta.

El botón **Poner cinta…** del lector acepta texto Ferranti en UTF-8 o
un `.bin` con un byte de 0 a 31 por fila. En modo **Como en 1961** podés
arrancar con el lector vacío y poner la cinta cuando la máquina la pida.
El deslizador bajo la cinta de salida permite revisar las perforaciones
anteriores.

En **Programa propio**, escribí Mercury Autocode en el editor o abrí un
archivo `.mer` o `.txt`. El texto se conserva al cambiar de modo. Podés
poner los datos del programa después de una línea con `->` o montar una
cinta de datos desde un archivo.

* En **Modo fácil**, **Correr en Clementina** compila el texto y se encarga
  de la carga y el arranque a la velocidad elegida.
* En **Como en 1961**, **Compilar y poner cinta en el lector** prepara una
  cinta binaria para *tele-input*. Subí la llave **INPUT 2**, activá
  **WRITE CURRENT**, pasá a **AUTO** y **CONTINUOUS** y pulsá **INITIAL
  TRANSFER**. En la parada de carga, pulsá **PREPULSE** para arrancar.

El selector marca **COMIC** como pendiente. Para la **Forma escrita** se
puede abrir un programa y una imagen `.tambor`. Si ya tenés una cinta de
programa, elegí **Input Routine** o **tele-input binario** y arrancá con
**INITIAL TRANSFER**.

En **Equipo**, **Clementina 1966 · 4 tambores** instala 1024 sectores y
activa las órdenes `65` y `66` de la UBA. Cambiar de equipo reinicia los
núcleos y el tambor. En **Por dentro** se ve la mitad seleccionada para
datos cortos y el mapa de sectores de la instalación elegida.

## Ejemplos

* [`examples/fibonacci.mer`](examples/fibonacci.mer): números de Fibonacci en
  código de máquina, impresos en la perforadora por restas sucesivas y una tabla
  de caracteres de cinta.
* [`examples/primos.mer`](examples/primos.mer): réplica de las dos cintas
  perforadas del Museo de Informática de CABA (`CALCULANDO PRIMOS: 2, 3, 5, 7,
  - FIN -`, en código ITA2 de teletipo). Calcula los primos menores que el valor
  de las llaves y perfora exactamente los mismos agujeros; un test lo compara
  fila por fila con la cinta leída de las fotos. Ver
  [`docs/cinta/TRANSCRIPCION.md`](docs/cinta/TRANSCRIPCION.md).

  ```powershell
  .\target\release\clementina correr examples\primos.mer --llaves 10 --codigo ita2 --svg cinta.svg
  ```

* [`examples/nombre_ejemplo.mer`](examples/nombre_ejemplo.mer) perfora `CLEMENTINA`
  en código Ferranti. [`examples/primos_ferranti.mer`](examples/primos_ferranti.mer)
  usa el mismo código para los números primos. La opción `--svg` genera
  un dibujo de la cinta a escala real.

* [`examples/nim.mer`](examples/nim.mer): reconstrucción nueva del juego del Nim
  (Marienbad) que Wilfred Durán programó para visitantes (2018, págs. 33–36).
  Pide los palillos y jugadas en las llaves `INPUT` tras paradas `99`, dibuja
  los palillos como pulsos en `B1–B4` y con `64` (`Display A, E`), y perfora
  mensajes en letras que se leen a lo largo de la cinta de 5 canales.

* [`examples/clementine.mer`](examples/clementine.mer): la canción popular
  *Clementine*, que le dio el nombre a la máquina (según un testimonio de la
  serie de TEC, la melodía "venía con el equipo"), tocada con
  la instrucción 58 (*hoot*). El período de cada nota sale de los tiempos de
  instrucción del manual: 240 + 120·k µs.

* **De MIDI a Clementina.** `clementina midi` convierte un archivo MIDI en un
  programa que toca la melodía por el parlante mientras perfora un texto
  sincronizado con la música (letra, dibujos en caracteres). El parlante da
  una nota por vez: por omisión suena la más aguda de las que están activas,
  sin el canal 10; `--pista` y `--canal` eligen otra voz. La octava se elige
  sola para que nada se desafine más de 25 cents, si se puede.

  ```powershell
  .\target\release\clementina midi cancion.mid --guion letra.txt
  ```

  ```powershell
  .\target\release\clementina correr cancion.mer --tambor cancion.tambor --wav cancion.wav --html cancion.html
  ```

  El guion es texto común; una línea `@SEGUNDOS` (o `@MM:SS`) dice desde qué
  momento de la canción sale lo que sigue, y `@@` es una arroba. Las
  minúsculas pasan a mayúsculas y los signos que no existen en el código
  Ferranti se cambian por el más parecido (`#` → `≠`, `:` → `.`). La página
  de `--html` toca el WAV y va escribiendo lo perforado al ritmo de la
  máquina.

  La partitura vive en el tambor: el reproductor ([`midi.rs`](crates/mercury-core/src/midi.rs))
  copia un sector por vez y lo recorre registro por registro. Los
  períodos van de a 60 µs, y los que perforan duran lo mismo que los que no.
  El conversor calcula el tiempo de cada instrucción y de cada transferencia
  de tambor con el mismo modelo que el emulador; un test comprueba que cada
  impulso del parlante cae en el microsegundo previsto. Límites: la
  perforadora saca 33 caracteres por segundo a lo sumo y el papel sólo
  avanza (no hay dos columnas ni se borra), así que un guion muy cargado se
  atrasa respecto de sus marcas; el conversor informa cuánto.

  *Still Alive*, el final de *Portal*, sale del reproductor de consola de
  [CATDIMA/StillAlive](https://github.com/CATDIMA/StillAlive): el
  importador lo clona en `target/` y arma el MIDI y el guion con la letra y
  el logo de Aperture como portada (la música espera a que termine de
  perforarse, unos 30 s). `--dibujos todos` intercala los diez dibujos, pero
  la perforadora no da abasto y el texto se atrasa minutos. Lo generado
  queda en `target/`, fuera del repositorio, porque la letra no es nuestra.

  ```powershell
  python tools\importar_still_alive.py target\still_alive\still_alive
  ```

  Con `--logo IMAGEN.png` la portada sale de una imagen, convertida por
  [`tools/ascii_ferranti.py`](tools/ascii_ferranti.py): elige cada carácter
  del repertorio del teleimpresor por su forma (vectores de forma de
  A. Harri, con realce de contraste) y resuelve cada línea con Viterbi para
  ahorrar cambios de registro, que en la perforadora cuestan tiempo.

  ```powershell
  .\target\release\clementina midi target\still_alive\still_alive.mid --guion target\still_alive\still_alive.txt
  ```

* [`examples/elecciones`](examples/elecciones/LEEME.md): consulta de
  resultados electorales en Autocode, con los atlas de
  [257arg](https://github.com/azha2005/257arg) en una cinta: total del país,
  ganador por distrito, ficha de un distrito, diferencia entre los dos
  primeros y mapa del ganador perforado en el teleimpresor. Por ahora trae
  las elecciones de 1958, 1962 y 1965.

  ```powershell
  .\examples\elecciones\Elecciones-Clementina.ps1 1962 mapa
  ```

## Forma escrita

Los programas se escriben como en el *Programmers' Manual* (CS 225 A, cap. 2):

```
106 100           B6 = 100
426 16.0  (1      A = A + L(16.0 + 2·B6)      etiqueta 1
136 1             B6 = Bt = B6 - 1
080 v1            si Bt ≠ 0 saltar a la etiqueta 1
```

* Función de 2 cifras decimales + dígito B, luego la dirección escrita en el
  sistema de registros medios; `+` indica la mitad derecha; `p.l` es página.línea.
* `+123.456`, `-98765`, `-123.456,25` (× 10²⁵): números largos.
* `=n`, `>n`, `≠n` (o `#n`): enteros cortos de 10 bits.
* `( ... )` al principio de una línea: texto ignorado.
* Extensiones propias: `@dir` (ubicación), `!dir` (arranque), `;` (comentario).

Ojo: los `.mer` de [`samples/autocode/`](samples/autocode/) son programas en
**Autocode** (lenguaje de alto nivel), no en forma escrita: se corren con
`clementina autocode`.

## Autocode

El *Mercury Autocode* de R. A. Brooker (Manchester, 1958) fue el lenguaje en
el que se programó casi todo en Clementina. El traductor original corría en
la propia Mercury y no se conserva. [`crates/mercury-autocode`](crates/mercury-autocode)
es una reconstrucción a partir del *Mercury Autocode Manual* (1961) y del
manual de García Camarero (UBA, 1961). El compilador corre en el anfitrión y
deja en la máquina lo que habría dejado el traductor: cada capítulo en su
lugar del tambor, sus constantes y las rutinas de apoyo. Todo lo que se
ejecuta después es código Mercury en el emulador, incluidas la división, la
impresión y la lectura de números, que están escritas en forma escrita en
[`crates/mercury-autocode/runtime/`](crates/mercury-autocode/runtime).
Para escribir programas propios desde cero, ver el
[manualcito de Mercury Autocode](docs/MANUAL_AUTOCODE.md).

```powershell
.\target\release\clementina autocode samples\autocode\contour.mer --max 200000000
```

```powershell
.\target\release\clementina autocode samples\autocode\fib2.mer --listado
```

`--listado` muestra el código Mercury generado. Si no se da `--cinta` y el
programa termina en una línea `->`, lo que sigue se usa como cinta de datos
(así vienen los programas de prueba de Edimburgo).

El programa de contornos de H. Whitfield (`contour.mer`) corre completo y
dibuja su mapa de curvas de nivel. `fib2.mer` muestra una limitación real de
la máquina: allí `r`, `s` y `t` son índices, que sólo van de −512 a 511, así
que desde el término 14 los números dan la vuelta.

**Implementado**: variables especiales, primadas y principales con sufijos
(`x3`, `xi`, `x(i-1)`), reserva `x → n`, índices, expresiones con la regla
de la multiplicación implícita y una división por término, `=` redondeado y
`»` sin redondeo, φ `int pt`, `fr pt`, `mod`, `sign`, `divide`, `parity`,
`sqrt`, `sin`, `cos`, `tan`, `exp`, `log`, `arctan` y `radius`,
`i = φ int pt(x)`, `jump` con y sin condición, `n) = 3)`, `n) = m)`,
`jump (n)`, ciclos `i = p(q)r` … `repeat` anidados, `read`, `print (x) m, n`
(punto fijo y flotante), `?`, `newline`, `space`, `hoot`, `end`, `halt`,
instrucciones de máquina (`400 (x)`, `200 (i)`, `590 (3)`, `620, 27`),
capítulos con `chapter`/`close`, `variables N`, `across`, `down`, `up`,
`title` y `psa`. Se aceptan equivalentes ASCII: `->`, `>=`, `#`, `$` (π),
`F` o `X` (φ) y `~` (»), y la forma `print (x, m, n)` de Edimburgo.

La integral de Sievert del capítulo 1 del manual (`samples/autocode/int.mer`,
que usa φ cos y φ exp) corre completa; un test compara su tabla con el mismo
cálculo en el anfitrión.

Las funciones están escritas en código Mercury en
[`runtime/`](crates/mercury-autocode/runtime): reducción del argumento y
polinomios (raíz por Newton, logaritmo por la serie de atanh). En barridos
de cientos de argumentos el error relativo máximo es de 7·10⁻⁹ (sqrt, exp,
cos), 9·10⁻⁹ (sin), 1,2·10⁻⁸ (log), 1,4·10⁻⁸ (tan) y 1,5·10⁻⁸ (arctan): unas
8 cifras significativas, cerca del límite de los 29 bits de la máquina.

Las facilidades de los capítulos 2 a 4 incluyen `φ6` y `φ7` para el
tambor auxiliar, `preserve`/`restore`, los pares complejos, `double length`,
`int step (m)` con Runge-Kutta de cuarto orden y las operaciones matriciales
`φ8` a `φ28`. También se traducen `φ poly`, `φ max/min`, `check`,
`programme -N`, `down/across m/c - N` y `variables 0`. `rmp` pasa al
siguiente `chapter 0` de una cinta de dirección ya traducida y conserva
el almacén numérico. Los programas históricos externos de la biblioteca
se pueden llamar si se cuenta con su fuente y se incluye en la traducción.

Decisiones propias, marcadas `Supuesto:` en el código donde corresponde:

* El mapa de memoria sigue el Apéndice 1 de García Camarero. Dentro de él, el
  orden de las variables especiales en la página 31 (a'…z' y luego a…z), el
  uso de B1 para los sufijos, de B2 para las vueltas y del sac para los
  índices son elecciones propias; B5 y B6 quedan para el programador.
* La impresión (198 registros), la lectura (138) y las funciones (sqrt 64,
  exp 82, log 74, sin/cos/tan 138, arctan 126) se enlazan dentro de cada
  capítulo que las usa, como si todas fueran *quickies*; la lista de quickies
  antes de `close` se acepta y no cambia nada. En el original las funciones
  que no eran quickies venían del tambor a la página 15 en cada llamada
  (unos 23 ms en vez de 6), y la impresión y la lectura probablemente
  también; las rutinas originales eran más chicas (sqrt 48, sin/cos 36,
  exp 50 registros). Un capítulo que use las dos pierde 336 de sus 832
  registros, así que un programa de la época escrito al límite puede no
  caber, y `psa` no da los mismos números que el traductor original.
* La división hace tres pasos de Newton-Raphson sobre la mantisa y una
  corrección con el resto (unos 5 ms). En un barrido de 3000 casos el error
  máximo fue de 2,6 unidades del último bit (8 cifras y media). Con el
  redondeo de la Mercury, que fuerza un 1 en el último dígito, un cociente
  entero exacto queda una unidad por debajo más o menos la mitad de las
  veces, así que `φ int pt(x/h)` puede dar uno menos; el manual ya aconseja
  `φ int pt(x + 0.5)` para el entero más cercano. El final de la división
  está en los registros 112 a 117 de la página 1.
* Para estandarizar un índice recién convertido se le suma sin redondeo un
  cero de exponente 0 (registro 56 de la página 0). Las operaciones
  redondeadas sobre números sin estandarizar agrandan el 1 forzado, así
  que los índices y los enteros de un producto se multiplican primero y sin
  redondeo, lo que es exacto.
* `print` pasa a la forma flotante desde 2²⁸ (el manual dice "más o menos
  10¹⁰", que el original lograría con doble longitud).
* φ sqrt de un número negativo da la raíz de su valor absoluto; φ log de
  cero o de un negativo y φ exp de más de 177 provocan el desborde, que
  detiene la máquina; φ sin, cos y tan sólo valen para |x| < 800, porque el
  número de cuadrantes tiene que entrar en un índice.
* `?` imprime siempre; en el manual de la UBA dependía de una llave del
  pupitre durante la traducción.
* La perforadora se supone en registro de cifras; `newline` perfora CR LF.
* `halt` se traduce como una parada, igual que `end`.
* Los capítulos traducidos comparten 22 posiciones de tambor. Se reservan
  sectores de los huecos para los capítulos 0 sucesivos y los subprogramas
  numerados. Las rutinas de apoyo sólo ocupan sectores si se usan.
* Los pares complejos y de doble longitud copian los operandos primero,
  para permitir que las variables de entrada y salida coincidan. El barrido
  de doble longitud comprueba un error escalado menor que 10⁻¹⁵ en los
  casos ensayados. Los intermedios usan el rango de exponentes de la Mercury.
* Las operaciones matriciales usan una caché de tambor en la página 30 y
  guardan sus páginas de trabajo en el depósito secreto. Las funciones
  16, 26 y 27 admiten solapamiento con hasta 384 elementos de salida.
  Las operaciones lineales admiten matrices con la misma base, y las
  diagonales se copian antes de escribir. Si un rango o una combinación
  de direcciones no se puede usar, el programa se detiene en un bucle.

El programa de contornos mostró un error del emulador: la instrucción 62
(perforar `n`) no sumaba el registro B a `n`, aunque es B-modificable. Se
corrigió; hay un test en `mercury-core`.

## Estructura

```
crates/mercury-core      núcleo sin E/S: almacén, tambor, acumulador, CPU, ensamblador, cinta
crates/mercury-autocode  compilador de Mercury Autocode y sus rutinas en código Mercury
crates/mercury-cli       consola de texto (binario `clementina`)
tools/                lectura de cintas desde fotos (Python: Pillow, NumPy, SciPy)
examples/             programas en forma escrita
docs/                 manuales y fuentes (ver docs/FUENTES.md)
```

El núcleo no depende de nada ni hace E/S, de modo que puede compilarse a
WebAssembly para una consola web.

## Supuestos

Donde el manual es ambiguo se tomó una decisión, anotada en el código como
`Supuesto:`. Las principales:

* 28 salta si St ≠ 0 y 36 es S ≠ n, como en la tabla del Apéndice V (la pág. 15
  dice otra cosa, que no concuerda con el resto del manual).
* Si la diferencia de exponentes es ≥ 32, el número mayor se entrega sin redondear.
* Un resultado cero lleva el exponente a −256.
* La perforadora trabaja a 33⅓ caracteres por segundo. El reproductor de
  MIDI depende de esto: deja al menos 30 ms entre dos perforaciones para que
  la máquina nunca espere y la nota no se corte.
* Las funciones B-modificables se tomaron del sombreado de la tabla del
  Apéndice V (un solo escaneo).
* La ubicación por omisión en la página 1 del cargador propio (`clementina
  correr`) sale de fuentes secundarias. El *initial transfer* ya sigue el
  CS 225 A §7.4: el sector 0 va a la página 0 y se entra en la línea 0.

El manual *Convencional* de Clementina (1962, §4.2) lista `31b` (`B'b = S + n`,
con actualización de `Bt`), sin indicar su función binaria. En
`--interactivo` se puede probar la operación con `probar31 B N`. Su
codificación en cinta y su tiempo siguen pendientes.

En el equipo **Clementina 1966**, `65` selecciona la mitad alta para los
operandos cortos de `00`–`07`, `20`–`27`, `60` y `61`, y `66` vuelve a la
mitad baja. El bit de cuarto sigue eligiendo dentro de esa mitad. La
búsqueda de instrucciones y las direcciones de largos conservan su lógica.
Supuesto: las dos órdenes toman 60 µs, B modifica dentro de la mitad
elegida, `63` conserva su direccionamiento y **INITIAL TRANSFER** restablece
la mitad baja. Ver [las fuentes sobre la modificación de la UBA](docs/FUENTES.md).

Para probar la paridad en el modo **Como en 1961**, hacé **INITIAL TRANSFER**
en SINGLE y AUTO. En **Por dentro**, invertí la paridad de la próxima
instrucción y pulsá **PREPULSE**. Se encienden **PARITY STOP** y
**STOP FLIP FLOP**. Volvé a invertir el bit, o activá **INHIBIT PARITIES**,
y pulsá **PREPULSE**. En `--interactivo`, `fallo nucleos CORTO BIT` y
`fallo tambor SECTOR CORTO BIT` introducen una falla. El bit 10 es el de
paridad y los bits 0 a 9 son de datos. Supuesto: la falla deja la operación
pendiente de reintento y una transferencia de página se aplica entera.

## TINY 2.1

El [port de TINY 2.1](ports/tiny/README.md), de WollyMonjac, arranca desde
una cinta Ferranti por el *tele-input* original. La Mercury carga el
programa y los pesos en el tambor, lee la pregunta, ejecuta el modelo
3-bit SEQ o ternario y perfora la respuesta. El lanzador de PowerShell prepara la
cinta y permite escribir la pregunta por consola.

La consola del emulador también acepta imágenes de tambor con
`clementina correr PROGRAMA.mer --tambor IMAGEN.bin`. Cada sector
contiene 128 cortos de 10 bits, guardados como enteros de 16 bits
little-endian. El tamaño debe coincidir con `--sectores`.

Para cargar una cinta binaria de 1966 y conservar lo que dejó en el tambor:

```powershell
clementina tele-input CINTA.bin --clementina-1966 --guardar-tambor CARGA.tambor
```

La cinta es un byte de 0 a 31 por fila. En modo por lotes se reconoce el
aviso `30` al ejecutarse la bocina del sector 1. El comando termina la
carga ahí o ante un `990 0` válido. En el pupitre la bocina sigue sonando
hasta que el operador detiene la corrida. `--sectores` permite cambiar
el tamaño que propone `--clementina-1966`.

## El *Input Routine* de 1959

La transcripción en `input_routine/` parte del manuscrito anotado de julio
de 1959 y sus agregados de 1960. [Edición digital de la Universidad de Edimburgo](https://www.homepages.ed.ac.uk/jwp/history/mercury/manual/inputroutine/mercury-ip.pdf).
Los sectores 0–12 y
38 tienen direcciones explícitas en `sectorNN.txt`. Los demás están en
`capituloNN.txt`, con las referencias y etiquetas del manuscrito. El
enlazador resuelve esas referencias y escribe los sectores en `enlazados/`.
Cada sector se convierte en 128 cortos con `asm::assemble_sector`.

Para regenerar los sectores enlazados desde PowerShell:

```powershell
$capitulos = Get-ChildItem input_routine -Filter 'capitulo*.txt' | ForEach-Object FullName
python input_routine/enlazar.py $capitulos --salida input_routine/enlazados
cargo run -p mercury-core --example empaquetar_quickies
```

Las pruebas de `input_routine.rs` cargan el código en el tambor y lo ejecutan
desde la transferencia inicial. Cubren el arranque con y sin borrado, la
ida y vuelta de cintas binarias, el rescate con llaves de mano, el *post
mortem* de instrucciones y la traducción de cintas en forma escrita con
etiquetas adelantadas, constantes largas y un salto entre capítulos.

Los archivos de `quickies/` cubren las 16 rutinas que trae el apéndice del
manuscrito. Los `.mer` fijan un origen de prueba para ejecutar cada rutina
por separado. `empaquetar_quickies` reconstruye los sectores 47–63 con
la tabla de R67 (PDF págs. 89–91). Genera el código relativo y los
indicadores de reubicación. Los comentarios `; reubicar` y
`; reubicar largo` señalan operandos simbólicos que quedaron escritos
como números en las versiones de origen fijo. Los sectores 62 y 63
quedan en cero porque ninguna entrada de la tabla de septiembre de 1960
los usa.

Las pruebas verifican que el cargador inserte, reubique y ejecute las
16 rutinas en las páginas 2 y 8. Otra cinta combina `Q19` y `Q1` y
suministra los datos al programa que acaba de cargar.

El sector 11 usa una tabla reconstruida a partir de R16 y de la descripción
del *post mortem*. Las lecturas dudosas y los supuestos de ubicación están
indicados junto a cada transcripción. Las pruebas verifican los recorridos
descritos arriba. La imagen original del tambor sería necesaria para
comprobar identidad bit a bit.

## Licencias

El código propio está bajo [MIT](LICENSE) y la documentación y los datos
propios bajo CC BY 4.0. La *Input Routine* forma parte del proyecto. Los
materiales históricos y los datos de terceros conservan los términos de sus
fuentes. El PDF de Edimburgo se consulta en el sitio de la universidad, que
lo facilita para investigación y estudio privado y pide permiso para
reproducirlo. Ver [licencias y aviso de Edimburgo](LICENCIAS.md).

## Créditos

* Julián Alejandro Lell, por el emulador en Python publicado en Hackaday
  (2020–2022) y por la transcripción del *Annotated Input Routine*; de él no se
  copió código.
* Jeremy Putley y la Universidad de Edimburgo, por los escaneos del
  *Programmers' Manual* y del *Input Routine*.
* Chilton Computing, por la transcripción del *Mercury Autocode Manual* (1961).
* Ernesto García Camarero, autor de los manuales de Autocode (1961) y del sistema
  Convencional (1962) del Instituto de Cálculo, y elgranerocomun.net, que los publica.
