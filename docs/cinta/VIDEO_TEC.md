# Cinta perforada filmada por la UBA

El primer plano del capítulo 1 de *Clementina* de TEC aparece también en
*Ayer en Exactas: Clementina 1962*. La misma toma está en un encuadre más
amplio en *La Ciudad Universitaria*, el corto de Aldo Persano que difundió
Historia Exactas UBA. Coinciden las perforaciones y su desplazamiento entre
paradas. El corto muestra el primer plano dentro de una secuencia del
Instituto de Cálculo, seguido de una vista de la sala y el pupitre. Es
evidencia a favor de que la cinta se filmó allí. El montaje deja abierta la
procedencia de la cinta física y qué programa la produjo.

## Tomas comparadas

| Fuente | Intervalo aproximado | Qué se puede leer |
|---|---|---|
| [*La Ciudad Universitaria*](https://www.youtube.com/watch?v=9sFl_rHAqhM&t=645s) | 10:45,5–10:48,8 | Primer plano más amplio. Se recuperan 52 filas consecutivas. |
| [*Ayer en Exactas: Clementina 1962*](https://www.youtube.com/watch?v=zfJW_QUmXU8&t=3s) | 00:03,9–00:06,8 | Mismo primer plano con los bordes recortados. Confirma el segundo espacio delante de −0.08703 y el comienzo de la línea de 0.88. |
| [*Clementina*, cap. 1, TEC](https://www.youtube.com/watch?v=NAVuZyl5jz0&t=1027s) | 17:07,4–17:13,5 | Mismo primer plano, reencuadrado y repetido. En el corte de 17:09,6 se ve 0.88. La lectura anterior de 37 filas había omitido un espacio en el empalme de −0.08703. |
| [Impresión en el teletipo, *La Ciudad Universitaria*](https://www.youtube.com/watch?v=9sFl_rHAqhM&t=668s) | 11:08–11:10,9 | En la hoja se leen `BALISTICA` y `TABULACION 37`. |
| [Cinta en un mecanismo, Exactas](https://www.youtube.com/watch?v=zfJW_QUmXU8&t=22s) | 00:22,5–00:27,5 | Se leen 48 filas en Ferranti, con cuatro números en una misma línea. La película (cerca de 11:11) conserva más cinta por debajo del mecanismo. Exactas aporta el último retorno de carro visible. |
| [Demostración de Marienbad, DiFilm](https://www.youtube.com/watch?v=JVbiEeJv47w&t=175s) | 02:55–03:00 y 03:10–04:00 | Se ve una tira de papel que Paiuk saca del equipo y lleva a la pizarra. El relato la vincula con las jugadas. En estos cuadros la tira está muy clara o de canto y no se distinguen filas de agujeros con seguridad. |

## Lectura de las perforaciones

En el primer plano hay cinco canales de datos. De izquierda a derecha en la
imagen, los pesos son `1 2 · 4 8 16`, con el agujero chico de arrastre en
`·`. Las filas se leen de arriba hacia abajo. Los cuadros quietos capturan
ventanas sucesivas de la cinta en sentido inverso, porque la cinta se
desplaza hacia abajo en la imagen.

Se midió la posición de las filas por el arrastre y se leyó la presencia de
agujeros en los cinco canales. El paso en la copia más amplia es de unos
44–45 píxeles por fila. Los empalmes se verificaron por el desplazamiento
vertical y por la coincidencia de los grupos de agujeros. Se revisaron
visualmente los extremos y el tramo de los dos espacios. Estas son las
52 filas, agrupadas por línea impresa.

```text
0 14 14 11 16 28 16 8 2 16 8 14 14 0 30 13 30
0 14 16 28 8 7 14 14 0 14 14 11 16 28 16 8 7 16 19 14 14 0 30 13 30
0 14 16 28 8 8 14 14 0 14
```

En código Ferranti (CS 225 A, Apéndice VI, y tabla II del *Autocode* de García
Camarero), `FS` es cambio a cifras (0), `_` es un espacio (14), `CR` es
retorno de carro (30) y `LF` es avance de línea (13).

```text
[…] FS __-0.08208__ FS CR LF CR
    FS _0.87__ FS __-0.08703__ FS CR LF CR
    FS _0.88__ FS _ […]
```

Para ubicar los detalles, la numeración del corto usa el cuadro 0 en
10:44,50 a 30 cuadros por segundo. Los tiempos pueden variar un cuadro
según la extracción.

| Cuadro del corto | Tiempo aproximado | Detalle |
|---|---|---|
| 129, 128 y 122 | 10:48,80 / 10:48,77 / 10:48,57 | El 129 muestra el `FS` inicial sobre la transición a la sala. El 128 muestra los dos espacios antes del menos y el 122 confirma el segundo espacio y el número. |
| 107 y 46 | 10:48,07 / 10:46,03 | Primer y segundo `FS CR LF CR`. |
| 77 | 10:47,07 | `7__ FS __-0.0`, con los dos espacios posteriores al `FS` a la vista. El cuadro 45 de Exactas y el 89 de TEC (contado desde 1 a partir de 17:07,4) muestran los mismos agujeros. |
| 40 y 33 | 10:45,83 / 10:45,60 | El 40 enlaza el salto de línea con 0.88 y el 33 alcanza a mostrar el `FS` siguiente y un espacio. |

## Estructura y relación con los manuales

* Se recupera una línea de dos columnas, `0.87` y `−0.08703`, además del final
  de la línea anterior (`−0.08208`) y el comienzo de la siguiente (`0.88`).
  El paso visible de la primera columna es 0.01. Un valor anterior de
  0.86 sería compatible con ese paso, pero queda fuera del encuadre.
* Los cuatro números muestran dos espacios finales. El campo de 0.87
  ocupa siete caracteres, compatible con `print (x) 1, 2`. Los campos
  negativos ocupan doce caracteres cada uno, compatible con
  `print (y) 3, 5` y dos ceros no significativos reemplazados por espacios.
  El manual de 1961, §4.2, da el ancho `m + n + 4` para punto fijo con
  decimales y permite agregar espacios. La cinta permite medir el ancho
  observado, pero no recuperar los parámetros exactos del programa.
* Hay dos secuencias idénticas de fin de línea `0 30 13 30`. La rutina
  `R 1020.1` del [manual de Convencional de García Camarero](../FUENTES.md#clementina-instituto-de-cálculo-uba) (pág. 29 del PDF)
  (1962, §5.2, pág. impresa 28) perfora esos mismos cuatro valores con
  `620 0`, `620 30`, `620 13` y `620 30`. El segundo `CR` tiene así un
  antecedente explícito en el sistema de la UBA. El manual de *Autocode*
  describe el efecto de `newline` en el teletipo, sin dar su secuencia de
  perforaciones.
* El `FS` aparece al comienzo de cada campo numérico visible y del fin de
  línea. El apéndice II, §3, de [*Autocode* de García Camarero](../FUENTES.md#clementina-instituto-de-cálculo-uba) (pág. 65 del PDF)
  también indica perforar el cambio de registro delante de cada grupo al
  usar directamente la instrucción `620`. El formato es compatible con
  Autocode y puede conseguirse también con rutinas en Convencional. La
  cinta registra perforaciones, que pueden provenir de distintas
  combinaciones de instrucciones.

El test `tape::tests::cinta_del_documental` usa las 52 filas como dato de
referencia. El decodificador convierte `LF` en salto de línea y conserva
los espacios. El listado de arriba conserva también los controles que no
se imprimen.

## Cinta en el mecanismo

En [*La Ciudad Universitaria*, 11:11–11:15](https://www.youtube.com/watch?v=9sFl_rHAqhM&t=671s),
la cinta avanza hacia abajo. En el tramo inferior se enderezaron la
inclinación de los canales y la de las filas, se aumentó el contraste
local y se cotejaron las perforaciones de una misma fila mientras
pasaban por distintas partes del cuadro. El avance visible suele ser
de unos 10 píxeles por fila. Hay cinco canales de datos y una columna
de arrastre. La marca longitudinal a la derecha corresponde al borde
junto al reflejo del rodillo. Se lee de abajo hacia arriba, con los pesos
`1 2 · 4 8 16` de izquierda a derecha.

Las 48 filas recuperadas, agrupadas por control o campo, son

```text
14 14 0 30 13 30
0 14 16 28 1 4 14 14
0 14 16 28 16 8 1 16 8 14 14
0 14 16 28 16 16 4 19 25 14 14
14 16 28 16 16 1 1 1 14 14
0 30
```

```text
[…] __ FS CR LF CR
FS _0.14__ FS _0.08108__ FS _0.00439___0.00111__ FS CR […]
```

El fragmento comprende el cierre de una línea anterior y cuatro valores
de la siguiente. Antes de los primeros tres se ve `FS`. Entre
`0.00439` y `0.00111` hay tres espacios consecutivos. El cierre anterior
repite el `FS CR LF CR` del primer plano. Del cierre siguiente se ven
`FS CR` hasta el corte. El formato coincide con la impresión de números
de ancho fijo y los valores corresponden a otro fragmento. Queda abierta
la posibilidad de que los dos fragmentos pertenezcan a una misma cinta.

La numeración de cuadros de la película comienza en 11:10,00 a 30 cuadros
por segundo. La de Exactas comienza en 00:22,50 a 25 cuadros por segundo.
Los tiempos pueden variar un cuadro según la extracción.

| Cuadros de referencia | Detalle |
|---|---|
| Película, 30, 33, 38 y 42 | Dos espacios al comienzo del fragmento y el primer `FS CR LF CR`. |
| Película, 48–63 | Campo de `0.14`. |
| Película, 63–94 | Campo de `0.08108`. |
| Película, 95–117 | Campo de `0.00439`. |
| Película, 126–145 | Tres espacios entre los dos últimos números, `0.00111`, dos espacios y `FS`. |
| Exactas, 114 y 116 | El 114 muestra los dos espacios y el `FS`. El avance en el 116 permite leer también el `CR` antes del corte, cerca de 00:27,14. |

En el tramo superior, sobre el rodillo, se intentó registrar la cinta
usando el mismo avance. La curvatura, el desplazamiento lateral y los
cambios de contraste dejaron perforaciones ambiguas. No se pudo unir
ese tramo con el inferior ni obtener de él una segunda lectura continua.

## El cálculo que muestra la película

En el teletipo se lee `BALISTICA` y debajo `TABULACION 37`, con los dos
renglones visibles juntos cerca de 11:09,8. La película corta de esa
impresión al mecanismo con la cinta. El [*Informativo del Instituto de
Cálculo* n.º 4](https://elgranerocomun.net/IMG/pdf/Informativo_4_pdf_.pdf#page=5)
(1962, pág. 5) registra el trabajo n.º 66, «Balística», para la Escuela
Superior Técnica, dirigido por el coronel Lamonega. Es un candidato
concreto para identificar el listado filmado. El [n.º 5](https://www.elgranerocomun.net/IMG/pdf/Informativo_5_pdf_.pdf#page=1)
también registra una exposición del capitán S. A. Insúa sobre el
cálculo de la trayectoria de un proyectil en el curso de Autocode de
mayo de 1962, otra evidencia de que el tema se trabajaba en el Instituto.

La cercanía de las tomas hace plausible que la cinta del mecanismo
corresponda a esa tabulación. Los cuadros de la hoja permiten leer el
título, pero no dan una fila numérica completa que se pueda cotejar
con la cinta. Quedan sin identificar el significado del `37`, el de las
columnas y la relación del primer plano de 10:45 con ese trabajo.

## El teletipo

El manual de Autocode de García Camarero (1961, pág. impresa 9) ubica
la teleimpresora de resultados en un escritorio a la derecha del
pupitre. El [*Informativo* n.º 3](https://www.elgranerocomun.net/IMG/pdf/Informativo_3_pdf_.pdf#page=3)
documenta que Oscar Mattiussi, del equipo de mantenimiento de la
Mercury, se formó en la compañía Creed.

El modelo mejor respaldado es el **Creed 75**. El equipo de reconstrucción
de Clementina buscaba un Creed 75 entre los periféricos de época
([*Resurrection* n.º 74, pág. 14](https://www.computerconservationsociety.org/resurrection/pdfs/res74.pdf)),
y Peta Walmisley lo identifica como el modelo que usaba Ferranti en su
crónica del museo ([n.º 91](https://www.computerconservationsociety.org/resurrection/res91.htm)).
En la película se ve desplazarse el cabezal mientras el rodillo queda
fijo lateralmente, un rasgo que destaca el [artículo técnico de
Creed](https://www.patstec.fr/MEDIAS/025-000000107/creed-model-75.pdf)
(1962, págs. 3–4). La variante exacta y el número de serie de la unidad
de la UBA quedan sin confirmar. La crónica de 2020 indica que la réplica
del museo usaba periféricos Siemens.
