"""Arma la tarjeta para redes (1200 × 630) con capturas de la consola real.

    python tools/armar_web.py --sin-compilar      primero, para tener dist/
    python tools/armar_tarjeta.py --url https://clementina.pages.dev/

Sirve `dist/` en un puerto local, corre cuatro programas con Chrome
(Playwright, canal `chrome`), saca las capturas y compone la tarjeta en
`web/tarjeta.png`. Después, `armar_web.py --url ...` la copia a `dist/` y
agrega las etiquetas `og:` y `twitter:` para que X y otros la muestren.
"""

import argparse
import base64
import functools
import http.server
import threading
from pathlib import Path

from playwright.sync_api import sync_playwright

REPO = Path(__file__).resolve().parent.parent
DIST = REPO / "dist"
SALIDA = REPO / "web" / "tarjeta.png"
CAPTURAS = REPO / "target" / "tarjeta"
LISTO = "typeof wasm !== 'undefined' && wasm && programa"


def servir_dist():
    manejador = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(DIST))
    manejador.log_message = lambda *a: None
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), manejador)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, f"http://127.0.0.1:{srv.server_address[1]}/"


def correr(pg, base, programa, espera_ms, preparar=""):
    pg.goto(f"{base}?programa={programa}")
    pg.wait_for_function(LISTO)
    pg.wait_for_timeout(600)
    if preparar:
        pg.evaluate(preparar)
    pg.evaluate("[...document.querySelectorAll('#speeds button')].find(b => b.textContent.includes('Máx')).click();"
                "$('runBtn').click()")
    pg.wait_for_timeout(espera_ms)


def capturas(base):
    CAPTURAS.mkdir(parents=True, exist_ok=True)
    with sync_playwright() as p:
        nav = p.chromium.launch(channel="chrome")
        pg = nav.new_page(viewport={"width": 1400, "height": 1000}, device_scale_factor=2)
        correr(pg, base, "nim", 5000)
        pg.locator("#pupitre").screenshot(path=str(CAPTURAS / "pupitre.png"))
        pg.locator(".tubos").screenshot(path=str(CAPTURAS / "tubos.png"))
        correr(pg, base, "primos_ferranti", 9000)
        pg.locator("#teleimpresor").screenshot(path=str(CAPTURAS / "teleimpresor.png"))
        pg.locator("#lector").screenshot(path=str(CAPTURAS / "lector.png"))
        pg.locator(".por-dentro .grilla").screenshot(path=str(CAPTURAS / "por_dentro.png"))
        correr(pg, base, "elecciones", 20000)
        pg.locator("#teleimpresor").screenshot(path=str(CAPTURAS / "elecciones.png"))
        nav.close()


def tarjeta(url, base):
    def img(nombre):
        datos = (CAPTURAS / nombre).read_bytes()
        return "data:image/png;base64," + base64.b64encode(datos).decode()

    sitio = url.removeprefix("https://").removeprefix("http://").rstrip("/")
    logo = "data:image/svg+xml;base64," + base64.b64encode((REPO / "web" / "logo.svg").read_bytes()).decode()
    html = f"""<!doctype html><html lang="es"><head><meta charset="utf-8">
<link href="https://fonts.googleapis.com/css2?family=Archivo:wdth,wght@62..125,400..700&family=Courier+Prime:wght@700&display=swap" rel="stylesheet">
<style>
  :root {{ --fondo: oklch(90% 0.006 95); --tinta: oklch(23% 0.01 95); --gris: oklch(45% 0.01 95);
          --acento: oklch(48% 0.12 152); --sombra: oklch(20% 0.01 95 / 0.22); }}
  html, body {{ margin: 0; }}
  body {{ width: 1200px; height: 630px; background: var(--fondo); color: var(--tinta); position: relative;
         font-family: Archivo, sans-serif; overflow: hidden; }}
  .texto {{ position: absolute; inset: 0 auto 0 0; width: 480px; padding: 56px 0 48px 56px; box-sizing: border-box;
           display: flex; flex-direction: column; }}
  .marca {{ height: 74px; width: auto; align-self: flex-start; }}
  h1 {{ margin: 22px 0 0; font-family: 'Courier Prime', monospace; font-size: 62px; line-height: 0.95; font-weight: 700; letter-spacing: 0.02em; }}
  .fechas {{ margin: 18px 0 0; font-size: 38px; color: var(--acento); font-weight: 550; }}
  .bajada {{ margin: 22px 0 0; font-size: 25px; line-height: 1.36; color: var(--gris); }}
  .url {{ margin: auto 0 0; font-size: 25px; font-weight: 600; }}
  /* Sin la línea de registros de abajo del pupitre. */
  .marco {{ position: absolute; left: 480px; top: 50%; width: 704px; height: 474px; transform: translateY(-50%); overflow: hidden; }}
  .marco img {{ width: 100%; display: block; }}
</style></head><body>
<div class="texto">
  <img class="marca" src="{logo}" alt="">
  <h1>CLEMENTINA</h1>
  <p class="fechas">Ferranti Mercury, 1961</p>
  <p class="bajada">Emulada casi 1:1 y abierta, para operarla desde el navegador.</p>
  <p class="url">{sitio}</p>
</div>
<div class="marco"><img src="{img('pupitre.png')}" alt=""></div>
</body></html>"""
    pagina = CAPTURAS / "tarjeta.html"
    pagina.write_text(html, encoding="utf-8")
    with sync_playwright() as p:
        nav = p.chromium.launch(channel="chrome")
        pg = nav.new_page(viewport={"width": 1200, "height": 630}, device_scale_factor=1)
        pg.goto(pagina.as_uri())
        pg.wait_for_timeout(1500)
        pg.screenshot(path=str(SALIDA))
        nav.close()


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--url", required=True, help="dirección pública del sitio, la que se lee en la tarjeta")
    ap.add_argument("--sin-capturas", action="store_true", help="reusa las capturas de target/tarjeta")
    args = ap.parse_args()
    if not (DIST / "index.html").is_file():
        raise SystemExit("Falta dist/: corré antes tools/armar_web.py.")
    srv, base = servir_dist()
    try:
        if not args.sin_capturas:
            capturas(base)
        tarjeta(args.url, base)
    finally:
        srv.shutdown()
    print(f"Tarjeta en {SALIDA.relative_to(REPO)}")


if __name__ == "__main__":
    main()
