# TINY 2.1 en Clementina

Modelo original de [WollyMonjac](https://github.com/WollyMonjac/Tiny-2.1).
El port ofrece `r3_80x2L2x256.pt` (dos bloques, 3-bit SEQ),
`rt3_96_wd05.pt` (`-Ternario`, dos bloques ternarios) y
`r3_80x3L2x224.pt` (`-Compacto`, tres bloques, 3-bit SEQ empaquetado en
los 512 sectores del tambor). Los dos primeros corren con contexto de 80
tokens y caché K/V de 5 bits; el compacto usa contexto de 60 tokens y
caché K/V de 4 bits con escala FP20.

## Cargar y hacer una pregunta

Desde la raíz del repositorio, con Windows PowerShell 5.1 o PowerShell 7,
Cargo y Python con PyTorch y NumPy:

```powershell
.\ports\tiny\Tiny-Clementina.ps1 "what is the capital of france"
.\ports\tiny\Tiny-Clementina.ps1 "what is the capital of france" -CintaSvg target\tiny\respuesta.svg
.\ports\tiny\Tiny-Clementina.ps1 -Ternario "how many legs does a spider have"
.\ports\tiny\Tiny-Clementina.ps1 -Compacto "name the capital of argentina"
```

Sin argumentos, el lanzador pide preguntas por consola y arranca desde la
cinta para cada una. Una línea vacía cierra la sesión. El parámetro
`-Checkpoint` permite indicar dónde está el modelo si se movió de su ubicación
predeterminada en Descargas.

## Consola en vivo

```powershell
.\ports\tiny\Tiny-Clementina.ps1 -EnVivo -Compacto
```

Levanta `en_vivo.py` en `http://127.0.0.1:7661/?programa=tiny-...` y abre la consola
web ([`web/index.html`](../../web/index.html)), que corre la cinta en WASM y muestra
el pupitre, el teleimpresor, el lector, el tambor y el guion de TINY. La distribución del pupitre sigue el
diagrama del Apéndice VII de CS 225 A y se dibuja plano, como ese diagrama.
Los colores y las piezas vienen de fotos de la réplica del Museo de
Informática: los tubos bajo una visera, los selectores
DISPLAY, el reloj de bisel dorado, las palancas de punta blanca y negra (6 y 4
en la primera hilera, 5 y 5 en las otras) y el botón OFF rojo al frente. Los
visores trazan pulsos rectangulares hacia arriba por cada 1 (Durán 2018,
págs. 35 y 106; TEC cap. 1 los describe como puntos brillantes). OFF corta la corrida.

Tiene dos modos (`?modo=1961` en la dirección elige el segundo):

* **Modo fácil.** Un botón perfora la pregunta, la pone en el lector, deja
  el pupitre en marcha normal (§7.5, llave 2 de INPUT arriba) y aprieta
  INITIAL TRANSFER.
* **Como en 1961.** Una hoja de operación muestra los pasos de §7.1, §7.4 y
  §7.5 (sin bloquear al operador si se equivoca). Todos los controles en vivo
  de 7.5 están conectados directamente al núcleo WASM (`INITIAL TRANSFER`,
  `PREPULSE`, `SINGLE`, `MANUAL`, `HANDSWITCH STOP`, `WRITE CURRENT`, `HOOT` y
  las tres filas de llaves `J`, `D`, `F`). Con todas las llaves `INPUT` abajo
  el *Input Routine* para en `990 0`; con `WRITE CURRENT` en `OFF`, la carga no
  termina y cae en la parada por bucle `280 60`.

Supuestos: qué registro va en cada visor (y orden de los selectores DISPLAY),
los colores, el sentido de las lámparas SAC TEST, B TEST y SHIFT ≥ 31, que con
WRITE CURRENT en OFF la instrucción 69 tarde igual sin cambiar el sector y la
limpieza de registros volátiles al cortar con OFF. `CLEAR TAPE`, paridad
(`INHIBIT PARITIES`, `PARITY STOP`), fusibles y cinta magnética no se modelan.
Los rótulos del panel van con mayúscula inicial (en la placa están en
mayúsculas). Los cuadros son instantáneas del emulador cada 10 s de
máquina o cuando se perfora algo.

## Temperatura

```powershell
.\ports\tiny\Tiny-Clementina.ps1 -Temperatura 0.8 -Semilla 17 "what is the capital of argentina"
```

Con `-Temperatura 0` (por omisión), la Mercury elige el token con mayor
puntaje. Con una temperatura entre `0.01` y `10`, calcula
`exp((puntaje − máximo) / temperatura)` para cada token y hace un sorteo.
Por debajo de 1 se concentra en las opciones de mayor puntaje. Por encima
de 1 da más peso relativo a las demás. Cambiar la temperatura puede
cambiar la respuesta, que puede seguir siendo incorrecta.

`-Semilla` va de 1 a 1023, con 1 por omisión. El mismo modelo, pregunta,
temperatura y semilla repiten la respuesta. La Mercury guarda el estado
del sorteo en núcleos y ejecuta el generador de 10 bits de
`muestreo.mer`. Si usás `-Preparar`, la temperatura y la semilla viajan
en la cinta. Las cintas con muestreo se guardan en un directorio con el
sufijo `-tTEMPERATURA-sSEMILLA`.

## Grabar la cinta

```powershell
.\ports\tiny\Tiny-Clementina.ps1 -Preparar
.\ports\tiny\Tiny-Clementina.ps1 -Preparar "what is the capital of france"
.\target\release\clementina.exe tele-input target\tiny\nativo\tiny-con-pregunta.bin
```

`-Preparar` guarda en `target/tiny/nativo/` el `.bin` con un byte por fila
(0 a 31), el `.svg` a escala real y un extracto `.vista.svg`. La cinta
de programa sola se llama `tiny.bin`; con pregunta, `tiny-con-pregunta.bin`.
A la primera se le puede agregar una pregunta de un archivo de texto con
`--cinta`. La cinta con pregunta arranca y responde por sí sola.
El lanzador guarda la variante ternaria en `target/tiny/nativo-ternario/`
y la compacta en `target/tiny/nativo-compacto/`.

El emulador prepara los sectores 0 y 1 del *Input Routine* histórico y
enciende la llave de mano 2. El *tele-input* carga la cinta binaria,
verifica la suma de control de cada bloque y escribe los sectores en
tambor. Después carga en núcleos y ejecuta el arranque. En una Mercury
física se necesitarían el *Input Routine* residente, un lector Ferranti
y la cinta perforada. La cinta 3-bit de dos bloques tiene 111.705 filas,
unos 284 metros a 2,54 mm por fila (462/512 sectores). La ternaria tiene
117.045 filas, unos 297 metros (482/512 sectores). La compacta de tres
bloques tiene 126.924 filas, unos 322 metros, y ocupa los 512/512 sectores
del tambor contando los 36 reservados para la caché K/V.

## Cálculo en la Mercury

- `control.mer` lee el código Ferranti, convierte los caracteres en tokens,
  recorre las órdenes de cálculo y perfora la respuesta. La pregunta admite
  hasta 78 caracteres del vocabulario disponibles en Ferranti (58 en
  `-Compacto`). El teleimpresor muestra letras en mayúsculas y usa `?`
  para los signos de salida que no tienen representación en ese código.
- `auxiliares.mer` y `tensores_compactos.mer` leen embeddings y ganancias
  del tambor, ejecutan L1Norm, ReLU al cuadrado y las sumas residuales, y
  eligen el token de mayor puntaje. En `-Compacto`, los embeddings se
  empaquetan a razón de diez valores de 9 bits en nueve cortos de 10 bits.
- `atencion.mer` cuantiza y guarda K y V en tambor, lee los pares de
  cachés, aplica ALiBi, calcula softmax con el máximo restado y pondera los
  valores. Usa la Quicky 1 para el recíproco y el runtime de Autocode para exp.
- `tmac.mer` y `coeficientes_compactos.mer` leen las matrices 3-bit SEQ
  desde el tambor. Las proyecciones combinan tres planos con tablas de 32
  entradas para grupos de cinco activaciones. Por fila se multiplica por
  `|alpha|/8` y se suma el sesgo. En `-Compacto`, `f1` comparte el banco de
  proyecciones y cada matriz guarda sus escalas y sesgos con dos exponentes
  compartidos y pares de cortos de 10 bits con signo.
- `ternario.mer` usa una sola tabla de 27 largos para cada grupo de
  tres columnas. Cada corto guarda dos índices de la tabla, uno por
  fila. Por fila se multiplica por `2·|alpha|/3` y se suma el sesgo.
  El vector intermedio usa 384 largos en núcleos.
- Se usa FP40 para las activaciones, escalas, sesgos y acumulaciones. Los
  embeddings y ganancias usan cortos de 10 bits (o 9 bits empaquetados en
  `-Compacto`) y escala potencia de dos. El despliegue omite la
  cuantización a 8 bits de las activaciones del entrenamiento.

## Verificación

`-Verificar` compara 3.096 resultados del núcleo de proyecciones con un
producto escalar para entradas diádicas, palabra por palabra. También
prueba inversa, exponencial, L1Norm y producto escalar.
`-Ternario -Verificar` compara de la misma forma 4.176 resultados del
núcleo ternario, y `-Compacto -Verificar` comprueba los 4.536 resultados
de las 18 matrices del modelo compacto.

```powershell
$tiny = Join-Path $env:USERPROFILE 'Downloads\tiny68k'
$modelo = Join-Path $tiny 'models_retro\tiny_retro_exp3\r3_80x2L2x256.pt'
python ports/tiny/verificar.py $tiny $modelo target/release/clementina.exe target/tiny/nativo/tiny.bin --nativo

$compacto = Join-Path $tiny 'models_retro\tiny_retro_3bit\r3_80x3L2x224.pt'
python ports/tiny/verificar.py $tiny $compacto target/release/clementina.exe target/tiny/nativo-compacto/tiny.bin --nativo
```

El verificador compara las respuestas que perfora el programa arrancado
desde cinta con la variante de despliegue en PyTorch. La comparación
normaliza las mayúsculas del código Ferranti. El modelo puede dar
respuestas incorrectas. El tiempo de máquina que muestra el emulador
incluye la carga, el cálculo y la perforación.

## Medición con ocho caracteres de salida

```powershell
.\ports\tiny\Tiny-Clementina.ps1 -SalidaMaxima 8 "what is the capital of france"
.\ports\tiny\Tiny-Clementina.ps1 -Ternario -SalidaMaxima 8 "what is the capital of france"
.\ports\tiny\Tiny-Clementina.ps1 -Compacto -SalidaMaxima 8 "what is the capital of france"
```

Se usó el mismo prefijo para cada variante. Los tres modelos generaron ocho
caracteres, con el tiempo medido desde el arranque por cinta.

| Pregunta | 3-bit 2L (s Mercury) | Ternario 2L (s Mercury) | Compacto 3L (s Mercury) |
| --- | ---: | ---: | ---: |
| `what is the capital of france` | 6.022,090 | 6.007,830 | 9.953,103 |
| `how many legs does a spider have` | 6.478,611 | 6.470,209 | 10.788,573 |
| `what language is spoken in brazil` | 6.633,560 | 6.627,025 | 11.071,844 |

Una pasada aislada por las matrices únicas llevó 82,528 s de Mercury en
3-bit (10 matrices), 77,725 s en ternario (10 matrices) y 121,874 s en el
compacto (18 matrices). Los tres modelos coincidieron con su respectiva
variante de despliegue de PyTorch en todas las preguntas del verificador.
