"""Lee los agujeros de una foto de cinta de papel de 5 canales ya enderezada
(la cinta horizontal) y devuelve el valor 0-31 de cada fila.

Uso: python tools/leer_cinta.py IMAGEN.png [--umbral N] [--debug SALIDA.png]

Supone el formato Ferranti/Mercury (CS 225 A, Apéndice VI): 3 canales de un
lado del agujero de arrastre (valores 16, 8, 4, del borde hacia el arrastre) y
2 del otro (2, 1, del arrastre hacia el borde).
"""

import argparse
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("imagen")
    ap.add_argument("--umbral", type=float, default=0.55, help="fracción del brillo del papel")
    ap.add_argument("--debug")
    ap.add_argument("--invertir", action="store_true", help="leer de derecha a izquierda")
    a = ap.parse_args()

    im = Image.open(a.imagen).convert("L")
    g = np.asarray(im, dtype=float)
    # Banda de la cinta: filas donde la mediana es clara.
    rowmed = np.median(g, axis=1)
    paper = np.percentile(g, 90)
    band = np.nonzero(rowmed > paper * 0.7)[0]
    y0, y1 = band.min(), band.max()
    # Columnas con papel en el centro de la banda.
    colmed = np.median(g[y0:y1], axis=0)
    cols = np.nonzero(colmed > paper * 0.6)[0]
    x0, x1 = cols.min(), cols.max()

    dark = np.zeros_like(g, dtype=bool)
    sub = g[y0:y1, x0:x1]
    # Umbral local: comparar con el brillo del papel en un entorno.
    local = ndimage.uniform_filter(sub, size=61)
    dark[y0:y1, x0:x1] = sub < local * a.umbral
    lab, n = ndimage.label(dark)
    objs = ndimage.find_objects(lab)
    blobs = []
    for i, sl in enumerate(objs, 1):
        area = int((lab[sl] == i).sum())
        h = sl[0].stop - sl[0].start
        w = sl[1].stop - sl[1].start
        if area < 12 or h > 3 * w or w > 3 * h:
            continue
        cy, cx = ndimage.center_of_mass(lab == i) if area < 5000 else (None, None)
        if cy is None:
            continue
        blobs.append((cx, cy, area))
    blobs = np.array(blobs)
    areas = blobs[:, 2]
    # Separar arrastre (chicos) de datos (grandes) por k-medias en el área.
    lo, hi = np.percentile(areas, 10), np.percentile(areas, 90)
    for _ in range(20):
        thr = (lo + hi) / 2
        lo = areas[areas < thr].mean()
        hi = areas[areas >= thr].mean()
    small = blobs[areas < thr]
    # Curva del arrastre (cuadrática, la cinta puede estar curvada), robusta.
    p = np.polyfit(small[:, 0], small[:, 1], 1)
    for deg in (1, 2, 2):
        r = np.abs(small[:, 1] - np.polyval(p, small[:, 0]))
        keep = r < max(8, np.percentile(r, 70) * 2)
        p = np.polyfit(small[keep, 0], small[keep, 1], deg)
    sy = lambda x: np.polyval(p, x)
    sx_raw = np.sort(small[np.abs(small[:, 1] - sy(small[:, 0])) < 12][:, 0])
    pitch = np.median(np.diff(sx_raw))
    # Todo agujero sobre la línea de arrastre es de arrastre, sea del tamaño que sea.
    on_line = np.abs(blobs[:, 1] - sy(blobs[:, 0])) < pitch * 0.45
    sprk = blobs[on_line]
    data = blobs[~on_line]
    sx = list(np.sort(sprk[:, 0]))
    # Rellenar agujeros de arrastre faltantes por interpolación.
    filled = [sx[0]]
    for x in sx[1:]:
        k = int(round((x - filled[-1]) / pitch))
        for j in range(1, k):
            filled.append(filled[-1] + (x - filled[-1]) / (k - j + 1))
        if k >= 1:
            filled.append(x)
    sx = np.array(filled)
    # Desplazamiento perpendicular de cada agujero de datos.
    off = data[:, 1] - sy(data[:, 0])
    above = off < 0
    side_up = above.sum()
    # El lado con 3 canales tiene más dispersión de offsets.
    up_off = np.sort(-off[above])
    dn_off = np.sort(off[~above])
    span_up = np.percentile(up_off, 95) if len(up_off) else 0
    span_dn = np.percentile(dn_off, 95) if len(dn_off) else 0
    three_up = span_up > span_dn
    gap = pitch  # separación entre canales ≈ paso del arrastre
    print(f"agujeros: {len(sprk)} de arrastre, {len(data)} de datos; paso {pitch:.1f}px; "
          f"lado de 3 canales: {'arriba' if three_up else 'abajo'}")

    # Canal de cada agujero de datos según su distancia al arrastre.
    def channel(o):
        d = -o if three_up else o  # positivo hacia el lado de 3 canales
        k = int(round(abs(d) / gap))
        if d > 0:
            return {1: 4, 2: 8, 3: 16}.get(k)
        return {1: 2, 2: 1}.get(k)

    # Asignar cada agujero a la fila (agujero de arrastre) más cercana en x.
    rows = {i: 0 for i in range(len(sx))}
    bad = 0
    for (cx, cy, _), o in zip(data, off):
        i = int(np.argmin(np.abs(sx - cx)))
        if abs(sx[i] - cx) > pitch * 0.4:
            bad += 1
            continue
        c = channel(o)
        if c is None:
            bad += 1
            continue
        rows[i] |= c
    vals = [rows[i] for i in range(len(sx))]
    # Huecos: si falta algún agujero de arrastre, avisar.
    gaps = np.diff(sx) / pitch
    holes_missing = [i for i, gq in enumerate(gaps) if gq > 1.5]
    if holes_missing:
        print("aviso: faltan agujeros de arrastre después de las filas", holes_missing)
    if bad:
        print(f"aviso: {bad} agujeros sin asignar")
    if a.invertir:
        vals = vals[::-1]
    print(" ".join(str(v) for v in vals))

    if a.debug:
        dbg = im.convert("RGB")
        d = ImageDraw.Draw(dbg)
        for cx, cy, _ in sprk:
            d.ellipse((cx - 4, cy - 4, cx + 4, cy + 4), outline=(0, 0, 255), width=2)
        for (cx, cy, _), o in zip(data, off):
            c = channel(o)
            col = (0, 200, 0) if c else (255, 0, 0)
            d.ellipse((cx - 9, cy - 9, cx + 9, cy + 9), outline=col, width=3)
            if c:
                d.text((cx - 4, cy + 10), str(c), fill=(255, 0, 0))
        for i, x in enumerate(sx):
            d.text((x - 6, 2), str(vals[i] if not a.invertir else rows[i]), fill=(255, 0, 255))
        dbg.save(a.debug)


if __name__ == "__main__":
    main()
