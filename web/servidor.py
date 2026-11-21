"""Servidor HTTP local de la consola web de Clementina (WASM + catálogo).

Sirve `web/index.html`, `web/catalogo.json`, el módulo `mercury_wasm.wasm`,
los programas declarados en el catálogo y, cuando existen checkpoints o cintas
en `target/tiny/`, las variantes de TINY 2.1 mediante `ports/tiny/en_vivo.py`.
"""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import re
import sys
from urllib.parse import parse_qs, unquote, urlparse
import webbrowser

WEB_DIR = Path(__file__).resolve().parent
REPO = WEB_DIR.parent
_TINY_MOD = None
DIRS_MIDI = ("still_alive", "demo_midi")
NOMBRE_MIDI = re.compile(r"(?:still_alive(?:_m12)?|sa_[a-z0-9_]+)\.(?:mer|tambor|mid|txt)", re.ASCII)


def archivos_midi():
    """Lista sólo archivos normales con nombres admitidos dentro de target."""
    target = REPO / "target"
    permitidos = {}
    for nombre_dir in DIRS_MIDI:
        carpeta = target / nombre_dir
        try:
            if carpeta.resolve() != carpeta or not carpeta.is_dir():
                continue
            for archivo in carpeta.iterdir():
                if (NOMBRE_MIDI.fullmatch(archivo.name)
                        and archivo.resolve() == archivo and archivo.is_file()):
                    permitidos[f"{nombre_dir}/{archivo.name}"] = archivo
        except (OSError, RuntimeError):
            continue
    return permitidos


def midi_disponibles():
    conjuntos = {}
    for ruta in sorted(archivos_midi()):
        base, extension = ruta.rsplit(".", 1)
        conjuntos.setdefault(base, {})[extension] = f"/midi/archivo/{ruta}"
    return [
        {"id": base, "nombre": base.split("/")[1], "archivos": archivos}
        for base, archivos in conjuntos.items()
        if "mid" in archivos or {"mer", "tambor"} <= archivos.keys()
    ]


def ruta_wasm():
    """Devuelve la ruta de `mercury_wasm.wasm` si fue compilado."""
    candidatos = [
        REPO / "target" / "wasm32-unknown-unknown" / "release" / "mercury_wasm.wasm",
        REPO / "target" / "wasm32-unknown-unknown" / "debug" / "mercury_wasm.wasm",
        WEB_DIR / "mercury_wasm.wasm",
    ]
    for p in candidatos:
        if p.is_file():
            return p
    return None


def archivos_permitidos():
    """Devuelve el conjunto de rutas relativas declaradas en `web/catalogo.json`."""
    cat = json.loads((WEB_DIR / "catalogo.json").read_text(encoding="utf-8"))
    return {
        Path(p[campo]).as_posix()
        for p in cat.get("programas", [])
        for campo in ("archivo", "archivo_datos", "archivo_catalogo")
        if p.get(campo)
    }


def cargar_tiny_modulo(tiny_mod=None):
    """Carga perezosamente `ports/tiny/en_vivo.py` si está disponible."""
    global _TINY_MOD
    if tiny_mod is not None:
        _TINY_MOD = tiny_mod
        return _TINY_MOD
    if _TINY_MOD is not None:
        return _TINY_MOD
    ruta = REPO / "ports" / "tiny" / "en_vivo.py"
    if not ruta.is_file():
        return None
    spec = importlib.util.spec_from_file_location("clementina_tiny_en_vivo", ruta)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = mod
    spec.loader.exec_module(mod)
    _TINY_MOD = mod
    return mod


def tiny_disponibles(tiny_mod=None):
    """Lista las variantes de TINY que tienen checkpoint o cinta armada en disco."""
    mod = cargar_tiny_modulo(tiny_mod)
    if mod is None or not hasattr(mod, "VARIANTES"):
        return []
    salida = []
    for clave, cfg in mod.VARIANTES.items():
        cinta = REPO / "target" / "tiny" / cfg["dir_nativo"] / "tiny.bin"
        disponible = cfg["checkpoint"].is_file() or cinta.is_file()
        salida.append({
            "id": f"tiny-{clave}",
            "variante": clave,
            "nombre": cfg["nombre"],
            "disponible": disponible,
        })
    return salida


def cors_headers(handler):
    handler.send_header("Access-Control-Allow-Origin", "*")
    handler.send_header("Access-Control-Allow-Methods", "GET, OPTIONS")
    handler.send_header("Access-Control-Allow-Headers", "*")


def enviar_bytes(handler, status, content_type, data, cache_control=None):
    handler.send_response(status)
    cors_headers(handler)
    handler.send_header("Content-Type", content_type)
    handler.send_header("Content-Length", str(len(data)))
    if cache_control:
        handler.send_header("Cache-Control", cache_control)
    handler.end_headers()
    handler.wfile.write(data)


def despachar_ruta_comun(handler, url, qs, tiny_mod=None):
    """Atiende las rutas estáticas, catálogo, WASM y endpoints `/tiny/*`.

    Devuelve `True` si la ruta fue respondida, o `False` si el llamador debe
    resolver otra ruta propia (p. ej. `/info` o `/correr` en `en_vivo.py`).
    """
    path = url.path

    if path in ("/", "/index.html"):
        enviar_bytes(handler, 200, "text/html; charset=utf-8", (WEB_DIR / "index.html").read_bytes())
        return True

    if path == "/favicon.ico":
        handler.send_response(204)
        cors_headers(handler)
        handler.end_headers()
        return True

    if path == "/catalogo.json":
        enviar_bytes(handler, 200, "application/json; charset=utf-8", (WEB_DIR / "catalogo.json").read_bytes())
        return True

    if path == "/mercury_wasm.wasm":
        wasm_path = ruta_wasm()
        if wasm_path is None:
            msg = (
                "Falta mercury_wasm.wasm. Compilalo con:\n"
                "  cargo build -p mercury-wasm --target wasm32-unknown-unknown --release\n"
            ).encode("utf-8")
            enviar_bytes(handler, 404, "text/plain; charset=utf-8", msg)
            return True
        enviar_bytes(handler, 200, "application/wasm", wasm_path.read_bytes(), cache_control="no-store")
        return True

    if path == "/midi/disponibles":
        cuerpo = json.dumps({"variantes": midi_disponibles()}).encode("utf-8")
        enviar_bytes(handler, 200, "application/json; charset=utf-8", cuerpo, cache_control="no-store")
        return True

    if path.startswith("/midi/archivo/"):
        nombre = unquote(path[len("/midi/archivo/"):])
        archivo = archivos_midi().get(nombre)
        try:
            datos = archivo.read_bytes() if archivo else None
        except OSError:
            datos = None
        if datos is None:
            enviar_bytes(handler, 404, "text/plain; charset=utf-8", b"Archivo MIDI no encontrado")
        else:
            tipo = {
                ".mer": "text/plain; charset=utf-8", ".txt": "text/plain; charset=utf-8",
                ".mid": "audio/midi", ".tambor": "application/octet-stream",
            }[archivo.suffix]
            enviar_bytes(handler, 200, tipo, datos, cache_control="no-store")
        return True

    if path.startswith("/archivo/"):
        rel = unquote(path[len("/archivo/"):]).lstrip("/")
        norm = Path(rel).as_posix()
        if norm not in archivos_permitidos():
            enviar_bytes(handler, 404, "text/plain; charset=utf-8", b"Archivo fuera del catalogo")
            return True
        archivo = (REPO / norm).resolve()
        if not archivo.is_file():
            enviar_bytes(handler, 404, "text/plain; charset=utf-8", b"Archivo no encontrado")
            return True
        enviar_bytes(handler, 200, "text/plain; charset=utf-8", archivo.read_bytes())
        return True

    if path == "/tiny/disponibles":
        cuerpo = json.dumps({"variantes": tiny_disponibles(tiny_mod)}).encode("utf-8")
        enviar_bytes(handler, 200, "application/json; charset=utf-8", cuerpo)
        return True

    if path in ("/tiny/info", "/tiny/cinta"):
        mod = cargar_tiny_modulo(tiny_mod)
        if mod is None:
            enviar_bytes(handler, 404, "application/json; charset=utf-8", b'{"error":"tiny no disponible"}')
            return True
        clave = qs.get("variante", ["compacto"])[0]
        if clave not in mod.VARIANTES:
            clave = "compacto"
        try:
            info = mod.preparar_variante(clave)
        except Exception as exc:
            cuerpo = json.dumps({"error": str(exc)}).encode("utf-8")
            enviar_bytes(handler, 404, "application/json; charset=utf-8", cuerpo)
            return True
        if path == "/tiny/info":
            cuerpo = json.dumps({k: v for k, v in info.items() if k != "cinta_path"}).encode("utf-8")
            enviar_bytes(handler, 200, "application/json; charset=utf-8", cuerpo)
            return True
        cinta = Path(info["cinta_path"]).read_bytes()
        enviar_bytes(handler, 200, "application/octet-stream", cinta)
        return True

    return False


class ManejadorConsola(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass

    def do_OPTIONS(self):
        self.send_response(204)
        cors_headers(self)
        self.end_headers()

    def do_GET(self):
        url = urlparse(self.path)
        qs = parse_qs(url.query)
        if despachar_ruta_comun(self, url, qs):
            return
        enviar_bytes(self, 404, "text/plain; charset=utf-8", b"No encontrado")


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--puerto", type=int, default=7661)
    cli.add_argument("--programa", default=None, help="ID de programa inicial en el catálogo")
    cli.add_argument("--no-abrir", action="store_true")
    args = cli.parse_args()

    if ruta_wasm() is None:
        print(
            "Aviso: falta target/wasm32-unknown-unknown/release/mercury_wasm.wasm.\n"
            "Compilalo con: cargo build -p mercury-wasm --target wasm32-unknown-unknown --release",
            file=sys.stderr,
        )

    servidor = ThreadingHTTPServer(("127.0.0.1", args.puerto), ManejadorConsola)
    sufijo = f"?programa={args.programa}" if args.programa else ""
    url = f"http://127.0.0.1:{args.puerto}/{sufijo}"
    print(f"Consola web de Clementina escuchando en {url}", flush=True)
    if not args.no_abrir:
        webbrowser.open(url)
    try:
        servidor.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
