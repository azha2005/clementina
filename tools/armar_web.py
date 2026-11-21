"""Arma la consola web como sitio estático en `dist/`.

    python tools/armar_web.py               compila el WASM y arma dist/
    python tools/armar_web.py --sin-compilar  usa el .wasm ya compilado
    python tools/armar_web.py --url https://clementina.pages.dev/
                                            agrega la tarjeta para redes

La carpeta repite las rutas que atiende `web/servidor.py` (`catalogo.json`,
`mercury_wasm.wasm`, `archivo/<ruta>`), así que `index.html` es el mismo en
los dos casos y anda en la raíz o en una subcarpeta de cualquier hosting
estático. Lleva sólo los archivos que nombra el catálogo, nada de las cintas
del museo ni de los MIDI locales (Still Alive no es nuestro).

TINY 2.1 (MIT, de WollyMonjac) entra si están sus checkpoints: el script
arma de antemano lo que `servidor.py` prepara en el momento y lo deja en
`tiny/<variante>/info.json` y `tiny/<variante>/cinta.bin`, con la licencia
en `tiny/LICENCIA_TINY.txt`. `--sin-tiny` lo deja afuera.

Incluye las cabeceras para Vercel (`vercel.json`), Cloudflare Pages y
Netlify (`_headers`) y GitHub Pages (`.nojekyll`). No publica nada: eso se
hace a mano, por ejemplo `npx vercel deploy dist --prod` o
`npx wrangler pages deploy dist`.
"""

import argparse
import gzip
import importlib.util
import json
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
WEB = REPO / "web"
DIST = REPO / "dist"
WASM = REPO / "target" / "wasm32-unknown-unknown" / "release" / "mercury_wasm.wasm"
MARCA = ".clementina-dist"

# Sólo se publican archivos de estas carpetas, y nunca los que llevan datos
# personales (la cinta 3 del museo tiene un nombre real; ver AGENTS.md).
CARPETAS_PERMITIDAS = ("examples/", "samples/")
PROHIBIDOS = {"examples/nombre.mer", "examples/nombre_ferranti.mer"}

PAGINA_404 = """<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Clementina: no está</title>
<style>
  :root { --fondo: #e4e3df; --tinta: #26271f; }
  body { margin: 0; min-height: 100vh; display: grid; place-items: center;
         background: var(--fondo); color: var(--tinta);
         font: 16px/1.5 system-ui, sans-serif; padding: 16px; }
</style>
</head>
<body>
<p>Esta cinta no está en el lector. <a href="./">Volver al pupitre</a>.</p>
</body>
</html>
"""

CABECERAS = """/mercury_wasm.wasm
  Content-Type: application/wasm
  Cache-Control: public, max-age=3600
/archivo/*
  Content-Type: text/plain; charset=utf-8
/catalogo.json
  Cache-Control: public, max-age=300
"""

VERCEL = {
    "headers": [
        {"source": "/mercury_wasm.wasm", "headers": [
            {"key": "Content-Type", "value": "application/wasm"},
            {"key": "Cache-Control", "value": "public, max-age=3600"},
        ]},
        {"source": "/archivo/(.*)", "headers": [
            {"key": "Content-Type", "value": "text/plain; charset=utf-8"},
        ]},
    ],
}


def etiquetas_redes(url):
    """Etiquetas para que X y otros muestren la tarjeta; piden direcciones completas."""
    if not url.startswith("https://"):
        sys.exit("--url tiene que empezar con https://")
    url = url.rstrip("/") + "/"
    titulo = "Clementina"
    texto = "La Ferranti Mercury de la FCEN-UBA, emulada casi 1:1 y abierta, para operarla desde el navegador."
    imagen = url + "tarjeta.png"
    return "\n".join([
        '<meta property="og:type" content="website">',
        f'<meta property="og:url" content="{url}">',
        f'<meta property="og:title" content="{titulo}">',
        f'<meta property="og:description" content="{texto}">',
        f'<meta property="og:image" content="{imagen}">',
        '<meta property="og:image:width" content="1200">',
        '<meta property="og:image:height" content="630">',
        '<meta name="twitter:card" content="summary_large_image">',
        f'<meta name="twitter:title" content="{titulo}">',
        f'<meta name="twitter:description" content="{texto}">',
        f'<meta name="twitter:image" content="{imagen}">',
    ])


def archivos_del_catalogo(catalogo):
    rutas = set()
    for p in catalogo.get("programas", []):
        for campo in ("archivo", "archivo_datos", "archivo_catalogo"):
            if p.get(campo):
                rutas.add(Path(p[campo]).as_posix())
    for ruta in sorted(rutas):
        if ruta in PROHIBIDOS or not ruta.startswith(CARPETAS_PERMITIDAS) or ".." in ruta:
            sys.exit(f"El catálogo nombra {ruta}, que no se publica.")
        if not (REPO / ruta).is_file():
            sys.exit(f"Falta {ruta}, que nombra el catálogo.")
    return sorted(rutas)


def compilar():
    orden = ["cargo", "build", "-p", "mercury-wasm",
             "--target", "wasm32-unknown-unknown", "--release"]
    print("$", " ".join(orden))
    subprocess.run(orden, cwd=REPO, check=True)


def vaciar_dist():
    if DIST.exists():
        if not (DIST / MARCA).is_file():
            sys.exit(f"{DIST} existe y no lo armó este script; no lo toco.")
        shutil.rmtree(DIST)
    DIST.mkdir()
    (DIST / MARCA).write_text("Carpeta generada por tools/armar_web.py\n", encoding="utf-8")


def exportar_tiny():
    """Arma las variantes de TINY disponibles. Devuelve cuántas entraron."""
    ruta = REPO / "ports" / "tiny" / "en_vivo.py"
    if not ruta.is_file():
        return 0
    spec = importlib.util.spec_from_file_location("clementina_tiny_en_vivo", ruta)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = mod
    spec.loader.exec_module(mod)
    disponibles = []
    for clave, cfg in mod.VARIANTES.items():
        cinta = REPO / "target" / "tiny" / cfg["dir_nativo"] / "tiny.bin"
        if not (cfg["checkpoint"].is_file() or cinta.is_file()):
            print(f"TINY {clave}: falta el checkpoint, queda afuera.")
            continue
        info = mod.preparar_variante(clave)
        carpeta = DIST / "tiny" / clave
        carpeta.mkdir(parents=True, exist_ok=True)
        publico = {k: v for k, v in info.items() if k != "cinta_path"}
        (carpeta / "info.json").write_text(json.dumps(publico, ensure_ascii=False), encoding="utf-8")
        shutil.copy2(info["cinta_path"], carpeta / "cinta.bin")
        disponibles.append({"id": f"tiny-{clave}", "variante": clave, "nombre": cfg["nombre"], "disponible": True})
    if disponibles:
        (DIST / "tiny" / "disponibles.json").write_text(json.dumps({"variantes": disponibles}, ensure_ascii=False),
                                                       encoding="utf-8")
        shutil.copy2(REPO / "ports" / "tiny" / "LICENCIA_TINY.txt", DIST / "tiny" / "LICENCIA_TINY.txt")
    return len(disponibles)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--sin-compilar", action="store_true", help="no corre cargo build")
    ap.add_argument("--sin-tiny", action="store_true", help="no incluye TINY 2.1")
    ap.add_argument("--url", help="dirección pública del sitio, para las etiquetas og: y twitter:")
    args = ap.parse_args()

    if not args.sin_compilar:
        compilar()
    if not WASM.is_file():
        sys.exit("Falta mercury_wasm.wasm: corré sin --sin-compilar.")

    catalogo = json.loads((WEB / "catalogo.json").read_text(encoding="utf-8"))
    rutas = archivos_del_catalogo(catalogo)

    vaciar_dist()
    pagina = (WEB / "index.html").read_text(encoding="utf-8")
    if args.url:
        pagina = pagina.replace("</title>", "</title>\n" + etiquetas_redes(args.url), 1)
    else:
        print("Sin --url: la página no lleva tarjeta para redes.")
    (DIST / "index.html").write_text(pagina, encoding="utf-8", newline="")
    for nombre in ("logo.svg", "tarjeta.png"):
        shutil.copy2(WEB / nombre, DIST / nombre)
    # Sin servidor no hay MIDI locales, y TINY va en archivos fijos.
    estatico = {"sitio_estatico": True, **catalogo}
    (DIST / "catalogo.json").write_text(json.dumps(estatico, ensure_ascii=False, indent=1) + "\n",
                                        encoding="utf-8")
    shutil.copy2(WASM, DIST / "mercury_wasm.wasm")
    for ruta in rutas:
        destino = DIST / "archivo" / ruta
        destino.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / ruta, destino)
    (DIST / "404.html").write_text(PAGINA_404, encoding="utf-8")
    (DIST / "_headers").write_text(CABECERAS, encoding="utf-8")
    (DIST / "vercel.json").write_text(json.dumps(VERCEL, indent=2) + "\n", encoding="utf-8")
    (DIST / ".nojekyll").write_text("", encoding="utf-8")
    if not args.sin_tiny:
        print(f"TINY: {exportar_tiny()} variantes.")

    archivos = [p for p in DIST.rglob("*") if p.is_file()]
    total = sum(p.stat().st_size for p in archivos)
    comprimido = sum(len(gzip.compress(p.read_bytes(), 9)) for p in archivos)
    print(f"dist/ listo: {len(archivos)} archivos, {total / 1024:.0f} KiB "
          f"({comprimido / 1024:.0f} KiB con gzip).")


if __name__ == "__main__":
    main()
