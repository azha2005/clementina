"""Servidor HTTP local que conecta la visualización de Clementina con el
emulador real (`tiny_en_vivo.exe`), permitiendo escribir cualquier pregunta
y ver en tiempo real las filas de cinta Ferranti, los registros A/B/C, los
accesos al tambor de 512 sectores, los bancos de núcleos y el teleimpresor.
"""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
import threading
from urllib.parse import parse_qs, urlparse
import webbrowser

RUTA = Path(__file__).resolve().parent
REPO = RUTA.parent.parent
TINY68K = Path.home() / "Downloads" / "tiny68k"

VARIANTES = {
    "compacto": {
        "nombre": "r3_80x3L2x224 (Compacto · 3 bloques · 3-bit SEQ · 512/512 sectores)",
        "checkpoint": TINY68K / "models_retro" / "tiny_retro_3bit" / "r3_80x3L2x224.pt",
        "dir_nativo": "nativo-compacto",
        "bloques": 3,
        "dimension": 80,
        "intermedia": 224,
        "ternario": False,
        "compacto": True,
    },
    "ternario": {
        "nombre": "rt3_96_wd05 (Ternario · 2 bloques · {-1,0,+1} · 482/512 sectores)",
        "checkpoint": TINY68K / "models_retro" / "tiny_retro_exp3" / "rt3_96_wd05.pt",
        "dir_nativo": "nativo-ternario",
        "bloques": 2,
        "dimension": 96,
        "intermedia": 384,
        "ternario": True,
        "compacto": False,
    },
    "3bit": {
        "nombre": "r3_80x2L2x256 (2 bloques · 3-bit SEQ · 462/512 sectores)",
        "checkpoint": TINY68K / "models_retro" / "tiny_retro_exp3" / "r3_80x2L2x256.pt",
        "dir_nativo": "nativo",
        "bloques": 2,
        "dimension": 80,
        "intermedia": 256,
        "ternario": False,
        "compacto": False,
    },
}

LOCK = threading.Lock()
CACHE_INFO = {}


def asegurar_binarios():
    exe = REPO / "target" / "release" / "examples" / "tiny_en_vivo.exe"
    grab = REPO / "target" / "release" / "examples" / "grabar_cinta.exe"
    if not exe.exists() or not grab.exists():
        subprocess.run(
            ["cargo", "build", "--release", "-p", "mercury-core",
             "--example", "tiny_en_vivo", "--example", "grabar_cinta"],
            cwd=REPO, check=True,
        )
    return exe, grab


def preparar_variante(clave):
    with LOCK:
        if clave in CACHE_INFO:
            return CACHE_INFO[clave]
        cfg = VARIANTES[clave]
        exe, grab = asegurar_binarios()
        destino = REPO / "target" / "tiny" / cfg["dir_nativo"]
        imagen = destino / "modelo.bin"
        cinta = destino / "tiny.bin"
        destino.mkdir(parents=True, exist_ok=True)
        if not imagen.exists():
            subprocess.run(
                [sys.executable, str(RUTA / "exportar.py"), str(cfg["checkpoint"]), str(imagen)],
                cwd=REPO, check=True,
            )
        if not cinta.exists() or not (destino / "datos.bin").exists():
            subprocess.run(
                [sys.executable, str(RUTA / "nativo.py"), str(imagen), str(destino)],
                cwd=REPO, check=True,
            )
            bancos = json.loads((destino / "bancos.json").read_text(encoding="utf-8"))
            sectores = []
            for i in range(0, len(bancos), 2):
                sectores.extend([str(bancos[i]), str(destino / bancos[i + 1])])
            subprocess.run(
                [str(grab), str(destino / "datos.bin"), str(destino / "arranque.mer"),
                 str(destino / "tiny"), *sectores],
                cwd=REPO, check=True,
            )
        info = construir_mapa(clave, cfg, imagen, cinta)
        CACHE_INFO[clave] = info
        return info


def construir_ordenes(bloques):
    ordenes = [{"capa": -1, "nombre": "embedding", "banco": "auxiliares"}]
    for capa in range(2 * bloques):
        ordenes.append({"capa": capa, "nombre": "loop_emb", "banco": "auxiliares"})
        ordenes.append({"capa": capa, "nombre": "n1", "banco": "auxiliares"})
        if capa % 2 == 0:
            ordenes.append({"capa": capa, "nombre": "k", "banco": "matriz"})
            ordenes.append({"capa": capa, "nombre": "K→tambor", "banco": "atención"})
            ordenes.append({"capa": capa, "nombre": "v", "banco": "matriz"})
            ordenes.append({"capa": capa, "nombre": "V→tambor", "banco": "atención"})
        if capa == 2 * bloques - 2:
            ordenes.append({"capa": capa, "nombre": "prefijo", "banco": "control"})
        ordenes.append({"capa": capa, "nombre": "q", "banco": "matriz"})
        ordenes.append({"capa": capa, "nombre": "atención", "banco": "atención"})
        ordenes.append({"capa": capa, "nombre": "sn1", "banco": "auxiliares"})
        ordenes.append({"capa": capa, "nombre": "o", "banco": "matriz"})
        ordenes.append({"capa": capa, "nombre": "+residual", "banco": "auxiliares"})
        ordenes.append({"capa": capa, "nombre": "n2", "banco": "auxiliares"})
        ordenes.append({"capa": capa, "nombre": "f1", "banco": "matriz"})
        ordenes.append({"capa": capa, "nombre": "ReLU²+sn2", "banco": "auxiliares"})
        ordenes.append({"capa": capa, "nombre": "f2", "banco": "matriz"})
        ordenes.append({"capa": capa, "nombre": "+residual", "banco": "auxiliares"})
    ordenes.append({"capa": 2 * bloques, "nombre": "nf", "banco": "auxiliares"})
    ordenes.append({"capa": 2 * bloques, "nombre": "logits (55)", "banco": "auxiliares"})
    ordenes.append({"capa": 2 * bloques, "nombre": "salida", "banco": "control"})
    return ordenes


def construir_mapa(clave, cfg, imagen, cinta):
    origen = struct.unpack("<65536H", imagen.read_bytes())
    compacto = cfg["compacto"]
    ternario = cfg["ternario"]
    bloques = cfg["bloques"]
    d, hid = cfg["dimension"], cfg["intermedia"]
    cabeza = d // 4
    paginas_mat = 4 if ternario or compacto else 3
    sectores_vector = 6 if compacto else 10
    sectores_kv = 2 * bloques * sectores_vector
    ctrl = origen[6] if compacto else origen[49] + 2
    aux = ctrl + (3 if compacto else 4)
    atn, mat_sector = aux + 6, aux + 12
    f1_sector = mat_sector if compacto else mat_sector + paginas_mat
    tabla = f1_sector + paginas_mat
    constantes, ordenes_sec = tabla + 1, tabla + 2
    kv = ordenes_sec + (6 if compacto else 4)

    cats = ["free"] * 512
    labels = ["libre"] * 512

    def pintar(a, b, cat, lbl):
        for s in range(max(0, a), min(512, b + 1)):
            cats[s] = cat
            labels[s] = lbl

    pintar(0, 1, "ir", "Input Routine (sectores 0–1 · tele-input)")
    if compacto:
        off = 256

        def alloc(n, cat, lbl):
            nonlocal off
            a = off // 128
            off += n
            b = (off - 1) // 128
            for s in range(a, b + 1):
                if cats[s] == "free" or s > a:
                    cats[s] = cat
                    labels[s] = lbl

        alloc(3961, "emb", "emb.weight · 55×80 a 9 bits (10 en 9 cortos)")
        alloc(481, "norm", "loop_emb · 6×80 (10 bits)")
        for c in range(6):
            for n in ("n1", "sn1", "n2", "sn2"):
                alloc(225 if n == "sn2" else 81, "norm", f"pass_norms.{c}.{n}.g")
        alloc(81, "norm", "nf.g")
        formas = (("q", d, d), ("k", cabeza, d), ("v", cabeza, d), ("o", d, d), ("f1", hid, d), ("f2", d, hid))
        for b in range(3):
            for nom, filas, cols in formas:
                alloc(math.ceil(cols / 5) * (filas // 2) * 3, f"mat{b}", f"bloque {b} · {nom} · 3-bit SEQ")
                alloc(2 + 2 * filas, "coef", f"bloque {b} · {nom} · α/8 y sesgo (10 bits)")
    else:
        pintar(2, 2, "ir", "Directorio del tambor")
        nombres_t = ["emb.weight", "loop_emb"] + [
            f"pass_norms.{c}.{n}.g" for c in range(4) for n in ("n1", "sn1", "n2", "sn2")
        ] + ["nf.g"]
        punteros = [(origen[6 + i] + 2, ("emb" if i == 0 else "norm"), nombres_t[i]) for i in range(19)]
        formas = ("q", "k", "v", "o", "f1", "f2")
        for b in range(2):
            for t, nom in enumerate(formas):
                campo = 25 + 2 * (6 * b + t)
                if origen[campo]:
                    punteros.append((origen[campo] + 2, f"mat{b}", f"bloque {b} · {nom}"))
                    punteros.append((origen[campo + 1] + 2, "coef", f"bloque {b} · {nom} · α y sesgo FP40"))
        punteros.sort(key=lambda x: x[0])
        for idx, (s0, cat, lbl) in enumerate(punteros):
            s1 = (punteros[idx + 1][0] - 1) if idx + 1 < len(punteros) else (ctrl - 1)
            pintar(s0, s1, cat, lbl)

    pintar(ctrl, aux - 1, "code", "control.mer (fijo en páginas 10–12)")
    pintar(aux, atn - 1, "code", "auxiliares.mer (overlay páginas 1–6)")
    pintar(atn, mat_sector - 1, "code", "atencion.mer (overlay páginas 1–6)")
    if compacto:
        pintar(mat_sector, tabla - 1, "code", "matriz.mer (q k v o f1 f2 · páginas 2–5)")
    else:
        pintar(mat_sector, f1_sector - 1, "code", "matriz.mer (q k v o f2)")
        pintar(f1_sector, tabla - 1, "code", "f1.mer (proyección f1)")
    pintar(tabla, tabla, "guion", "tabla código Ferranti ↔ tokens")
    pintar(constantes, constantes, "guion", "constantes FP40 (página 31)")
    pintar(ordenes_sec, kv - 1, "guion", f"guion de {89 if compacto else 61} órdenes")
    for b in range(bloques):
        base = kv + b * 2 * sectores_vector
        pintar(base, base + sectores_vector - 1, "kv", f"caché K · bloque {b}")
        pintar(base + sectores_vector, base + 2 * sectores_vector - 1, "kv", f"caché V · bloque {b}")

    bancos_map = {
        str(ctrl): "control",
        str(aux): "auxiliares",
        str(atn): "atención",
        str(mat_sector): "matriz",
        str(f1_sector): "f1" if not compacto else "matriz",
    }
    filas_prog = len(cinta.read_bytes())
    return {
        "variante": clave,
        "nombre": cfg["nombre"],
        "cinta_path": str(cinta),
        "filas_programa": filas_prog,
        "bloques": bloques,
        "guion_base": ordenes_sec,
        "kv_base": kv,
        "kv_sectores_vector": sectores_vector,
        "kv_sectores": sectores_kv,
        "bancos": bancos_map,
        "cats": cats,
        "labels": labels,
        "ordenes": construir_ordenes(bloques),
    }


def _servidor_web():
    web_dir = str(REPO / "web")
    if web_dir not in sys.path:
        sys.path.insert(0, web_dir)
    import servidor
    return servidor


class Manejador(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass

    def _cors(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "*")

    def do_OPTIONS(self):
        self.send_response(204)
        self._cors()
        self.end_headers()

    def do_GET(self):
        url = urlparse(self.path)
        qs = parse_qs(url.query)
        srv = _servidor_web()
        if srv.despachar_ruta_comun(self, url, qs, tiny_mod=sys.modules[__name__]):
            return

        if url.path == "/info":
            clave = qs.get("variante", ["compacto"])[0]
            if clave not in VARIANTES:
                clave = "compacto"
            info = preparar_variante(clave)
            cuerpo = json.dumps({k: v for k, v in info.items() if k != "cinta_path"}).encode("utf-8")
            self.send_response(200)
            self._cors()
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(cuerpo)))
            self.end_headers()
            self.wfile.write(cuerpo)
            return

        if url.path == "/correr":
            clave = qs.get("variante", ["compacto"])[0]
            if clave not in VARIANTES:
                clave = "compacto"
            pregunta = qs.get("pregunta", ["name the capital of argentina"])[0].strip().lower()
            cada_ms = int(qs.get("cada_ms", ["10000"])[0])
            cada_ms = max(1000, min(200000, cada_ms))
            # Modo difícil: llaves de mano F y WRITE CURRENT del pupitre.
            try:
                llaves = int(qs.get("llaves", ["4"])[0]) & 0x3FF
            except ValueError:
                llaves = 4
            escritura = qs.get("escritura", ["1"])[0] != "0"
            info = preparar_variante(clave)
            exe = REPO / "target" / "release" / "examples" / "tiny_en_vivo.exe"

            self.send_response(200)
            self._cors()
            self.send_header("Content-Type", "application/x-ndjson; charset=utf-8")
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()

            meta = {k: v for k, v in info.items() if k != "cinta_path"}
            meta["tipo"] = "info"
            meta["pregunta"] = pregunta
            self.wfile.write((json.dumps(meta) + "\n").encode("utf-8"))
            self.wfile.flush()

            argumentos = [str(exe), info["cinta_path"], "--pregunta", pregunta,
                          "--cada-ms", str(cada_ms), "--llaves", str(llaves)]
            if not escritura:
                argumentos.append("--sin-escritura")
            proc = subprocess.Popen(
                argumentos,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=REPO,
            )
            try:
                for linea in proc.stdout:
                    self.wfile.write(linea)
                    self.wfile.flush()
            except OSError:
                proc.kill()
            finally:
                proc.wait()
            return

        self.send_response(404)
        self._cors()
        self.end_headers()


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--puerto", type=int, default=7661)
    cli.add_argument("--variante", choices=list(VARIANTES), default="compacto")
    cli.add_argument("--no-abrir", action="store_true")
    args = cli.parse_args()

    print(f"Preparando {VARIANTES[args.variante]['nombre']}...", flush=True)
    preparar_variante(args.variante)
    servidor = ThreadingHTTPServer(("127.0.0.1", args.puerto), Manejador)
    url = f"http://127.0.0.1:{args.puerto}/?programa=tiny-{args.variante}"
    print(f"Clementina en vivo escuchando en {url}", flush=True)
    if not args.no_abrir:
        webbrowser.open(url)
    try:
        servidor.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
