//! Conversor de MIDI a un programa de Clementina: la melodía suena por el
//! parlante (instrucción 58) y, mientras tanto, la perforadora saca un texto
//! sincronizado con la música (un guion con tiempos).
//!
//! El resultado son dos archivos: un programa en forma escrita (`.mer`), el
//! reproductor, y una imagen de tambor con la partitura. El reproductor copia
//! un sector por vez a la página 2 (instrucción 68) y lo recorre registro por
//! registro. Cada registro es una nota o un silencio, con los caracteres que
//! hay que perforar mientras dura:
//!
//! ```text
//! [0] tipo: 0 nota o silencio, 1 pasar al sector siguiente, 2 fin
//! [1] mitad izquierda de la instrucción del parlante: 580 (suena) o 570 (calla)
//! [2] mitad izquierda del ajuste fino: 590 (sin él) o 570 (60 µs más)
//! [3] k, vueltas de espera por período
//! [4] k − 4, las vueltas cuando el período perfora
//! [5] N, cantidad de períodos
//! [6] q, cada cuántos períodos se perfora (1023: nunca)
//! [7…] los caracteres a perforar, en código Ferranti
//! ```
//!
//! El período de una nota es 420 + 120 k (+ 60) µs, contando los tiempos de
//! sección 3.11 del CS 225 A que usa el emulador: 60 µs por instrucción, 120
//! la perforación. Los períodos que perforan tardan lo mismo que los que no
//! (se esperan cuatro vueltas menos), así que la nota no se desafina.
//!
//! Supuesto: la perforadora tarda 30 ms por carácter (`Config::punch_char_us`).
//! Si se perforara antes de que esté lista, la máquina quedaría esperando y la
//! nota se cortaría; el conversor deja siempre al menos 30 ms entre dos
//! perforaciones.
//!
//! Supuesto: el tiempo de una transferencia de tambor sale del modelo de
//! `cpu.rs` (espera a que pase el sector más 7,68 ms). El conversor calcula los
//! tiempos de la máquina exactamente con ese mismo modelo, de modo que el
//! retraso de cada carga de sector se descuenta del silencio siguiente.

use crate::asm::assemble;
use crate::cpu::{DRUM_GAP_US, DRUM_REV_US, DRUM_SECTOR_US};
use crate::store::SHORTS_PER_PAGE;
use crate::tape::{self, Shift};

// ───────────────────────────── lectura del MIDI ─────────────────────────────

/// Una nota del archivo MIDI, con los tiempos ya en microsegundos.
#[derive(Debug, Clone, PartialEq)]
pub struct Nota {
    pub inicio: u64,
    pub fin: u64,
    pub altura: u8,
    pub canal: u8,
    pub pista: usize,
}

#[derive(Debug, Clone, Default)]
pub struct InfoPista {
    pub nombre: String,
    pub notas: usize,
    pub canales: Vec<u8>,
    pub grave: u8,
    pub aguda: u8,
}

#[derive(Debug, Clone, Default)]
pub struct Smf {
    pub notas: Vec<Nota>,
    pub pistas: Vec<InfoPista>,
}

struct Lector<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Lector<'a> {
    fn byte(&mut self) -> Result<u8, String> {
        let b = *self.d.get(self.p).ok_or("el archivo MIDI termina antes de tiempo")?;
        self.p += 1;
        Ok(b)
    }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.d.get(self.p..).and_then(|resto| resto.get(..n))
            .ok_or("el archivo MIDI termina antes de tiempo")?;
        self.p += s.len();
        Ok(s)
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// Cantidad de longitud variable (hasta 4 bytes de 7 bits).
    fn vlq(&mut self) -> Result<u32, String> {
        let mut v = 0u32;
        for _ in 0..4 {
            let b = self.byte()?;
            v = (v << 7) | (b & 0x7f) as u32;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("cantidad de longitud variable demasiado larga".into())
    }
}

/// Lee un archivo MIDI estándar (formatos 0 y 1).
pub fn leer_smf(datos: &[u8]) -> Result<Smf, String> {
    let mut r = Lector { d: datos, p: 0 };
    if r.bytes(4)? != b"MThd" {
        return Err("no es un archivo MIDI (falta MThd)".into());
    }
    let largo = r.u32()? as usize;
    if largo < 6 {
        return Err("encabezado MIDI incompleto".into());
    }
    let formato = r.u16()?;
    if formato > 1 {
        return Err("sólo se aceptan MIDI en formatos 0 y 1".into());
    }
    let n_pistas = r.u16()? as usize;
    let division = r.u16()?;
    if division & 0x8000 != 0 {
        return Err("división SMPTE: sólo se aceptan archivos con pulsos por negra".into());
    }
    let ppq = u64::from(division);
    if ppq == 0 {
        return Err("el MIDI tiene cero pulsos por negra".into());
    }
    r.bytes(largo - 6)?;

    // (tick, µs por negra) de todas las pistas.
    let mut tempos: Vec<(u64, u64)> = Vec::new();
    // (tick inicio, tick fin, altura, canal, pista)
    let mut crudas: Vec<(u64, u64, u8, u8, usize)> = Vec::new();
    let mut pistas = Vec::new();
    for pista in 0..n_pistas {
        let id = r.bytes(4)?;
        let largo = r.u32()? as usize;
        let datos_pista = r.bytes(largo)?;
        if id != b"MTrk" {
            return Err(format!("pista {pista}: falta MTrk"));
        }
        let mut r = Lector { d: datos_pista, p: 0 };
        let mut info = InfoPista::default();
        let mut tick = 0u64;
        let mut estado = 0u8;
        let mut abiertas = std::collections::BTreeMap::<(u8, u8), std::collections::VecDeque<u64>>::new();
        while r.p < datos_pista.len() {
            tick += r.vlq()? as u64;
            let mut b = r.byte()?;
            if b == 0xff {
                let tipo = r.byte()?;
                let n = r.vlq()? as usize;
                let d = r.bytes(n)?;
                match tipo {
                    0x51 if n == 3 => tempos.push((tick, ((d[0] as u64) << 16) | ((d[1] as u64) << 8) | d[2] as u64)),
                    0x03 if info.nombre.is_empty() => info.nombre = String::from_utf8_lossy(d).trim().to_string(),
                    0x2f => break,
                    _ => {}
                }
                continue;
            }
            if b == 0xf0 || b == 0xf7 {
                let n = r.vlq()? as usize;
                r.bytes(n)?;
                continue;
            }
            let primero = if b < 0x80 {
                // Estado corrido: el byte ya es el primer dato.
                if estado == 0 {
                    return Err(format!("pista {pista}: dato sin byte de estado"));
                }
                let d = b;
                b = estado;
                d
            } else {
                estado = b;
                r.byte()?
            };
            let canal = b & 0x0f;
            match b & 0xf0 {
                0x80 | 0x90 => {
                    let vel = r.byte()?;
                    let altura = primero & 0x7f;
                    if b & 0xf0 == 0x90 && vel > 0 {
                        abiertas.entry((altura, canal)).or_default().push_back(tick);
                    } else if let Some(t0) = abiertas.get_mut(&(altura, canal)).and_then(|cola| cola.pop_front()) {
                        crudas.push((t0, tick, altura, canal, pista));
                    }
                }
                0xa0 | 0xb0 | 0xe0 => {
                    r.byte()?;
                }
                0xc0 | 0xd0 => {}
                _ => return Err(format!("pista {pista}: evento desconocido {b:#04x}")),
            }
        }
        for ((altura, canal), inicios) in abiertas {
            crudas.extend(inicios.into_iter().map(|t0| (t0, tick, altura, canal, pista)));
        }
        pistas.push(info);
    }

    // De pulsos a microsegundos con el mapa de tempos.
    tempos.sort();
    let (mut us, mut anterior, mut tempo) = (0u128, 0, 500_000);
    let mapa: Vec<_> = tempos.into_iter().map(|(tick, nuevo)| {
        us += u128::from(tick - anterior) * u128::from(tempo) / u128::from(ppq);
        anterior = tick;
        tempo = nuevo;
        (tick, us, nuevo)
    }).collect();
    let a_us = |t: u64| -> u64 {
        let i = mapa.partition_point(|&(tick, _, _)| tick < t);
        let (tick, us, tempo) = if i == 0 { (0, 0, 500_000) } else { mapa[i - 1] };
        let total = us + u128::from(t - tick) * u128::from(tempo) / u128::from(ppq);
        u64::try_from(total).unwrap_or(u64::MAX)
    };
    let mut notas: Vec<Nota> = crudas
        .into_iter()
        .filter(|n| n.1 > n.0)
        .map(|(t0, t1, altura, canal, pista)| Nota { inicio: a_us(t0), fin: a_us(t1), altura, canal, pista })
        .collect();
    notas.sort_by_key(|n| (n.inicio, n.altura));
    for n in &notas {
        if let Some(p) = pistas.get_mut(n.pista) {
            if p.notas == 0 {
                p.grave = n.altura;
                p.aguda = n.altura;
            }
            p.notas += 1;
            p.grave = p.grave.min(n.altura);
            p.aguda = p.aguda.max(n.altura);
            if !p.canales.contains(&n.canal) {
                p.canales.push(n.canal);
            }
        }
    }
    Ok(Smf { notas, pistas })
}

// ──────────────────────────────── la melodía ────────────────────────────────

/// Un tramo de la melodía: una sola altura entre dos instantes (µs).
#[derive(Debug, Clone, PartialEq)]
pub struct Tramo {
    pub inicio: u64,
    pub fin: u64,
    pub altura: u8,
}

/// El parlante da una sola nota por vez: en cada instante suena la más aguda
/// de las que están activas. Sin filtros se excluye el canal 10 (percusión).
pub fn melodia(notas: &[Nota], pista: Option<usize>, canal: Option<u8>, separacion_us: u64) -> Vec<Tramo> {
    let elegidas: Vec<&Nota> = notas
        .iter()
        .filter(|n| pista.map_or(true, |p| n.pista == p))
        .filter(|n| match canal {
            Some(c) => n.canal == c,
            None => n.canal != 9,
        })
        .collect();
    let mut por_inicio: Vec<_> = elegidas.iter().enumerate().collect();
    por_inicio.sort_by_key(|(_, n)| n.inicio);
    let mut puntos: Vec<u64> = elegidas.iter().flat_map(|n| [n.inicio, n.fin]).collect();
    puntos.sort();
    puntos.dedup();
    let mut tramos: Vec<(Tramo, usize)> = Vec::new();
    let mut activas = std::collections::BinaryHeap::new();
    let mut siguientes = por_inicio.into_iter().peekable();
    for w in puntos.windows(2) {
        let (a, b) = (w[0], w[1]);
        while let Some((i, n)) = siguientes.next_if(|(_, n)| n.inicio <= a) {
            activas.push((n.altura, n.inicio, i, n.fin));
        }
        while activas.peek().is_some_and(|&(_, _, _, fin)| fin <= a) {
            activas.pop();
        }
        let Some(&(altura, _, i, _)) = activas.peek() else { continue };
        match tramos.last_mut() {
            Some((t, j)) if *j == i && t.fin == a => t.fin = b,
            _ => tramos.push((Tramo { inicio: a, fin: b, altura }, i)),
        }
    }
    let mut tramos: Vec<Tramo> = tramos.into_iter().map(|(t, _)| t).collect();
    // Separar las notas ligadas para que se oiga cada ataque.
    for i in 0..tramos.len().saturating_sub(1) {
        let sig = tramos[i + 1].inicio;
        if sig < tramos[i].fin.saturating_add(separacion_us) {
            let dur = sig.saturating_sub(tramos[i].inicio);
            tramos[i].fin = tramos[i].inicio + dur.saturating_sub(separacion_us.min(dur / 4));
        }
    }
    tramos.retain(|t| t.fin > t.inicio);
    tramos
}

// ──────────────────────────── alturas y períodos ────────────────────────────

/// Período más corto posible: k = 5 (k − 4 tiene que ser al menos 1).
pub const PERIODO_MIN_US: u64 = 1020;

/// Mitades de un período en unidades de 60 µs → (k, ajuste fino).
fn k_y_ajuste(m: u64) -> (u16, bool) {
    (((m - 7) / 2) as u16, (m - 7) % 2 == 1)
}

/// Período en unidades de 60 µs más cercano a la altura MIDI (con transposición)
/// y su error en cents. `None` si la nota es demasiado aguda para el parlante.
pub fn periodo(altura: u8, transponer: i32) -> Option<(u64, f64)> {
    let f = 440.0 * 2f64.powf((altura as f64 + transponer as f64 - 69.0) / 12.0);
    let p = 1e6 / f;
    let m = (p / 60.0).round() as u64;
    if m > 7 + 2 * 1023 || m * 60 < PERIODO_MIN_US {
        return None;
    }
    Some((m, 1200.0 * ((m as f64 * 60.0) / p).log2()))
}

/// Elige la octava: la más aguda en la que ninguna nota se desafine más de
/// 25 cents; si ninguna lo logra, la de menor error.
pub fn transposicion_automatica(tramos: &[Tramo]) -> i32 {
    let peor = |tr: i32| -> Option<f64> {
        tramos.iter().map(|t| periodo(t.altura, tr).map(|(_, c)| c.abs())).try_fold(0f64, |a, c| c.map(|c| a.max(c)))
    };
    let opciones: Vec<(i32, f64)> = [12, 0, -12, -24, -36].into_iter().filter_map(|tr| peor(tr).map(|e| (tr, e))).collect();
    if let Some(&(tr, _)) = opciones.iter().find(|(_, e)| *e <= 25.0) {
        return tr;
    }
    opciones.iter().min_by(|a, b| a.1.total_cmp(&b.1)).map(|o| o.0).unwrap_or(-12)
}

// ──────────────────────────────── el guion ─────────────────────────────────

/// Bloque de texto que empieza a perforarse en un instante (µs de la canción).
#[derive(Debug, Clone, PartialEq)]
pub struct Bloque {
    pub desde: u64,
    pub texto: String,
}

/// Lee un guion: el texto se perfora tal cual; una línea `@SEGUNDOS` (o
/// `@MM:SS.s`) marca desde cuándo se perfora lo que sigue. `@@` al principio
/// de una línea es una arroba literal.
pub fn leer_guion(src: &str) -> Result<Vec<Bloque>, String> {
    let mut bloques = vec![Bloque { desde: 0, texto: String::new() }];
    for (n, linea) in src.lines().enumerate() {
        let l = linea.trim_end_matches('\r');
        if let Some(resto) = l.strip_prefix('@') {
            if !resto.starts_with('@') {
                let s = resto.trim();
                let seg = match s.split_once(':') {
                    Some((m, s)) => m.trim().parse::<f64>().ok().zip(s.trim().parse::<f64>().ok()).map(|(m, s)| m * 60.0 + s),
                    None => s.parse::<f64>().ok(),
                };
                let seg = seg.filter(|s| s.is_finite() && *s >= 0.0)
                    .ok_or_else(|| format!("guion, línea {}: tiempo inválido «{s}»", n + 1))?;
                bloques.push(Bloque { desde: (seg * 1e6) as u64, texto: String::new() });
                continue;
            }
        }
        let b = bloques.last_mut().unwrap();
        b.texto.push_str(l.strip_prefix('@').filter(|r| r.starts_with('@')).unwrap_or(l));
        b.texto.push('\n');
    }
    bloques.retain(|b| !b.texto.is_empty());
    Ok(bloques)
}

/// Lleva un carácter a uno que exista en el código Ferranti. Las minúsculas
/// pasan a mayúsculas; los signos que no existen, al más parecido.
fn normalizar(ch: char) -> Option<char> {
    let c = ch.to_ascii_uppercase();
    let hay = |c: char| tape::FIGURES.contains(&Some(c)) || tape::LETTERS.contains(&Some(c)) || c == '\n';
    if hay(c) {
        return Some(c);
    }
    let r = match c {
        '\t' => ' ',
        ':' | '!' | '·' => '.',
        ';' | '\'' | '"' | '`' => ',',
        '_' | '~' | '^' => '-',
        '|' | '\\' | '%' => '/',
        '<' | '[' | '{' => '(',
        ']' | '}' => ')',
        '&' => '+',
        '#' | '$' => '≠',
        '@' => '×',
        'Á' | 'À' => 'A',
        'É' => 'E',
        'Í' => 'I',
        'Ó' => 'O',
        'Ú' | 'Ü' => 'U',
        'Ñ' => 'N',
        _ => return None,
    };
    Some(r)
}

/// Codifica el texto siguiendo el registro (cifras o letras) que quedó del
/// bloque anterior. Devuelve los caracteres de cinta y cuántos signos hubo
/// que reemplazar o descartar.
pub fn codificar(texto: &str, shift: &mut Shift) -> (Vec<u16>, usize) {
    let mut out = Vec::new();
    let mut cambiados = 0;
    for ch in texto.chars() {
        if ch == '\r' {
            continue;
        }
        let Some(c) = normalizar(ch) else {
            cambiados += 1;
            continue;
        };
        if c != ch.to_ascii_uppercase() && c != ch {
            cambiados += 1;
        }
        if c == '\n' {
            if *shift != Shift::Figures {
                out.push(tape::FIG_SHIFT);
                *shift = Shift::Figures;
            }
            out.push(tape::CR);
            out.push(tape::LF);
            continue;
        }
        let fig = tape::FIGURES.iter().position(|x| *x == Some(c));
        let let_ = tape::LETTERS.iter().position(|x| *x == Some(c));
        let (quiere, v) = match (fig, let_) {
            (Some(f), Some(l)) => if *shift == Shift::Figures { (Shift::Figures, f) } else { (Shift::Letters, l) },
            (Some(f), None) => (Shift::Figures, f),
            (None, Some(l)) => (Shift::Letters, l),
            (None, None) => unreachable!(),
        };
        if quiere != *shift {
            out.push(if quiere == Shift::Figures { tape::FIG_SHIFT } else { tape::LET_SHIFT });
            *shift = quiere;
        }
        out.push(v as u16);
    }
    (out, cambiados)
}

// ─────────────────────────────── el reproductor ─────────────────────────────

/// El programa que corre en Clementina. Las etiquetas 20 y 21 son las dos
/// instrucciones que se reescriben en cada registro.
pub const REPRODUCTOR: &str = "\
101 0          ; B1 = 0, posicion en el sector
200 v40  (1    ; S = sector siguiente
677 0          ; T = S
680 2          ; pagina 2 = sector T
320 1
210 v40
101 0
201 128  (2    ; S = tipo del registro
280 v3
201 128+       ; parlante: 580 o 570
210 v20
201 129        ; ajuste fino: 590 o 570
210 v21
201 129+       ; k
210 v41
201 130        ; k - 4
210 v42
201 130+       ; N
210 v43
003 v43        ; B3 = N
201 131        ; q
210 v44
004 v44        ; B4 = q
011 v45
006 v45
126 7          ; B6 = primer caracter del registro
580 0    (20   ; un impulso al parlante, o nada
590 v22  (21   ; saltear, o no, los 60 microsegundos de ajuste
570 0
134 1    (22   ; B4 = B4 - 1
080 v23        ; si no toca perforar, esperar k vueltas
004 v44        ; B4 = q
206 128        ; S = caracter
210 v45
005 v45
625 0          ; perforar
126 1
002 v42        ; esperar k - 4 vueltas
590 v24
002 v41  (23   ; esperar k vueltas
132 1    (24
080 v24
133 1          ; un periodo menos
080 v20
016 v45        ; B1 = siguiente registro
001 v45
590 v2
370 1    (3    ; tipo 1: cargar el sector siguiente
280 v4
590 v1
990 0    (4    ; tipo 2: fin
=1       (40   ; sector siguiente
=0
=0       (41   ; k
=0
=0       (42   ; k - 4
=0
=0       (43   ; N
=0
=0       (44   ; q
=0
=0       (45   ; auxiliar
=0
";

/// Tiempo desde el comienzo de un registro hasta su primer impulso.
const PREPARACION_US: u64 = 19 * 60;
/// Tiempo de la salida de un registro (de la última vuelta al siguiente).
const SALIDA_US: u64 = 3 * 60;
/// Del comienzo de un registro de tipo 1 a la instrucción 68.
const HASTA_68_US: u64 = 7 * 60;
/// De la instrucción 68 al registro siguiente.
const DESPUES_68_US: u64 = 3 * 60;
/// Del arranque del programa a la instrucción 68.
const ARRANQUE_68_US: u64 = 3 * 60;
const SECTOR_INICIAL: usize = 1;
/// Período de los silencios: k = 5, sin parlante.
const PERIODO_SILENCIO_US: u64 = PERIODO_MIN_US;
const N_MAX: u64 = 1022;
const Q_NUNCA: u16 = 1023;
const MAX_CARACTERES: usize = 110;
const CABECERA: usize = 7;

/// Tiempo de una transferencia de tambor que empieza en `t`, igual que en `cpu.rs`.
fn tiempo_tambor(t: u64, sector: usize) -> u64 {
    let inicio = (sector as u64 & 1) * (DRUM_SECTOR_US + DRUM_GAP_US);
    let pos = t % DRUM_REV_US;
    (inicio + DRUM_REV_US - pos) % DRUM_REV_US + DRUM_SECTOR_US
}

/// Mitad izquierda (función y B) de una instrucción.
fn mitad_izquierda(instr: &str) -> u16 {
    let p = assemble(instr).expect("plantilla");
    p.shorts.iter().min_by_key(|x| x.0).map(|x| x.1).expect("plantilla vacía")
}

/// Un registro tal como lo va a tocar la máquina (para los tests y el resumen).
#[derive(Debug, Clone, PartialEq)]
pub struct Tocado {
    pub primer_impulso: u64,
    pub periodos: u64,
    pub periodo_us: u64,
    pub suena: bool,
    pub perforaciones: usize,
}

#[derive(Debug, Default)]
pub struct Partitura {
    pub sectores: Vec<[u16; SHORTS_PER_PAGE]>,
    pub tocados: Vec<Tocado>,
    /// Instante de máquina que corresponde al 0 de la canción.
    pub origen_us: u64,
    pub fin_us: u64,
    pub perforaciones: usize,
    /// Mayor diferencia entre el ataque de una nota y el MIDI (µs).
    pub peor_ataque_us: u64,
    /// Mayor retraso de un carácter respecto de su tiempo en el guion (µs).
    pub peor_retraso_texto_us: u64,
    pub peor_cents: f64,
    pub sin_altura: usize,
}

struct Armador {
    p: Partitura,
    sectores_tambor: usize,
    pos: usize,
    ahora: u64,
    cola: std::collections::VecDeque<(u64, u16)>,
    suena: u16,
    calla: u16,
    sin_ajuste: u16,
    con_ajuste: u16,
    /// Instante en que empezó la última perforación.
    ultima: Option<u64>,
}

/// Del impulso a la instrucción 625 dentro de un período que perfora
/// (580, 590 o 570 y 570, 134, 080, 004, 206, 210, 005).
fn hasta_perforar(t: u64) -> u64 {
    let (_, ajuste) = k_y_ajuste(t / 60);
    8 * 60 + if ajuste { 60 } else { 0 }
}

impl Armador {
    fn sector(&mut self) -> &mut [u16; SHORTS_PER_PAGE] {
        self.p.sectores.last_mut().unwrap()
    }

    /// Termina el sector: un registro de tipo 1 y la transferencia del siguiente.
    fn saltar_sector(&mut self) -> Result<(), String> {
        if self.sector_siguiente() >= self.sectores_tambor {
            return Err(format!("la partitura no entra en el tambor de {} sectores", self.sectores_tambor));
        }
        let pos = self.pos;
        self.sector()[pos] = 1;
        let t68 = self.ahora + HASTA_68_US;
        self.ahora = t68 + tiempo_tambor(t68, self.sector_siguiente()) + DESPUES_68_US;
        self.p.sectores.push([0; SHORTS_PER_PAGE]);
        self.pos = 0;
        Ok(())
    }

    fn sector_siguiente(&self) -> usize {
        SECTOR_INICIAL + self.p.sectores.len()
    }

    /// Cuántos caracteres perforar en `n` períodos de `t` µs, y cada cuántos.
    /// El registro tiene que dejar lugar en el sector para el de tipo 1.
    fn reparto(&self, n: u64, t: u64, inicio: u64) -> (usize, u64) {
        // Dentro del registro, una perforación cada q períodos; la primera,
        // a 30 ms de la última del registro anterior.
        let q_primero = match self.ultima {
            Some(u) => (u + 30_000).saturating_sub(inicio + hasta_perforar(t)).div_ceil(t) + 1,
            None => 1,
        };
        let q_min = 30_000u64.div_ceil(t).max(q_primero);
        let espacio = SHORTS_PER_PAGE - 1 - self.pos - CABECERA;
        let listos = self.cola.iter().take(MAX_CARACTERES.min(espacio)).take_while(|c| c.0 <= inicio).count();
        let mut c = listos.min((n / q_min) as usize);
        while c > 0 {
            let cc = c as u64;
            let q = q_min.max(n / (cc + 1) + 1);
            if q * cc <= n && q < Q_NUNCA as u64 {
                return (c, q);
            }
            c -= 1;
        }
        (0, Q_NUNCA as u64)
    }

    /// Agrega un registro de nota (`m` = período en unidades de 60 µs) o de
    /// silencio (`m` = None) de `n` períodos, con los caracteres que entren.
    /// Si después de él el sector queda casi lleno, el cambio de sector se
    /// hace enseguida y se descuenta de este mismo registro, para no atrasar
    /// el ataque de la nota siguiente.
    fn registro(&mut self, m: Option<u64>, n: u64) -> Result<(), String> {
        let mut n = n.clamp(1, N_MAX);
        let mm = m.unwrap_or(PERIODO_SILENCIO_US / 60);
        let t = mm * 60;
        if self.pos + CABECERA >= SHORTS_PER_PAGE {
            self.saltar_sector()?;
        }
        let inicio = self.ahora + PREPARACION_US;
        let (mut c, mut q) = self.reparto(n, t, inicio);
        let saltar = self.pos + 2 * CABECERA + c + 4 >= SHORTS_PER_PAGE;
        if saltar {
            let sector = self.sector_siguiente();
            let original = n;
            for _ in 0..3 {
                let t68 = inicio + n * t + SALIDA_US + HASTA_68_US;
                let d = HASTA_68_US + tiempo_tambor(t68, sector) + DESPUES_68_US;
                n = original.saturating_sub((d as f64 / t as f64).round() as u64).max(1);
            }
            (c, q) = self.reparto(n, t, inicio);
        }
        let (k, ajuste) = k_y_ajuste(mm);
        let mut r = vec![
            0,
            if m.is_some() { self.suena } else { self.calla },
            if ajuste { self.con_ajuste } else { self.sin_ajuste },
            k,
            k - 4,
            n as u16,
            q as u16,
        ];
        for j in 0..c {
            let (desde, ch) = self.cola.pop_front().unwrap();
            // El carácter j sale en el período q·(j+1).
            let sale = inicio + (q * (j as u64 + 1) - 1) * t + hasta_perforar(t);
            self.p.peor_retraso_texto_us = self.p.peor_retraso_texto_us.max(sale.saturating_sub(desde));
            r.push(ch);
        }
        let pos = self.pos;
        self.sector()[pos..pos + r.len()].copy_from_slice(&r);
        self.pos += r.len();
        self.p.perforaciones += c;
        if c > 0 {
            self.ultima = Some(inicio + (q * c as u64 - 1) * t + hasta_perforar(t));
        }
        self.p.tocados.push(Tocado { primer_impulso: inicio, periodos: n, periodo_us: t, suena: m.is_some(), perforaciones: c });
        self.ahora = inicio + n * t + SALIDA_US;
        if saltar {
            self.saltar_sector()?;
        }
        Ok(())
    }

    /// Silencio hasta que el próximo registro dé su primer impulso en `hasta`.
    fn silencio_hasta(&mut self, hasta: u64) -> Result<(), String> {
        let por_registro = PREPARACION_US + SALIDA_US;
        loop {
            let falta = hasta.saturating_sub(self.ahora + PREPARACION_US);
            if falta < por_registro + PERIODO_SILENCIO_US {
                return Ok(());
            }
            let n = ((falta - por_registro) as f64 / PERIODO_SILENCIO_US as f64).round() as u64;
            self.registro(None, n.min(N_MAX))?;
        }
    }
}

pub struct Opciones {
    pub transponer: i32,
}

/// Arma la partitura en el tambor a partir de la melodía y el guion.
pub fn armar(tramos: &[Tramo], bloques: &[Bloque], op: &Opciones, sectores_tambor: usize) -> Result<Partitura, String> {
    if sectores_tambor <= SECTOR_INICIAL {
        return Err("no hay sectores para la partitura".into());
    }
    let mut a = Armador {
        p: Partitura { sectores: vec![[0; SHORTS_PER_PAGE]], ..Default::default() },
        sectores_tambor,
        pos: 0,
        ahora: 0,
        cola: Default::default(),
        ultima: None,
        suena: mitad_izquierda("580 0"),
        calla: mitad_izquierda("570 0"),
        sin_ajuste: mitad_izquierda("590 0"),
        con_ajuste: mitad_izquierda("570 0"),
    };
    a.ahora = ARRANQUE_68_US + tiempo_tambor(ARRANQUE_68_US, SECTOR_INICIAL) + DESPUES_68_US;
    // Medio segundo de silencio al principio, para que el texto del instante 0
    // empiece antes que la música.
    let origen = a.ahora + PREPARACION_US + 500_000;
    a.p.origen_us = origen;
    // La perforadora arranca en cifras, como lo supone tape::decode.
    let mut shift = Shift::Figures;
    a.cola.push_back((0, tape::FIG_SHIFT));
    for b in bloques {
        let (cs, _) = codificar(&b.texto, &mut shift);
        a.cola.extend(cs.into_iter().map(|c| (origen.saturating_add(b.desde), c)));
    }
    for t in tramos {
        let Some((m, cents)) = periodo(t.altura, op.transponer) else {
            a.p.sin_altura += 1;
            continue;
        };
        a.p.peor_cents = a.p.peor_cents.max(cents.abs());
        let ataque = origen.saturating_add(t.inicio);
        a.silencio_hasta(ataque)?;
        let real = a.ahora + PREPARACION_US;
        a.p.peor_ataque_us = a.p.peor_ataque_us.max(real.abs_diff(ataque));
        let fin = origen.saturating_add(t.fin);
        let periodo_us = m * 60;
        let mut primero = true;
        loop {
            let desde = a.ahora + PREPARACION_US;
            if !primero && desde + periodo_us > fin {
                break;
            }
            let n = ((fin.saturating_sub(desde)) as f64 / periodo_us as f64).round() as u64;
            a.registro(Some(m), n.max(1))?;
            primero = false;
            if n <= N_MAX {
                break;
            }
        }
    }
    // Lo que quede del guion se perfora en silencio.
    while let Some(&(desde, _)) = a.cola.front() {
        a.silencio_hasta(desde)?;
        let n = ((a.cola.len() as u64 + 1) * 32).min(N_MAX);
        a.registro(None, n)?;
    }
    a.registro(None, 300)?;
    let pos = a.pos;
    a.sector()[pos] = 2;
    // 201, 280, 370, 280 y 990.
    a.p.fin_us = a.ahora + 5 * 60;
    Ok(a.p)
}

/// Imagen del tambor (cortos de 16 bits little-endian) con la partitura desde
/// el sector 1, del tamaño que espera `--tambor`.
pub fn imagen_tambor(p: &Partitura, sectores_tambor: usize) -> Result<Vec<u8>, String> {
    if SECTOR_INICIAL + p.sectores.len() > sectores_tambor {
        return Err(format!(
            "la partitura ocupa {} sectores y el tambor tiene {sectores_tambor}",
            p.sectores.len()
        ));
    }
    let mut img = vec![0u8; sectores_tambor * SHORTS_PER_PAGE * 2];
    for (i, s) in p.sectores.iter().enumerate() {
        for (j, v) in s.iter().enumerate() {
            let o = ((SECTOR_INICIAL + i) * SHORTS_PER_PAGE + j) * 2;
            img[o..o + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::BufferIo;
    use crate::{Config, Machine, Stop};

    fn vlq(mut v: u32, out: &mut Vec<u8>) {
        let mut b = vec![(v & 0x7f) as u8];
        v >>= 7;
        while v > 0 {
            b.push((v & 0x7f) as u8 | 0x80);
            v >>= 7;
        }
        out.extend(b.iter().rev());
    }

    /// MIDI de formato 1: pista 0 con el tempo, pista 1 con las notas
    /// (altura, duración en pulsos), con estado corrido y note-on de
    /// velocidad 0 como fin de nota.
    fn smf(notas: &[(u8, u32)], ppq: u16, tempo: u32) -> Vec<u8> {
        let mut d = b"MThd".to_vec();
        d.extend(6u32.to_be_bytes());
        d.extend(1u16.to_be_bytes());
        d.extend(2u16.to_be_bytes());
        d.extend(ppq.to_be_bytes());
        let mut p0 = vec![0, 0xff, 0x51, 3];
        p0.extend(&tempo.to_be_bytes()[1..]);
        p0.extend([0, 0xff, 0x2f, 0]);
        d.extend(b"MTrk");
        d.extend((p0.len() as u32).to_be_bytes());
        d.extend(p0);
        let mut p1 = vec![0, 0xff, 0x03, 4];
        p1.extend(b"Voz1");
        let mut espera = 0;
        for (i, &(a, dur)) in notas.iter().enumerate() {
            vlq(espera, &mut p1);
            if i == 0 {
                p1.push(0x90);
            }
            p1.extend([a, 100]);
            vlq(dur * 7 / 8, &mut p1);
            p1.extend([a, 0]);
            espera = dur - dur * 7 / 8;
        }
        vlq(espera, &mut p1);
        p1.extend([0xff, 0x2f, 0]);
        d.extend(b"MTrk");
        d.extend((p1.len() as u32).to_be_bytes());
        d.extend(p1);
        d
    }

    /// Clementine en Fa: sol sol sol re si si si sol...
    fn clementine() -> Vec<u8> {
        let n = [(67, 360), (67, 120), (67, 480), (62, 480), (71, 360), (71, 120), (71, 480), (67, 480)];
        smf(&n, 480, 600_000)
    }

    #[test]
    fn lee_el_midi() {
        let s = leer_smf(&clementine()).unwrap();
        assert_eq!(s.notas.len(), 8);
        assert_eq!(s.pistas[1].nombre, "Voz1");
        assert_eq!(s.notas[0].inicio, 0);
        // 360 pulsos · 7/8 a 600 000 µs por negra de 480 pulsos.
        assert_eq!(s.notas[0].fin, 315 * 600_000 / 480);
        assert_eq!(s.notas[1].inicio, 360 * 600_000 / 480);
    }

    #[test]
    fn la_mas_aguda_gana() {
        let notas = vec![
            Nota { inicio: 0, fin: 1000, altura: 60, canal: 0, pista: 0 },
            Nota { inicio: 200, fin: 600, altura: 72, canal: 0, pista: 0 },
            Nota { inicio: 0, fin: 1000, altura: 90, canal: 9, pista: 0 },
        ];
        let t = melodia(&notas, None, None, 0);
        assert_eq!(t.iter().map(|t| (t.inicio, t.fin, t.altura)).collect::<Vec<_>>(), vec![(0, 200, 60), (200, 600, 72), (600, 1000, 60)]);
    }

    #[test]
    fn guion_con_tiempos() {
        let b = leer_guion("HOLA\n@1.5\nCHAU\n@0:02\n@@ARROBA\n").unwrap();
        assert_eq!(b, vec![
            Bloque { desde: 0, texto: "HOLA\n".into() },
            Bloque { desde: 1_500_000, texto: "CHAU\n".into() },
            Bloque { desde: 2_000_000, texto: "@ARROBA\n".into() },
        ]);
    }

    #[test]
    fn alturas() {
        // La 440: 2272,7 µs → 38 unidades de 60 (2280), +5,6 cents.
        let (m, c) = periodo(69, 0).unwrap();
        assert_eq!(m, 38);
        assert!((c - 5.6).abs() < 0.1, "{c}");
        assert!(periodo(100, 0).is_none());
    }

    /// Corre el reproductor con la partitura y devuelve los impulsos, lo
    /// perforado y el tiempo de máquina al detenerse.
    fn tocar(p: &Partitura) -> (Vec<u64>, Vec<u16>, u64) {
        let mut m = Machine::new(Config::default());
        for (i, s) in p.sectores.iter().enumerate() {
            *m.drum.sector_mut(SECTOR_INICIAL + i) = *s;
        }
        let prog = assemble(REPRODUCTOR).unwrap();
        prog.load(&mut m.store);
        m.c = prog.start;
        let mut io = BufferIo::default();
        assert_eq!(m.run(&mut io, 200_000_000), Some(Stop::Stop99));
        (io.hoots, io.output[1].clone(), m.time_us)
    }

    #[test]
    fn la_maquina_toca_lo_que_dice_el_modelo() {
        let s = leer_smf(&clementine()).unwrap();
        let tramos = melodia(&s.notas, None, None, 15_000);
        let guion = "OH MY DARLING, OH MY DARLING\n@1.2\nOH MY DARLING CLEMENTINE.\n@2\n  ≠≠≠≠  \n ≠    ≠ \n  ≠≠≠≠  \n";
        let bloques = leer_guion(guion).unwrap();
        let tr = transposicion_automatica(&tramos);
        let p = armar(&tramos, &bloques, &Opciones { transponer: tr }, 512).unwrap();
        // La transposición elegida desafina poco.
        assert!(p.peor_cents <= 25.0, "{} cents", p.peor_cents);
        let (hoots, perforado, fin) = tocar(&p);

        // Cada impulso cae exactamente donde lo calculó el conversor; si la
        // perforadora hubiera hecho esperar a la máquina, no coincidiría.
        let esperados: Vec<u64> = p
            .tocados
            .iter()
            .filter(|t| t.suena)
            .flat_map(|t| (0..t.periodos).map(move |i| t.primer_impulso + i * t.periodo_us))
            .collect();
        assert_eq!(hoots.len(), esperados.len());
        assert!(hoots == esperados, "los impulsos no coinciden con el modelo");
        assert_eq!(fin, p.fin_us);

        // Los ataques caen cerca de los del MIDI, sin deriva.
        let ataques: Vec<u64> = p.tocados.iter().filter(|t| t.suena).map(|t| t.primer_impulso).collect();
        for (a, t) in ataques.iter().zip(&tramos) {
            let d = a.abs_diff(p.origen_us + t.inicio);
            assert!(d < 3_000, "ataque corrido {d} µs");
        }

        // Lo perforado es el guion.
        let texto = tape::decode(&perforado);
        assert_eq!(texto, guion.lines().filter(|l| !l.starts_with('@')).map(|l| format!("{l}\n")).collect::<String>());
        assert_eq!(perforado.len(), p.perforaciones);
    }

    #[test]
    fn un_texto_largo_cruza_sectores() {
        // Una sola nota larga y mucho texto: los caracteres se reparten en
        // varios sectores y lo que no entra en la nota sale después.
        let tramos = vec![Tramo { inicio: 0, fin: 6_000_000, altura: 57 }];
        let linea = "ABCDEFGHIJ 0123456789 KLMNOPQRST\n";
        let bloques = vec![Bloque { desde: 0, texto: linea.repeat(20) }];
        let p = armar(&tramos, &bloques, &Opciones { transponer: 0 }, 512).unwrap();
        assert!(p.sectores.len() > 3);
        let (hoots, perforado, fin) = tocar(&p);
        let esperados: Vec<u64> = p
            .tocados
            .iter()
            .filter(|t| t.suena)
            .flat_map(|t| (0..t.periodos).map(move |i| t.primer_impulso + i * t.periodo_us))
            .collect();
        assert_eq!(hoots.len(), esperados.len());
        assert!(hoots == esperados, "los impulsos no coinciden con el modelo");
        assert_eq!(fin, p.fin_us);
        assert_eq!(tape::decode(&perforado), linea.repeat(20));
    }
}
