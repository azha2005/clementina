# Elecciones en Clementina

Consulta de resultados electorales argentinos escrita en Mercury Autocode,
como podría haberse corrido en el Instituto de Cálculo en los años sesenta. Los datos
salen de los atlas de [257arg](https://github.com/azha2005/257arg) y van en
una cinta de papel. La máquina lee la consulta, recorre la cinta hasta la
elección pedida y perfora la respuesta en el teleimpresor. La idea viene del
visor `atlas_terminal` de 257arg: las mismas vistas, pero en el teleimpresor.

La cinta trae **1958** (presidente), **1962** y **1965** (diputados
nacionales). El generador puede armar todo el catálogo de 1904 a 1965
(`--todas`), y el control de abajo pasó con las 19 elecciones, pero esas
vistas todavía no se revisaron una por una.

## Uso

En la consola web, elegí **Elecciones argentinas** en el selector del lector.
En modo fácil, **Correr en Clementina** usa la velocidad elegida.
**Acelerar consulta · volver a ×1** completa la consulta a Máx y vuelve a
×1 en la parada final. En **Como en 1961**, elegí la vista antes de pulsar
**Poner cinta del programa**.

Con el binario compilado (`cargo build --release`):

```powershell
.\examples\elecciones\Elecciones-Clementina.ps1                  # pregunta en un bucle
.\examples\elecciones\Elecciones-Clementina.ps1 catalogo
.\examples\elecciones\Elecciones-Clementina.ps1 1958 pais
.\examples\elecciones\Elecciones-Clementina.ps1 1958 distritos
.\examples\elecciones\Elecciones-Clementina.ps1 1962 ficha 1      # Buenos Aires
.\examples\elecciones\Elecciones-Clementina.ps1 1962 diferencia
.\examples\elecciones\Elecciones-Clementina.ps1 1962 mapa
```

El envoltorio sólo arma la cinta: pone la consulta adelante y `cinta.txt`
atrás. La consulta son tres números, elección, vista y distrito:

| Vista | Qué perfora |
|---|---|
| 0 | Catálogo de la cinta |
| 1 | Total del país: votos y porcentaje de cada fuerza, métricas y participación |
| 2 | Cada distrito con el porcentaje y el nombre del ganador |
| 3 | Ficha de un distrito (el número de la vista 2) |
| 4 | Porcentajes de los dos primeros, su diferencia y el distrito más ajustado |
| 5 | Porcentaje nacional de cada fuerza publicada por el atlas y mapa del ganador por distrito |

Sin el envoltorio:

```powershell
.\target\release\clementina autocode examples\elecciones\elecciones.mer --cinta CONSULTA_Y_DATOS.txt
```

Las salidas de [`salidas/`](salidas) son las que compara el test
`crates/mercury-autocode/tests/elecciones.rs`. Llegar a 1962 lleva unos dos
minutos de máquina, porque la cinta se lee a la velocidad del lector. Con el
catálogo completo serían unos siete.

## Mapas provinciales

En el lector de la consola web, elegí **Mapas provinciales**, la elección
y el distrito. Hay 24 distritos por elección. La letra indica el bloque
ganador y el número remite al índice perforado al pie. El encabezado incluye
el porcentaje de cada lista distrital sobre votos positivos, cuando el
atlas lo trae. El índice marca los lugares donde el dibujo no tiene espacio
para un rótulo.

El generador toma los ganadores del atlas de 257arg (transcripción de Cantón,
1968, t. II, y fuentes complementarias) y sus límites históricos. Los trazos usan el cotejo por forma y el
realce direccional de [Alex Harri](https://alexharri.com/blog/ascii-rendering),
adaptados al repertorio Ferranti. Supuesto: el grosor y el ancho del dibujo
se eligen para que se distingan los límites en papel, por lo que el mapa es
esquemático. Clementina recibe la imagen ya codificada en caracteres y la
perfora desde la cinta. El último carácter de la cinta, `n`, señala el fin
del mapa.

`-` indica que el atlas carece de ganador departamental. En 1958 eso
incluye Corrientes y Río Negro, para los que sólo hay resultado provincial.
Cuando el distrito no eligió diputados ese año se indica en el encabezado.

```powershell
python examples/elecciones/generar_mapa_departamental.py E:/dataeleccproyecto/mapas examples/elecciones/mapas_provinciales.json --todos
python examples/elecciones/generar_mapa_departamental.py E:/dataeleccproyecto/mapas/mapa1962 examples/elecciones/cordoba_1962.txt
.\target\release\clementina correr examples/elecciones/perforar_mapa.mer --cinta examples/elecciones/cordoba_1962.txt
```

## Cómo está hecho

* **El programa** ([`elecciones.mer`](elecciones.mer)) no entra en un solo
  capítulo. El capítulo 0 recorre la cinta y saltea lo que no se pidió. El 1
  perfora el registro pedido y los rótulos fijos, y el 2 lee el mapa y lo
  perfora. Se pasa
  de uno a otro con `down` y `up`.
* **Los nombres no se guardan.** En este Autocode, `caption` imprime al
  traducir el programa, no al correrlo, así que los nombres de fuerzas y
  distritos viajan en la cinta. El programa los lee carácter por carácter con
  la función 60 y los perfora tal como vienen con la 63, o los saltea. Los
  rótulos fijos (PARTICIPACION, SIN DATO) son instrucciones 620.
* **Los porcentajes los calcula la máquina**, con los votos y el denominador
  de cada registro. El generador elige el denominador que reproduce los
  porcentajes publicados por el atlas: votos positivos o válidos, o la suma de
  las filas.
* **El mapa** se hace como los mapas de impresora de los años sesenta.
  Primero se rasterizan los departamentos del atlas en 56 columnas, con
  celdas el doble de altas que anchas, como en el visor de terminal. Cada
  celda toma el distrito de su provincia y la perforadora pinta cada distrito
  con la inicial de su ganador: en 1962, J (Frente Justicialista),
  I (UCR Intransigente), P (UCR del Pueblo) y C (Federación de Partidos de
  Centro). Las celdas donde cambia el distrito se marcan con `.` como límite,
  `-` es sin dato, y cada distrito lleva su número de la vista 2 en el centro
  de su parte más grande. En la cinta el mapa va por tramos (largo y valor).
  La máquina busca el ganador de cada distrito en lo que leyó antes y perfora
  el cambio entre registro de letras y de cifras sólo cuando hace falta.

```
          JJJJJ..-----..PPPPP...IIIIII......CC
          J18JJJ..----.PPPPPPPP..I21II.IIIII
         JJJJJJJJ..--.PPPPPPPPPP.IIII.IIIIII
         JJ..J..J......PPPP7PPP.IIII.III8III
```

### Formato de la cinta

Los números terminan en espacio o fin de línea. Un texto va al final de la
línea, después de un solo espacio, y llega hasta el CR.

```
E TITULO                  número de elección y título
N SUBTITULO               cantidad de distritos y fuente
L M D I V W NOMBRE        registro: filas de partido, métricas, denominador,
                          inscriptos, votantes (-1 sin dato), símbolo del
                          ganador en el mapa; primero el país, después N distritos
  L × VOTOS NOMBRE
  M × VALOR ROTULO
F                         filas del mapa
  F × T L1 V1 … LT VT     tramos de L celdas de valor V: 0 afuera, 1 a N
                          relleno del distrito, 100 + código un carácter
                          fijo (límite o número de distrito)
Z                         líneas de la leyenda
  Z × CODIGO NOMBRE       símbolo y su nombre
0                         fin de la cinta
```

Los códigos son del teleimpresor Ferranti; 32 + código es el mismo
carácter en registro de letras.

## Regenerar y controlar

Hace falta una copia de 257arg (por omisión `E:\dataeleccproyecto`; también
se puede dar su carpeta como argumento o en la variable `ARG257`) y Node 18
o posterior.

```powershell
node examples/elecciones/generar_cinta.mjs                      # 1958, 1962 y 1965
node examples/elecciones/generar_cinta.mjs --todas --esperado esperado.json
node examples/elecciones/controlar.mjs esperado.json
```

`controlar.mjs` corre en el emulador la ficha de cada registro de la cinta y
compara cada porcentaje y cada participación con los del atlas. Con
`--todas` (19 elecciones, 468 registros) comparó 1220 porcentajes y 221
participaciones sin diferencias (20/11/2026).

## Supuestos y límites

* Supuesto: la relación de aspecto de las celdas del mapa (2) es la que usa
  por omisión el visor de 257arg, y las 56 columnas entran en el renglón del
  teleimpresor. No se midieron el paso ni el ancho del teleimpresor de
  Clementina.
* Capital Federal no ocupa ninguna celda a esta escala: no aparece en el
  mapa, pero sí en las demás vistas. Los límites quedan escalonados donde la
  frontera es oblicua.
* Los textos pierden tildes y eñes, porque el teleimpresor Ferranti no las
  tiene, y los nombres largos se perforan enteros.
* Las cifras son las del atlas: los distritos sin dato del atlas salen como
  SIN DATO, nunca completados. La agrupación de las fuerzas del mapa es la
  del atlas (en 1962, por ejemplo, el Frente Justicialista reúne a las listas
  neoperonistas de cada provincia).
* No hay cálculo de bancas.
