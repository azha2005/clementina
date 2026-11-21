//! Verifica que `web/catalogo.json` sea JSON válido y que cada programa no
//! opcional corra en `WasmMachine` produciendo exactamente la salida declarada.

use mercury_core::tape;
use mercury_wasm::{WasmMachine, STOP_99};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    fn obj(&self) -> &BTreeMap<String, Json> {
        match self {
            Json::Obj(m) => m,
            _ => panic!("se esperaba un objeto JSON, llegó {self:?}"),
        }
    }
    fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(a) => a,
            _ => panic!("se esperaba un arreglo JSON, llegó {self:?}"),
        }
    }
    fn str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            _ => panic!("se esperaba una cadena JSON, llegó {self:?}"),
        }
    }
    fn bool(&self) -> bool {
        match self {
            Json::Bool(b) => *b,
            _ => panic!("se esperaba un booleano JSON, llegó {self:?}"),
        }
    }
    fn u32(&self) -> u32 {
        match self {
            Json::Num(n) => *n as u32,
            _ => panic!("se esperaba un número JSON, llegó {self:?}"),
        }
    }
}

struct JsonParser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> JsonParser<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn next(&mut self) -> u8 {
        assert!(self.i < self.b.len(), "fin inesperado del JSON en byte {}", self.i);
        let c = self.b[self.i];
        self.i += 1;
        c
    }

    fn peek(&mut self) -> u8 {
        self.ws();
        assert!(self.i < self.b.len(), "fin inesperado del JSON");
        self.b[self.i]
    }

    fn parse(&mut self) -> Json {
        match self.peek() {
            b'{' => self.obj(),
            b'[' => self.arr(),
            b'"' => Json::Str(self.string()),
            b't' => {
                assert!(self.b[self.i..].starts_with(b"true"));
                self.i += 4;
                Json::Bool(true)
            }
            b'f' => {
                assert!(self.b[self.i..].starts_with(b"false"));
                self.i += 5;
                Json::Bool(false)
            }
            b'n' => {
                assert!(self.b[self.i..].starts_with(b"null"));
                self.i += 4;
                Json::Null
            }
            _ => self.num(),
        }
    }

    fn string(&mut self) -> String {
        assert_eq!(self.next(), b'"');
        let start = self.i;
        let mut buf = Vec::new();
        while self.i < self.b.len() {
            let c = self.next();
            if c == b'"' {
                return String::from_utf8(buf).expect("UTF-8 válido en cadena JSON");
            }
            if c == b'\\' {
                let esc = self.next();
                match esc {
                    b'"' => buf.push(b'"'),
                    b'\\' => buf.push(b'\\'),
                    b'/' => buf.push(b'/'),
                    b'n' => buf.push(b'\n'),
                    b'r' => buf.push(b'\r'),
                    b't' => buf.push(b'\t'),
                    b'u' => {
                        let hex = std::str::from_utf8(&self.b[self.i..self.i + 4]).unwrap();
                        self.i += 4;
                        let code = u32::from_str_radix(hex, 16).unwrap();
                        let ch = char::from_u32(code).unwrap();
                        let mut tmp = [0u8; 4];
                        buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                    }
                    otro => panic!("escape desconocido \\{} cerca de {}", otro as char, start),
                }
            } else {
                buf.push(c);
            }
        }
        panic!("cadena sin cerrar desde {start}");
    }

    fn num(&mut self) -> Json {
        let start = self.i;
        while self.i < self.b.len()
            && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        {
            self.i += 1;
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).unwrap();
        Json::Num(s.parse::<f64>().unwrap_or_else(|_| panic!("número JSON inválido: {s}")))
    }

    fn arr(&mut self) -> Json {
        assert_eq!(self.next(), b'[');
        let mut out = Vec::new();
        if self.peek() == b']' {
            self.i += 1;
            return Json::Arr(out);
        }
        loop {
            out.push(self.parse());
            match self.peek() {
                b',' => self.i += 1,
                b']' => {
                    self.i += 1;
                    return Json::Arr(out);
                }
                c => panic!("se esperaba ',' o ']', llegó '{}'", c as char),
            }
        }
    }

    fn obj(&mut self) -> Json {
        assert_eq!(self.next(), b'{');
        let mut map = BTreeMap::new();
        if self.peek() == b'}' {
            self.i += 1;
            return Json::Obj(map);
        }
        loop {
            assert_eq!(self.peek(), b'"');
            let k = self.string();
            assert_eq!(self.peek(), b':');
            self.i += 1;
            let v = self.parse();
            map.insert(k, v);
            match self.peek() {
                b',' => self.i += 1,
                b'}' => {
                    self.i += 1;
                    return Json::Obj(map);
                }
                c => panic!("se esperaba ',' o '}}', llegó '{}'", c as char),
            }
        }
    }
}

fn raiz_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn decodificar_salida(codigo: &str, filas: &[u16]) -> String {
    match codigo {
        "ferranti" => tape::decode(filas),
        "ita2" => tape::ita2::decode(filas),
        "longitudinal" => {
            // Reutiliza la misma tabla 5×4 de `nim.rs` para verificar el mensaje en catálogo.
            let fuente = [
                (" ", [0, 0, 0, 0]), ("A", [15, 20, 20, 15]), ("C", [14, 17, 17, 17]),
                ("D", [31, 17, 17, 14]), ("E", [31, 21, 21, 17]), ("F", [31, 20, 20, 16]),
                ("G", [14, 17, 21, 23]), ("H", [31, 4, 4, 31]), ("I", [17, 31, 17, 0]),
                ("J", [2, 1, 17, 30]), ("L", [31, 1, 1, 1]), ("M", [31, 8, 8, 31]),
                ("N", [31, 12, 6, 31]), ("O", [14, 17, 17, 14]), ("P", [31, 20, 20, 8]),
                ("Q", [14, 17, 19, 13]), ("R", [31, 20, 22, 9]), ("S", [9, 21, 21, 18]),
                ("T", [16, 31, 16, 16]), ("U", [30, 1, 1, 30]), ("V", [28, 3, 3, 28]),
                ("Y", [24, 7, 4, 24]), ("Z", [19, 21, 25, 17]), ("!", [0, 29, 0, 0]),
            ];
            let mut s = String::new();
            for celda in filas.chunks_exact(5) {
                let pat = [celda[0], celda[1], celda[2], celda[3]];
                let &(ch, _) = fuente.iter().find(|(_, p)| *p == pat).unwrap();
                s.push_str(ch);
            }
            s
        }
        otro => panic!("código desconocido: {otro}"),
    }
}

#[test]
fn catalogo_valido_y_programas_verificados() {
    let repo = raiz_repo();
    let json_txt = fs::read_to_string(repo.join("web/catalogo.json")).unwrap();
    let mut parser = JsonParser { b: json_txt.as_bytes(), i: 0 };
    let raiz = parser.parse();
    parser.ws();
    assert_eq!(parser.i, json_txt.len(), "bytes sobrantes al final de catalogo.json");

    let programas = raiz.obj().get("programas").unwrap().arr();
    assert!(programas.len() >= 12);

    for prog in programas {
        let p = prog.obj();
        let id = p.get("id").unwrap().str();
        let opcional = p.get("opcional").is_some_and(Json::bool);
        let carga = p.get("carga").unwrap().str();
        let codigo = p.get("codigo_salida").unwrap().str();
        let f = p
            .get("llaves_defecto")
            .unwrap()
            .obj()
            .get("F")
            .unwrap()
            .u32() as u16;
        assert!(!id.contains("nombre_ferranti"));

        if let Some(arch) = p.get("archivo") {
            let ruta = repo.join(arch.str());
            assert!(ruta.is_file(), "{id}: falta {}", ruta.display());
            assert!(
                !arch.str().contains("nombre.mer") && !arch.str().contains("nombre_ferranti"),
                "no incluir cinta con datos personales en el catálogo web"
            );
        }

        if opcional {
            continue;
        }

        let mut wm = WasmMachine::new();
        wm.set_switches(f, 0, 0, 8);
        if id == "mapas_provinciales" {
            let src = fs::read_to_string(repo.join(p.get("archivo").unwrap().str())).unwrap();
            let atlas = fs::read_to_string(repo.join(p.get("archivo_catalogo").unwrap().str())).unwrap();
            let mut parser = JsonParser { b: atlas.as_bytes(), i: 0 };
            let atlas = parser.parse();
            assert_eq!(atlas.obj().keys().map(String::as_str).collect::<Vec<_>>(), ["1958", "1962", "1965"]);
            for (anio, eleccion) in atlas.obj() {
                let provincias = eleccion.obj().get("provincias").unwrap().obj();
                assert_eq!(provincias.len(), 24, "{anio}");
                for (provincia, datos) in provincias {
                    let datos = datos.str();
                    let mapa = datos.strip_suffix('n').expect("marca de fin de mapa");
                    assert!(mapa.contains("VOTO PROVINCIAL"), "{anio} {provincia}");
                    wm.load_written(&src).unwrap();
                    wm.set_tape_text(datos).unwrap();
                    assert_eq!(wm.step_batch(1_000_000, 0), STOP_99, "{anio} {provincia}");
                    assert_eq!(tape::decode(&wm.io.output), mapa, "{anio} {provincia}");
                }
            }
            continue;
        }
        match carga {
            "escrito" => {
                let src = fs::read_to_string(repo.join(p.get("archivo").unwrap().str())).unwrap();
                wm.load_written(&src).unwrap();
            }
            "midi" => {
                let guion = p.get("datos_defecto").unwrap().str();
                wm.load_midi(&[], guion, None, None, None, 15).unwrap();
            }
            "autocode" => {
                let mut src = fs::read_to_string(repo.join(p.get("archivo").unwrap().str())).unwrap();
                if let Some(pref) = p.get("prefijo_autocode") {
                    src = format!("{}{}", pref.str(), src);
                }
                wm.load_autocode(&src).unwrap();
            }
            "input_routine" => {
                let cinta = p.get("cinta_programa").unwrap().str();
                wm.load_input_routine_tape(cinta).unwrap();
                let datos = p.get("datos_defecto").unwrap().str();
                wm.append_tape_text(datos).unwrap();
                wm.initial_transfer();
            }
            otro => panic!("{id}: carga inesperada {otro}"),
        }
        if let Some(arch) = p.get("archivo_datos") {
            let datos = fs::read_to_string(repo.join(arch.str())).unwrap();
            let consulta = p.get("datos_defecto").unwrap().str();
            wm.set_tape_text(&format!("{consulta}{datos}")).unwrap();
        }

        let esp = p.get("esperado").unwrap().obj();
        if esp.get("nim_prueba").is_some_and(Json::bool) {
            assert_eq!(wm.step_batch(100_000, 0), STOP_99, "{id}: primer 99");
            for fila in [1u16, 3, 5, 7] {
                wm.set_switches(fila, 0, 0, 8);
                wm.prepulse();
                assert_eq!(wm.step_batch(100_000, 0), STOP_99, "{id}: carga");
            }
            let texto = decodificar_salida(codigo, &wm.io.output);
            assert_eq!(texto.trim(), "JUEGUE QUE YO LE GANARE", "{id}");
            continue;
        }

        let stop = wm.step_batch(200_000_000, 0);
        assert_eq!(stop, esp.get("parada").unwrap().u32(), "{id}: parada");

        let salida = decodificar_salida(codigo, &wm.io.output);
        let header = String::from_utf8_lossy(&wm.autocode_header).into_owned();
        if let Some(v) = esp.get("header_contiene") {
            assert!(header.contains(v.str()), "{id}: header {header:?}");
        }
        if let Some(v) = esp.get("salida_exacta") {
            assert_eq!(salida, v.str(), "{id}");
        }
        if let Some(v) = esp.get("salida_contiene") {
            assert!(salida.contains(v.str()), "{id}: salida {salida:?}");
        }
        if let Some(v) = esp.get("salida_archivo") {
            let ref_txt = fs::read_to_string(repo.join(v.str())).unwrap().replace("\r\n", "\n");
            let norm = |s: &str| {
                s.lines()
                    .map(str::trim_end)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim_matches('\n')
                    .to_string()
            };
            assert_eq!(norm(&salida), norm(&ref_txt), "{id}: salida vs {}", v.str());
        }
        if let Some(v) = esp.get("hoots_min") {
            assert!(wm.io.hoots.len() >= v.u32() as usize, "{id}: hoots {}", wm.io.hoots.len());
        }
    }
}
