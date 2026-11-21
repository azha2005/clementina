"""Prepara mapas en caracteres Ferranti a partir de los atlas de 257arg."""

import argparse
from collections import Counter
import json
from pathlib import Path
import re
import sys
import unicodedata
from xml.etree import ElementTree

import numpy as np
from PIL import Image, ImageDraw
from scipy.ndimage import distance_transform_edt, find_objects

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
import ascii_ferranti as ferranti


ELECCIONES = {1958: "Presidente", 1962: "Diputados nacionales", 1965: "Diputados nacionales"}
BLOQUES = {
    "FJ": "J", "UCRI": "I", "UCRP": "P", "FNPC": "C", "PSD": "S", "PDP": "D",
    "CON": "C", "NPE": "N", "URB": "R", "UP": "U", "MPP": "M", "UCRB": "B",
    "MID": "E", "UPM": "V", "MFL": "F", "PUN": "A",
}
SIN_ELECCION = {
    1962: {"Catamarca", "Formosa", "La Rioja", "Misiones", "San Luis", "Santa Cruz"},
    1965: {"Jujuy", "Salta"},
}
TRAZOS = " .,/()-=+>≥ILV"


def texto(s):
    plano = "".join(c for c in unicodedata.normalize("NFD", s) if not unicodedata.combining(c))
    return re.sub(r"[^ A-Z0-9.,()/+-]", " ", plano.upper())


def cargar_atlas(carpeta, anio):
    datos = json.loads((carpeta / f"dataset_{anio}.json").read_text(encoding="utf-8"))
    html = (carpeta / f"atlas_{anio}.html").read_text(encoding="utf-8")
    bloque = html.split('<g id="B">', 1)[1].split("</g>", 1)[0]
    caminos, detallados = {}, {}
    for nodo in ElementTree.fromstring("<g>" + bloque + "</g>").findall("path"):
        if "data-i" in nodo.attrib:
            grupo = detallados if nodo.get("class") == "dc" else caminos
            grupo.setdefault(int(nodo.attrib["data-i"]), []).append(nodo.attrib["d"])
    # En Capital el atlas trae el mismo contorno generalizado y detallado.
    # Para el recorte provincial se usa sólo el detallado.
    caminos.update(detallados)
    return datos, caminos


def cargar_provincia(datos, caminos, provincia, anio):
    resultados = next(m for m in datos["modes"] if m["id"] == "gan")
    solo_total = provincia in datos.get("solid", [])
    excepciones = datos.get("solidExcept", [])
    unidades = [(texto(t["n"]), i) for i, t in enumerate(datos["territories"]) if t["p"] == provincia]
    if provincia != "Capital Federal":
        unidades.sort()
    por_departamento = []
    for nombre, i in unidades:
        anillos = []
        for camino in caminos.get(i, []):
            if not re.fullmatch(r"[MLZ0-9 .+\-]+", camino):
                raise ValueError(f"Geometría inesperada en {nombre}.")
            for anillo in re.findall(r"M([^Z]+)Z", camino):
                puntos = re.findall(r"(-?[\d.]+) (-?[\d.]+)", anillo)
                if len(puntos) >= 3:
                    anillos.append([(float(x), float(y)) for x, y in puntos])
        if not anillos:
            raise ValueError(f"Falta el polígono de {nombre}.")
        ganador = (resultados["units"][i] or {}).get("w")
        if solo_total and i not in excepciones:
            ganador = None
        por_departamento.append((nombre, ganador, anillos))
    if not por_departamento:
        raise ValueError(f"No hay departamentos para {provincia}.")
    partidos = [
        (texto(datos["candidates"][clave]["n"]), porcentaje)
        for clave, _, porcentaje in (resultados["prov"].get(provincia) or {}).get("lines", [])
        if not re.search(r"(^|_)(BLA|NUL|ANU|IMP|DIF)$", clave) and porcentaje is not None
    ]
    nota = ""
    if solo_total:
        nota = "EL ATLAS SOLO TRAE RESULTADOS PROVINCIALES"
    elif provincia in SIN_ELECCION.get(anio, set()):
        nota = "SIN ELECCION DE DIPUTADOS NACIONALES"
    elif provincia == "Tierra del Fuego":
        nota = "SIN RESULTADO SEPARADO EN EL ATLAS"
    return por_departamento, partidos, nota


def rasterizar(unidades, columnas):
    puntos = [p for _, _, anillos in unidades for anillo in anillos for p in anillo]
    xmin, ymin = np.min(puntos, axis=0)
    xmax, ymax = np.max(puntos, axis=0)
    w, h = ferranti.CELDA_W, ferranti.CELDA_H
    escala = (columnas - 2) * w / (xmax - xmin)
    filas = 2 + int(np.ceil((ymax - ymin) * escala / h))
    medidas = (columnas * w, filas * h)
    bordes = Image.new("L", medidas)
    pincel = ImageDraw.Draw(bordes)
    territorios = np.zeros((medidas[1], medidas[0]), dtype=np.uint16)
    for numero, (_, _, anillos) in enumerate(unidades, 1):
        interior = np.zeros(territorios.shape, dtype=bool)
        for anillo in anillos:
            xy = [((x - xmin) * escala + w, (y - ymin) * escala + h) for x, y in anillo]
            region = Image.new("1", medidas)
            ImageDraw.Draw(region).polygon(xy, fill=1)
            interior ^= np.asarray(region)
            pincel.line([*xy, xy[0]], fill=255, width=12, joint="curve")
        territorios[interior] = numero
    imagen = np.asarray(bordes, dtype=np.float32) / 255
    return imagen, territorios, filas


def rotular(lineas, territorios, unidades, columnas):
    grilla = [list(l.ljust(columnas)) for l in lineas]
    w, h = ferranti.CELDA_W, ferranti.CELDA_H
    ubicados = set()
    limites = find_objects(territorios, max_label=len(unidades))
    for numero, ((_, ganador, _), limite) in enumerate(zip(unidades, limites), 1):
        if limite is None:
            continue
        rotulo = f"{BLOQUES.get(ganador, '-')}{numero:02}"
        ancho = len(rotulo)
        ys, xs = limite
        region = np.pad(territorios[limite] == numero, 1)
        distancias = distance_transform_edt(region)[1:-1, 1:-1]
        candidatos = []
        for f in range(ys.start//h, (ys.stop-1)//h + 1):
            y = f*h + h//2 - ys.start
            if not 0 <= y < distancias.shape[0]:
                continue
            fila = grilla[f]
            for c in range(max(1, xs.start//w), min(columnas-ancho, (xs.stop-1)//w + 1)):
                x = c*w + w//2 - xs.start
                if x < 0 or x+(ancho-1)*w >= distancias.shape[1]:
                    continue
                if any(g != " " for g in fila[c:c + ancho]):
                    continue
                margen = distancias[y, x:x+ancho*w:w].min()
                if margen >= w / 3:
                    candidatos.append((margen, f, c))
        if candidatos:
            _, f, c = max(candidatos)
            grilla[f][c:c + ancho] = rotulo
            ubicados.add(numero)
    return ["".join(fila).rstrip() for fila in grilla], ubicados


def generar(datos, caminos, provincia, anio, columnas):
    unidades, partidos, nota = cargar_provincia(datos, caminos, provincia, anio)
    candidatos = datos["candidates"]
    mascara, territorios, filas = rasterizar(unidades, columnas)
    celdas = mascara.reshape(filas, ferranti.CELDA_H, columnas, ferranti.CELDA_W)
    ocupadas = celdas.mean(axis=(1, 3)) >= 0.14
    bordes, _ = ferranti.convertir(mascara, columnas, filas, cambio=0.04,
                                 repertorio=TRAZOS, ocupadas=ocupadas)
    mapa, ubicados = rotular(bordes, territorios, unidades, columnas)
    totales = Counter(ganador for _, ganador, _ in unidades)
    leyenda = [
        f"{BLOQUES.get(ganador, '-')} = {texto(candidatos[ganador]['n']) if ganador else 'SIN DATO'} ({n})"
        for ganador, n in totales.most_common()
    ]
    indice = [
        f"{BLOQUES.get(ganador, '-')}{numero:02}  {nombre}" +
        ("  (SIN ROTULO A ESTA ESCALA)" if numero not in ubicados else "")
        for numero, (nombre, ganador, _) in enumerate(unidades, 1)
    ]
    resumen = ["VOTO PROVINCIAL (POR CIENTO DE LOS POSITIVOS)"]
    resumen.extend(f"{porcentaje:6.2f}  {nombre}" for nombre, porcentaje in partidos)
    if not partidos:
        resumen.append("SIN DATOS")
    if nota:
        resumen.extend(["", nota])
    unidad = {"Capital Federal": "CIRCUNSCRIPCION", "Buenos Aires": "PARTIDO BONAERENSE"}
    lineas = [f"{texto(provincia)} - {texto(ELECCIONES[anio])} {anio}", "", *resumen, "",
              "GANADOR POR " + unidad.get(provincia, "DEPARTAMENTO"), *mapa, "", *leyenda, "", *indice]
    return lineas, mapa, mascara, len(ubicados), len(unidades)


def texto_cinta(lineas):
    return "\n".join(lineas) + "\nn"


def generar_catalogo(raiz_atlas, salida):
    catalogo = {}
    for anio, titulo in ELECCIONES.items():
        datos, caminos = cargar_atlas(raiz_atlas / f"mapa{anio}", anio)
        provincias = {}
        for provincia in sorted({t["p"] for t in datos["territories"]}, key=texto):
            columnas = 108 if provincia == "Buenos Aires" else 76
            lineas, _, _, ubicados, total = generar(datos, caminos, provincia, anio, columnas)
            provincias[provincia] = texto_cinta(lineas)
            print(f"{anio} {provincia}: {ubicados}/{total} rótulos en el dibujo.", flush=True)
        catalogo[anio] = {"titulo": titulo, "provincias": provincias}
    salida.write_text(json.dumps(catalogo, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("atlas", type=Path, help="Carpeta del atlas, o carpeta mapas/ de 257arg con --todos.")
    parser.add_argument("salida", type=Path, help="Mapa y leyenda en código Ferranti.")
    parser.add_argument("--provincia", default="Córdoba")
    parser.add_argument("--anio", type=int, choices=ELECCIONES, default=1962)
    parser.add_argument("--todos", action="store_true", help="Generar el catálogo JSON de los tres años.")
    parser.add_argument("--columnas", type=int, default=76)
    parser.add_argument("--comparar", type=Path, help="Comparación PNG de los límites originales y la salida.")
    args = parser.parse_args()
    if args.todos:
        generar_catalogo(args.atlas, args.salida)
        return
    datos, caminos = cargar_atlas(args.atlas, args.anio)
    lineas, mapa, mascara, ubicados, total = generar(datos, caminos, args.provincia, args.anio, args.columnas)
    args.salida.write_text(texto_cinta(lineas), encoding="utf-8")
    if args.comparar:
        ferranti.lado_a_lado([mascara, ferranti.rasterizar(mapa)], args.comparar)
    print(f"{ubicados} de {total} departamentos rotulados en el dibujo. {ferranti.punzadas(lineas)} filas de cinta.")


if __name__ == "__main__":
    main()
