# AGENTS.md

Guía para quien trabaje en este repositorio (personas o agentes). Resume lo
que se fue aprendiendo; el README cuenta qué es el proyecto y cómo se usa.

## Qué es

Emulador de **Clementina**, la Ferranti Mercury de la FCEN-UBA (1961–1970),
en Rust, más un compilador de **Mercury Autocode** reconstruido a partir de
los manuales. Todo el texto del proyecto (código, comentarios, mensajes,
documentación, commits) va en **castellano rioplatense**.

## Estructura y comandos

```
crates/mercury-core      núcleo sin E/S ni dependencias (compilable a WASM)
crates/mercury-autocode  compilador de Autocode; runtime/*.mer son rutinas en código Mercury
crates/mercury-cli       binario `clementina` (correr, listar, autocode)
examples/                programas en forma escrita (.mer)
samples/autocode/        programas en Autocode (pruebas del compilador de Edimburgo)
docs/                    manuales, escaneos, fuentes (docs/FUENTES.md), cintas del museo
tools/                   Python para leer cintas desde fotos
```

* `cargo test --workspace`: tiene que pasar todo antes de cada commit.
* `cargo build --release` y `.\target\release\clementina autocode PROGRAMA.mer`.
* `clementina autocode X.mer --listado` muestra el código Mercury generado:
  es la primera herramienta para depurar el compilador.
* Para seguir una rutina instrucción por instrucción, un test con
  `m.step(&mut io)` en un bucle e imprimiendo `m.c` y `m.acc` resuelve casi
  todo (así se encontraron los errores de la división).

## Reglas del proyecto

* **Fuentes primarias.** CS 225 A (*Programmers' Manual*, escaneos en
  `docs/refs/`), el *Mercury Autocode Manual* (transcripción de Chilton en
  `docs/autocode_manual/`) y los manuales de García Camarero de la UBA
  (`docs/text/`). Cada fuente nueva se anota en `docs/FUENTES.md`.
* **`Supuesto:`** marca en el código toda decisión que el manual no
  respalda. Las importantes se repiten en la sección de supuestos del README.
* **Nada del emulador en Python de J. A. Lell** (Hackaday): su licencia no es
  clara. El *Input Routine* se retranscribe del PDF manuscrito de 1959
  ([enlace en Fuentes](docs/FUENTES.md)), no de su archivo.
* No inventar resultados: si algo no se verificó contra el manual o en el
  emulador, se dice.
* Los registros con datos personales se conservan en el archivo local.

## La máquina: trampas aprendidas

* **Direcciones escritas.** En la forma escrita todo se cuenta en registros
  medios (20 bits): `32` es el medio 32, `32+` su mitad derecha (corto 65),
  `p.l` es página·64 + línea. Un largo se escribe con su medio par (`36` =
  largo 18). Los cortos de las funciones 00–07 y 20–27 llegan a las páginas
  0–15 (con la versión de "segundo cuarto"); **63 sólo a la página 0**. Sólo
  las páginas 0–15 pueden tener instrucciones; las 16–31 sólo números largos.
* **B-modificación.** Todo es B-modificable salvo 0–7, 10–18, 70–77 y 90–97,
  donde el dígito B nombra el registro. Ojo con las ramas del CPU que usan
  `n` en vez de la dirección modificada: la 62 estaba mal (la destapó el
  programa de contornos).
* **Redondeo.** Las operaciones redondeadas (42, 43, 50, 51) fuerzan un 1 en
  el último dígito **antes** de estandarizar. Consecuencias:
  - ni `7 + 1` da 8 exacto con `=`; los tests comparan con tolerancia, y sólo
    con `»` (44, 45, 52, 53) se espera exactitud;
  - una resta redondeada con cancelación (`m·h − 1`) agranda el 1 forzado al
    correr hacia arriba: en esos casos, 44/45 sin redondeo;
  - una operación redondeada sobre un número **sin estandarizar** pierde
    muchos bits. Estandarizar primero.
* **Estandarizar** un número de exponente 9 o 29: sumarle sin redondeo (44)
  un cero de exponente 0 (página 0, registro 56). Sumar el cero de la máquina
  (exponente −256) sólo anda por la rama "diferencia ≥ 32" de `acc.rs`, que
  es un supuesto.
* **Parte entera:** `780 28` deja el acumulador con exponente 29 y la parte
  entera (piso, también para negativos) en la fracción; con `410 42` queda
  en la mitad baja del largo 21, el corto 85. **Índice a número:** el índice
  va a la mitad alta de la fracción de un largo con exponente 9 (largo 20,
  corto 83), que vale exactamente el índice pero sin estandarizar.
* **Multiplicaciones de enteros** sin estandarizar con 52 son exactas; con un
  factor no entero, no. El compilador multiplica primero índices y enteros.
* **Tambor:** 512 sectores de 128 cortos; 67 elige el sector (B-modificable,
  `677 0` toma el sector del sac) y 68/69 copian una página entera.

## El ensamblador (`asm.rs`): trampas

* Un comentario entre paréntesis termina en el **primer** `)`: nada de
  paréntesis dentro de los comentarios de cabecera de los `.mer`.
* Una etiqueta `(N` sobre un corto `=n` guarda el registro medio y pierde el
  `+`: cada corto etiquetado tiene que caer en posición par (rellenar con
  `=0`).
* `vN` como literal (funciones 10–17, 30–37) da el registro **medio** de la
  etiqueta, no el largo. Para pasar la dirección de un largo, usar una
  dirección fija conocida.
* Los números largos se alinean solos (insertan un `570`).

## El Autocode

* **Mapa de memoria** (García Camarero, Apéndice 1): índices i–t en los
  cortos 116–127; división en los medios 4–31 (el final, en 50–55 y en
  112–117 de la página 1); lugar de trabajo 32–38; capítulo en las páginas
  2–14 (código desde el medio 128, constantes desde el largo 479 hacia
  abajo); variables principales desde el largo 512; especiales en la página
  31 (a'…z', a…z, π, 0, −1, −4·10⁸). El capítulo c vive en los sectores
  464 − 16c a 476 − 16c. La receta de números aleatorios del manual
  (`i=29 j=955 k=202 l=140`, `400, 58` → 43⁵) es la prueba de que el mapa
  es correcto.
* **Registros:** B1 para sufijos y dirección del divisor, B2 para vueltas de
  rutinas (`102 vuelta; 590 rutina`, vuelven con `592 0`), el sac para
  índices; B3 y B4 los usan las rutinas; B5 y B6 son del programador.
* **Rutinas** en `runtime/*.mer`, ensambladas con `asm`. `pagina0.mer` y
  `capitulos.mer` (página 1) van fijas; las demás (impresión, lectura y
  funciones) se enlazan en cada capítulo que las usa. Una rutina puede tener
  varias entradas (`trig.mer`: etiqueta 1 sin, 2 cos, 3 tan).
  Toda rutina nueva necesita un test en `tests/rutinas.rs` que la corra sola.
* **Funciones nuevas:** reducir el argumento con la constante partida en dos
  (Cody-Waite: la primera parte con 19 cifras binarias, y el producto por
  ella **sin redondeo**, 52, para que sea exacto) y medir el error con un
  barrido en `tests/rutinas.rs`. Restar 1 a una mantisa de exponente 0
  pierde un bit al alinear; restar 1/2 dos veces es exacto.
* El segundo argumento de divide, arctan y radius espera en el largo 51
  (página 1, registro 102), que ninguna rutina toca: la división usa el 42.
* Los programas de Edimburgo usan variantes: `print (x, m, n)`, `caption`,
  `F`/`X` por φ, `->` y una línea `->` que separa los datos. Se aceptan
  como extensiones; el lenguaje de referencia son los dos manuales.
* En Autocode r, s, t son **índices** (−512 a 511): `fib2.mer` desborda a
  propósito de la máquina, no del compilador.

## Cintas

* Clementina usaba el **código Ferranti** (CS 225 A, Apéndice VI): cifras =
  0, letras = 27, CR = 30, LF = 13, espacio = 14 sólo en cifras. Las cifras
  tienen valor = código mod 16. El manual de Autocode le cambia el nombre a
  algunos símbolos del mismo código (12 = », 24 = φ, 29 = ', 30 en letras = π).
* Las tres cintas del museo están en **ITA2** (teletipo), no en código
  Ferranti; ver `docs/cinta/TRANSCRIPCION.md`.

## Entorno (Windows)

* La consola por omisión es PowerShell; Git Bash también está. Python con
  `PYTHONIOENCODING=utf-8` para no romper los acentos.
* Al generar código o tests con scripts de shell, un `"\n"` dentro de un
  heredoc puede llegar como salto de línea real: para cadenas con escapes,
  editar el archivo directamente.
