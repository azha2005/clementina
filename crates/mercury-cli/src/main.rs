//! `clementina`: consola de texto del emulador de la Ferranti Mercury.

use mercury_core::midi;
mod pagina;
mod sesion;

use mercury_core::acc::Float;
use mercury_core::asm::{assemble, assemble_sector, Program};
use mercury_core::io::Io;
use mercury_core::opcodes;
use mercury_core::{tape, Config, Machine, Stop};
use std::fs;
use std::io::Write;
use std::process::ExitCode;

const USO: &str = "\
clementina — emulador de la Ferranti Mercury (Clementina, UBA 1961)

Uso:
  clementina correr    PROGRAMA.mer [opciones]
  clementina listar    PROGRAMA.mer
  clementina autocode  PROGRAMA.mer [opciones] [--listado]
  clementina tele-input CINTA.bin [opciones] [--cinta DATOS.txt]
  clementina midi      CANCION.mid [--guion TEXTO.txt] [opciones de midi]

'autocode' compila un programa en Mercury Autocode y lo corre con las
mismas opciones que 'correr'. Si no se da --cinta y el programa termina en
una línea '->' (o '→'), lo que sigue se usa como cinta de datos.
--listado muestra el código Mercury generado en lugar de correrlo.
'tele-input' arranca con la llave 2 y lee una cinta binaria de código
Ferranti (un byte de 0 a 31 por fila). --cinta agrega datos a continuación.

Opciones de 'correr':
  --cinta ARCHIVO     texto a perforar en la cinta de entrada (canal 1)
  --llaves N          valor de las llaves de mano F (instrucción 61)
  --max N             límite de instrucciones (50 000 000, o 500 000 000 en tele-input)
  --traza             muestra cada instrucción ejecutada
  --interactivo       al detenerse abre el pupitre para dar PREPULSE y mover llaves
  --parada-opcional D activa HANDSWITCH STOP en la dirección D (entero o p.l)
  --single            arranca con SINGLE/CONTINUOUS en SINGLE
  --sin-escritura     arranca con WRITE CURRENT en OFF
  --wav ARCHIVO       guarda el sonido del parlante (instrucción 58) en WAV
  --html ARCHIVO      página que toca el WAV e imprime lo perforado en sincronía
  --clementina-1966   cuatro tambores y órdenes 65/66 del Instituto de Cálculo
  --sectores N        sectores de tambor (512 por omisión, 1024 con cuatro tambores)
  --tambor ARCHIVO    imagen de tambor para 'correr', cortos u16 little-endian
  --guardar-tambor ARCHIVO  guarda la imagen tras una carga 'tele-input' exitosa
  --codigo C          cómo mostrar lo perforado: ferranti (por omisión) o ita2
  --svg ARCHIVO       dibuja la cinta perforada en SVG, a escala real
  --guia N            filas de guía en cada punta del dibujo (por omisión 8)
  --solapar F:P       en el dibujo, la fila F queda a P pasos de la anterior
                      (defecto de avance de la perforadora; se puede repetir)

'midi' convierte la melodía de un MIDI en un programa para Clementina
(CANCION.mer y la partitura en el tambor, CANCION.tambor). Con --guion, el
texto del guion se perfora mientras suena la música; una línea @SEGUNDOS
(o @MM:SS) marca desde cuándo sale lo que sigue. Opciones de 'midi':
  --pista N           sólo las notas de la pista N (ver la lista que se muestra)
  --canal N           sólo el canal N (1 a 16; sin él, todos menos el 10)
  --transponer N      semitonos (por omisión, la octava que menos desafina)
  --separacion MS     silencio entre notas ligadas (15 ms)
  --salida BASE       nombre de los archivos generados (el del MIDI)

La salida perforada se muestra decodificada con el código de cinta del
Apéndice VI; el resumen de la ejecución va a stderr.";

pub(crate) struct ConsoleIo {
    pub(crate) input: Vec<u16>,
    pub(crate) pos: usize,
    pub(crate) punched: Vec<u16>,
    pub(crate) switches: u16,
    pub(crate) function_keys: u16,
    pub(crate) address_keys: u16,
    pub(crate) handswitch_stop: bool,
    pub(crate) single_mode: bool,
    pub(crate) manual_mode: bool,
    pub(crate) write_current: bool,
    pub(crate) inhibit_parities: bool,
    pub(crate) hoots: Vec<u64>,
}

impl Default for ConsoleIo {
    fn default() -> Self {
        Self {
            input: Vec::new(),
            pos: 0,
            punched: Vec::new(),
            switches: 0,
            function_keys: 0,
            address_keys: 0,
            handswitch_stop: false,
            single_mode: false,
            manual_mode: false,
            write_current: true,
            inhibit_parities: false,
            hoots: Vec::new(),
        }
    }
}

impl Io for ConsoleIo {
    fn read(&mut self, channel: u8, _device: u16) -> Option<u16> {
        if channel != 1 {
            return None;
        }
        let v = *self.input.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }
    fn write(&mut self, channel: u8, _device: u16, value: u16) {
        if channel == 1 {
            self.punched.push(value);
        }
    }
    fn handswitches(&mut self) -> u16 {
        self.switches
    }
    fn function_keys(&mut self) -> u16 {
        self.function_keys & 0x3ff
    }
    fn address_keys(&mut self) -> u16 {
        self.address_keys & 0x3ff
    }
    fn handswitch_stop(&mut self) -> bool {
        self.handswitch_stop
    }
    fn single_mode(&mut self) -> bool {
        self.single_mode
    }
    fn manual_mode(&mut self) -> bool {
        self.manual_mode
    }
    fn write_current(&mut self) -> bool {
        self.write_current
    }

    fn inhibit_parities(&mut self) -> bool {
        self.inhibit_parities
    }
    fn hoot(&mut self, t_us: u64) {
        self.hoots.push(t_us);
    }
    fn display(&mut self, long: [u16; 4]) {
        eprintln!("[monitor] {}", Float::from_long(long).to_f64());
    }
}

pub(crate) fn disasm(word: u32) -> String {
    let f = ((word >> 13) & 0x7f) as u8;
    let b = (word >> 10) & 7;
    let n = word & 0x3ff;
    match opcodes::decode(f) {
        Some(d) => format!("{:02}{}{} {:4}", d.dec, b, if d.second_quarter { "*" } else { " " }, n),
        None => format!("?{f:03o}{b}  {n:4}"),
    }
}

fn leer_programa(path: &str) -> Result<Program, String> {
    let src = fs::read_to_string(path).map_err(|e| format!("no se pudo leer {path}: {e}"))?;
    assemble(&src).map_err(|e| format!("{path}: {e}"))
}

fn cargar_tambor(m: &mut Machine, path: &str) -> Result<(), String> {
    let datos = fs::read(path).map_err(|e| format!("no se pudo leer {path}: {e}"))?;
    m.drum.cargar_imagen(&datos).map_err(|e| format!("{path}: {e}"))
}

fn guardar_imagen_tambor(m: &Machine, destino: &str) -> Result<(), String> {
    let datos: Vec<u8> = (0..m.drum.len())
        .flat_map(|sector| m.drum.sector(sector))
        .flat_map(|corto| corto.to_le_bytes())
        .collect();
    fs::write(destino, datos).map_err(|e| format!("no se pudo escribir {destino}: {e}"))
}

/// El aviso 30 del sector 1 deja la máquina en el bucle de la bocina.
/// Reconocer la rutina evita confundirla con un HOOT del programa cargado.
fn sono_fin_de_carga(m: &Machine, c_anterior: u16) -> bool {
    c_anterior == 27 && m.c == 28 && m.b[5] == 30
        && m.store.medium(27) == opcodes::make_instruction(0o060, 0, 0)  // 580 0
        && m.store.medium(29) == opcodes::make_instruction(0o037, 0, 29) // 380 29
        && m.store.medium(30) == opcodes::make_instruction(0o001, 0, 27) // 590 27
}

fn listar(path: &str) -> Result<(), String> {
    let prog = leer_programa(path)?;
    let mut st = mercury_core::store::Store::default();
    prog.load(&mut st);
    let mut mediums: Vec<usize> = prog.shorts.iter().map(|(a, _)| a / 2).collect();
    mediums.sort();
    mediums.dedup();
    println!("arranque: {}", prog.start);
    for m in mediums {
        let w = st.medium(m);
        println!("{:2}.{:02} ({m:4})  {:07o}  {}", m / 64, m % 64, w, disasm(w));
    }
    Ok(())
}

fn escribir_wav(path: &str, hoots: &[u64], fin_us: u64) -> Result<(), String> {
    const RATE: u64 = 44_100;
    let len = ((fin_us + 50_000) * RATE / 1_000_000) as usize;
    let mut s = vec![0i32; len];
    // Cada "hoot" es un impulso al diafragma: un pulso corto que decae.
    for &t in hoots {
        let i0 = (t * RATE / 1_000_000) as usize;
        for k in 0..40 {
            if let Some(x) = s.get_mut(i0 + k) {
                *x += (12_000.0 * (-(k as f64) / 8.0).exp()) as i32;
            }
        }
    }
    let mut data = Vec::with_capacity(len * 2);
    for v in s {
        data.extend_from_slice(&(v.clamp(-32768, 32767) as i16).to_le_bytes());
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&(RATE as u32).to_le_bytes());
    out.extend_from_slice(&(RATE as u32 * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    fs::write(path, out).map_err(|e| format!("no se pudo escribir {path}: {e}"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Modo {
    Escrito,
    Autocode,
    TeleInput,
}

fn correr(args: &[String], modo: Modo) -> Result<(), String> {
    let autocode = modo == Modo::Autocode;
    let tele_input = modo == Modo::TeleInput;
    let mut path = None;
    let mut listado = false;
    let mut cinta = None;
    let mut tambor = None;
    let mut destino_tambor = None;
    let mut llaves = if tele_input { 4 } else { 0 };
    let mut max = if tele_input { 500_000_000u64 } else { 50_000_000 };
    let mut traza = false;
    let mut interactivo = false;
    let mut parada_opcional = None;
    let mut single = false;
    let mut write_current = true;
    let mut wav = None;
    let mut config = Config::default();
    let mut sectores = None;
    let mut ita2 = false;
    let mut svg = None;
    let mut html = None;
    let mut svg_opt = tape::SvgOptions { leader: 8, overlaps: Vec::new() };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut valor = || it.next().cloned().ok_or(format!("falta el valor de {a}"));
        match a.as_str() {
            "--cinta" => cinta = Some(valor()?),
            "--tambor" if modo == Modo::Escrito => tambor = Some(valor()?),
            "--guardar-tambor" if tele_input => destino_tambor = Some(valor()?),
            "--llaves" => llaves = valor()?.parse().map_err(|_| "llaves inválidas")?,
            "--max" => max = valor()?.parse().map_err(|_| "--max inválido")?,
            "--traza" => traza = true,
            "--interactivo" => interactivo = true,
            "--parada-opcional" => parada_opcional = Some(sesion::parsear_dir(&valor()?)?),
            "--single" => single = true,
            "--sin-escritura" => write_current = false,
            "--wav" => wav = Some(valor()?),
            "--html" => html = Some(valor()?),
            "--codigo" => match valor()?.as_str() {
                "ita2" => ita2 = true,
                "ferranti" => ita2 = false,
                c => return Err(format!("código desconocido: {c} (ferranti o ita2)")),
            },
            "--svg" => svg = Some(valor()?),
            "--guia" => svg_opt.leader = valor()?.parse().map_err(|_| "--guia inválido")?,
            "--solapar" => {
                let v = valor()?;
                let (f, p) = v.split_once(':').ok_or("--solapar espera FILA:PASOS, p. ej. 29:0.35")?;
                let f: usize = f.parse().map_err(|_| "fila inválida en --solapar")?;
                let p: f64 = p.parse().map_err(|_| "pasos inválidos en --solapar")?;
                svg_opt.overlaps.push((f, p));
            }
            "--sectores" => sectores = Some(valor()?.parse().map_err(|_| "--sectores inválido")?),
            "--clementina-1966" => config = Config::clementina_1966(),
            "--listado" if autocode => listado = true,
            s if s.starts_with("--") => return Err(format!("opción desconocida: {s}")),
            s => path = Some(s.to_string()),
        }
    }
    let path = path.ok_or("falta el programa")?;
    if let Some(sectores) = sectores {
        config.drum_sectors = sectores;
    }
    let mut m = Machine::new(config);
    if let Some(path) = tambor {
        cargar_tambor(&mut m, &path)?;
    }
    let mut datos_del_programa = None;
    if tele_input {
        if m.drum.len() < 2 {
            return Err("tele-input necesita al menos dos sectores de tambor".into());
        }
        for (n, texto) in [
            include_str!("../../../input_routine/sector00.txt"),
            include_str!("../../../input_routine/sector01.txt"),
        ].into_iter().enumerate() {
            *m.drum.sector_mut(n) = assemble_sector(texto).map_err(|e| e.to_string())?;
        }
        m.initial_transfer();
    } else if autocode {
        let src = fs::read_to_string(&path).map_err(|e| format!("no se pudo leer {path}: {e}"))?;
        let img = mercury_autocode::compilar(&src).map_err(|e| format!("{path}: {e}"))?;
        if listado {
            print!("{}", img.listado);
            return Ok(());
        }
        if m.drum.len() < 480 {
            return Err("el Autocode necesita al menos 480 sectores de tambor".into());
        }
        // Lo que el traductor imprimía al leer el programa (title, psa).
        print!("{}", img.salida_traduccion);
        img.cargar(&mut m);
        datos_del_programa = img.datos.clone();
    } else {
        let prog = leer_programa(&path)?;
        prog.load(&mut m.store);
        m.c = prog.start;
    }

    let texto_cinta = match cinta {
        Some(c) => Some((fs::read_to_string(&c).map_err(|e| format!("no se pudo leer {c}: {e}"))?, c)),
        None => datos_del_programa.map(|d| (d, "datos del programa".to_string())),
    };
    let mut input = if tele_input {
        let filas = fs::read(&path).map_err(|e| format!("no se pudo leer {path}: {e}"))?;
        if filas.is_empty() || filas.iter().any(|&x| x > 31) {
            return Err(format!("{path}: se esperaba una cinta binaria con filas de 5 bits"));
        }
        filas.into_iter().map(u16::from).collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    input.extend(match texto_cinta {
        Some((txt, c)) => {
            // Limpiar los datos de Edimburgo sólo al correr Autocode.
            // La cinta de un programa en forma escrita conserva los espacios del mapa.
            let txt: String = if modo == Modo::Autocode {
                txt.lines().map(|l| format!("{}\n", l.split("//").next().unwrap_or("").trim())).collect()
            } else {
                txt.lines().map(|l| format!("{l}\n")).collect()
            };
            tape::encode(&txt).map_err(|ch| format!("{c}: el carácter '{ch}' no existe en el código de cinta"))?
        }
        None => Vec::new(),
    });
    let mut io = ConsoleIo {
        input,
        pos: 0,
        punched: Vec::new(),
        switches: llaves,
        function_keys: 0,
        address_keys: parada_opcional.unwrap_or(0),
        handswitch_stop: parada_opcional.is_some(),
        single_mode: single,
        manual_mode: false,
        write_current,
        inhibit_parities: false,
        hoots: Vec::new(),
    };

    let mut stop = None;
    let mut fin_sonoro = false;
    let mut printed = 0;
    let mut tiempos: Vec<(u64, u16)> = Vec::new();
    let stdout = std::io::stdout();
    let mut al_perforar = |io: &ConsoleIo, time_us: u64| {
        if io.punched.len() > printed {
            tiempos.extend(io.punched[printed..].iter().map(|&c| (time_us, c)));
            let decode = |c: &[u16]| if ita2 { tape::ita2::decode(c) } else { tape::decode(c) };
            let mut out = stdout.lock();
            let _ = write!(
                out,
                "{}",
                decode(&io.punched[..]).chars().skip(decode(&io.punched[..printed]).chars().count()).collect::<String>()
            );
            let _ = out.flush();
            printed = io.punched.len();
        }
    };
    for _ in 0..max {
        if traza {
            let w = m.store.medium(m.c as usize);
            eprintln!(
                "{:2}.{:02}  {}   A={:<14.8e} S={:4} Bt={:4} t={}µs",
                m.c / 64, m.c % 64, disasm(w), m.acc.to_f64(), m.sac(), m.bt, m.time_us
            );
        }
        let c_anterior = m.c;
        let r = m.step(&mut io);
        al_perforar(&io, m.time_us);
        if tele_input && !interactivo && r.is_ok() && sono_fin_de_carga(&m, c_anterior) {
            fin_sonoro = true;
            break;
        }
        if let Err(s) = r {
            if interactivo {
                let stdin = std::io::stdin();
                let stderr = std::io::stderr();
                let mut entrada = stdin.lock();
                let mut salida = stderr.lock();
                match sesion::consultar_operador(&mut m, &mut io, s, &mut entrada, &mut salida, &mut al_perforar)? {
                    sesion::Accion::Seguir => continue,
                    sesion::Accion::Salir => {
                        stop = m.stop().or(Some(s));
                        break;
                    }
                }
            }
            stop = Some(s);
            break;
        }
    }

    let motivo = if fin_sonoro { "fin de carga (bocina de tele-input)".into() } else { sesion::motivo_parada(stop, max) };
    eprintln!();
    eprintln!("── {motivo} en C = {}.{:02}", m.c / 64, m.c % 64);
    eprintln!(
        "   {} instrucciones, {:.3} s de máquina; A = {}; B1–B7 = {:?}",
        m.executed,
        m.time_us as f64 / 1e6,
        m.acc.to_f64(),
        &m.b[1..]
    );
    if let Some(f) = svg {
        fs::write(&f, tape::to_svg_with(&io.punched, &svg_opt)).map_err(|e| format!("no se pudo escribir {f}: {e}"))?;
        eprintln!("   cinta: {} filas → {f}", io.punched.len());
    }
    if let Some(w) = &wav {
        escribir_wav(&w, &io.hoots, m.time_us)?;
        eprintln!("   parlante: {} impulsos → {w}", io.hoots.len());
    }
    if let Some(h) = html {
        // La página busca el WAV a su lado si están en la misma carpeta.
        let wav_rel = wav.as_ref().map(|w| {
            let (pw, ph) = (std::path::Path::new(w), std::path::Path::new(&h));
            match (pw.parent() == ph.parent(), pw.file_name()) {
                (true, Some(f)) => f.to_string_lossy().into_owned(),
                _ => w.replace('\\', "/"),
            }
        });
        let titulo = std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        fs::write(&h, pagina::generar(&titulo, wav_rel.as_deref(), &tiempos, m.time_us))
            .map_err(|e| format!("no se pudo escribir {h}: {e}"))?;
        eprintln!("   página: {} caracteres con su instante → {h}", tiempos.len());
    }
    if tele_input {
        if stop == Some(Stop::Stop99) && m.c == 17 {
            return Err("el tele-input rechazó la suma de control".into());
        }
        if !fin_sonoro && stop != Some(Stop::Stop99) {
            return Err("la ejecución desde cinta terminó con error".into());
        }
        if !fin_sonoro {
            let estado = m.store.medium((m.c.wrapping_sub(1) & 1023) as usize) & 1023;
            if estado != 0 {
                return Err(format!("la ejecución desde cinta terminó con 990 {estado}"));
            }
        }
        if let Some(destino) = destino_tambor {
            guardar_imagen_tambor(&m, &destino)?;
            eprintln!("   imagen de {} sectores → {destino}", m.drum.len());
        }
    }
    Ok(())
}

fn convertir_midi(args: &[String]) -> Result<(), String> {
    let mut path = None;
    let mut guion = None;
    let mut pista = None;
    let mut canal = None;
    let mut transponer = None;
    let mut separacion = 15u64;
    let mut salida = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut valor = || it.next().cloned().ok_or(format!("falta el valor de {a}"));
        match a.as_str() {
            "--guion" => guion = Some(valor()?),
            "--pista" => pista = Some(valor()?.parse::<usize>().map_err(|_| "--pista inválida")?),
            "--canal" => {
                let c: u8 = valor()?.parse().map_err(|_| "--canal inválido")?;
                if !(1..=16).contains(&c) {
                    return Err("--canal va de 1 a 16".into());
                }
                canal = Some(c - 1);
            }
            "--transponer" => transponer = Some(valor()?.parse::<i32>().map_err(|_| "--transponer espera semitonos")?),
            "--separacion" => separacion = valor()?.parse().map_err(|_| "--separacion inválida")?,
            "--salida" => salida = Some(valor()?),
            s if s.starts_with("--") => return Err(format!("opción desconocida: {s}")),
            s => path = Some(s.to_string()),
        }
    }
    let path = path.ok_or("falta el archivo MIDI")?;
    let datos = fs::read(&path).map_err(|e| format!("no se pudo leer {path}: {e}"))?;
    let smf = midi::leer_smf(&datos).map_err(|e| format!("{path}: {e}"))?;
    eprintln!("pistas de {path}:");
    for (i, p) in smf.pistas.iter().enumerate() {
        if p.notas > 0 {
            let canales: Vec<String> = p.canales.iter().map(|c| (c + 1).to_string()).collect();
            eprintln!("  {i:2}  {:4} notas, canal {}, de {} a {}  {}", p.notas, canales.join(","), p.grave, p.aguda, p.nombre);
        }
    }
    let tramos = midi::melodia(&smf.notas, pista, canal, separacion * 1000);
    if tramos.is_empty() {
        return Err("no quedó ninguna nota (revisar --pista y --canal)".into());
    }
    let bloques = match &guion {
        Some(g) => midi::leer_guion(&fs::read_to_string(g).map_err(|e| format!("no se pudo leer {g}: {e}"))?)?,
        None => Vec::new(),
    };
    let mut cambiados = 0;
    let mut sh = tape::Shift::Figures;
    for b in &bloques {
        cambiados += midi::codificar(&b.texto, &mut sh).1;
    }
    let transponer = transponer.unwrap_or_else(|| midi::transposicion_automatica(&tramos));
    let p = midi::armar(&tramos, &bloques, &midi::Opciones { transponer }, Config::default().drum_sectors)?;

    let base = salida.unwrap_or_else(|| std::path::Path::new(&path).with_extension("").to_string_lossy().into_owned());
    let (mer, tambor) = (format!("{base}.mer"), format!("{base}.tambor"));
    // Sin paréntesis: cerrarían el comentario de la cabecera.
    let nombre = |s: &str| {
        std::path::Path::new(s).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default().replace(['(', ')'], "")
    };
    let cabecera = format!(
        "(REPRODUCTOR: {} hecho con clementina midi a partir de {}.\n La melodia y el texto estan en el tambor, en {}:\n clementina correr {} --tambor {} --wav {}.wav --html {}.html\n El formato de los registros esta en crates/mercury-core/src/midi.rs.)\n\n",
        nombre(&mer), nombre(&path), nombre(&tambor), nombre(&mer), nombre(&tambor), nombre(&base), nombre(&base)
    );
    fs::write(&mer, cabecera + midi::REPRODUCTOR).map_err(|e| format!("no se pudo escribir {mer}: {e}"))?;
    fs::write(&tambor, midi::imagen_tambor(&p, Config::default().drum_sectors)?)
        .map_err(|e| format!("no se pudo escribir {tambor}: {e}"))?;

    let notas = p.tocados.iter().filter(|t| t.suena).count();
    eprintln!();
    eprintln!("melodía: {} notas en {notas} registros, transpuesta {transponer:+} semitonos", tramos.len());
    eprintln!(
        "  a lo sumo {:.0} cents de desafinación; ataques a menos de {:.1} ms del MIDI",
        p.peor_cents,
        p.peor_ataque_us as f64 / 1000.0
    );
    if p.sin_altura > 0 {
        eprintln!("  {} notas demasiado agudas para el parlante quedaron afuera (probar --transponer)", p.sin_altura);
    }
    if !bloques.is_empty() {
        eprintln!(
            "guion: {} caracteres de cinta (con cambios de registro y CR LF); el más atrasado sale {:.1} s después de su marca",
            p.perforaciones,
            p.peor_retraso_texto_us as f64 / 1e6
        );
        if cambiados > 0 {
            eprintln!("  {cambiados} signos que no existen en el código Ferranti se reemplazaron o se omitieron");
        }
    }
    eprintln!("duración: {:.1} s de máquina, {} sectores de tambor", p.fin_us as f64 / 1e6, p.sectores.len());
    eprintln!("→ {mer}\n→ {tambor}");
    eprintln!("\npara escucharlo:\n  clementina correr {mer} --tambor {tambor} --wav {base}.wav --html {base}.html");
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(|s| s.as_str()) {
        Some("correr") | Some("run") => correr(&args[1..], Modo::Escrito),
        Some("autocode") => correr(&args[1..], Modo::Autocode),
        Some("tele-input") => correr(&args[1..], Modo::TeleInput),
        Some("midi") => convertir_midi(&args[1..]),
        Some("listar") | Some("list") => match args.get(1) {
            Some(p) => listar(p),
            None => Err("falta el programa".into()),
        },
        _ => {
            println!("{USO}");
            return ExitCode::SUCCESS;
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
