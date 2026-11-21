//! TINY 2.1 de WollyMonjac en vivo: corre una cinta de tele-input y emite el
//! estado de la Mercury en JSON, una línea por evento, para `ports/tiny/en_vivo.py`.
//!
//! Uso: tiny_en_vivo CINTA.bin --pregunta "TEXTO" [--cada-ms N] [--max N]
//!
//! La primera línea trae la cinta completa (una cifra base 32 por fila). Las
//! siguientes, cada N ms de máquina o cuando se perfora algo: tiempo, contador
//! de instrucciones, fila del lector, registros, cursor del guion de TINY y los
//! sectores de tambor leídos y escritos desde el evento anterior. Para el
//! pupitre van también la última instrucción obedecida (`i`, y `fd` con su
//! función decimal), los registros de prueba Bt y St, el último corrimiento
//! (`sh`) y, en la línea final, el tipo de parada (`parada`).
//!
//! Para el modo difícil de `en_vivo.html`: `--llaves N` arma las llaves de
//! mano F (por omisión 4, la llave 2 del tele-input) y `--sin-escritura`
//! deja WRITE CURRENT en OFF.
use mercury_core::{asm::assemble_sector, io::Io, opcodes, tape, Machine, Stop};
use std::io::{BufWriter, Write};

struct Vivo {
    input: Vec<u16>,
    pos: usize,
    punched: Vec<u16>,
    llaves: u16,
    escritura: bool,
}

impl Io for Vivo {
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
        self.llaves
    }
    fn write_current(&mut self) -> bool {
        self.escritura
    }
}

fn lista(v: &[u16]) -> String {
    v.iter().map(u16::to_string).collect::<Vec<_>>().join(",")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let cinta = args.next().ok_or("uso: tiny_en_vivo CINTA.bin --pregunta TEXTO [--cada-ms N] [--max N] [--llaves N] [--sin-escritura]")?;
    let mut pregunta = None;
    let mut cada_us = 1_000_000u64;
    let mut max = 600_000_000u64;
    let mut llaves = 4u16; // llave de mano 2: tele-input
    let mut escritura = true;
    while let Some(a) = args.next() {
        let mut valor = || args.next().ok_or(format!("falta el valor de {a}"));
        match a.as_str() {
            "--pregunta" => pregunta = Some(valor()?),
            "--cada-ms" => cada_us = valor()?.parse::<u64>()? * 1000,
            "--max" => max = valor()?.parse()?,
            "--llaves" => llaves = valor()?.parse::<u16>()? & 0x3ff,
            "--sin-escritura" => escritura = false,
            s => return Err(format!("opción desconocida: {s}").into()),
        }
    }
    let pregunta = pregunta.ok_or("falta --pregunta")?;

    let mut input: Vec<u16> = std::fs::read(&cinta)?.into_iter().map(u16::from).collect();
    if input.is_empty() || input.iter().any(|&x| x > 31) {
        return Err("se esperaba una cinta binaria con filas de 5 bits".into());
    }
    input.extend(
        tape::encode(&format!("{}\n", pregunta.trim()))
            .map_err(|c| format!("el carácter {c:?} no existe en el código Ferranti"))?,
    );

    let mut m = Machine::new(Default::default());
    for (n, texto) in [
        include_str!("../../../input_routine/sector00.txt"),
        include_str!("../../../input_routine/sector01.txt"),
    ]
    .into_iter()
    .enumerate()
    {
        *m.drum.sector_mut(n) = assemble_sector(texto)?;
    }
    m.initial_transfer();

    let mut out = BufWriter::new(std::io::stdout().lock());
    const B32: &[u8] = b"0123456789abcdefghijklmnopqrstuv";
    let filas: String = input.iter().map(|&v| B32[v as usize] as char).collect();
    writeln!(out, "{{\"tipo\":\"cinta\",\"filas\":\"{filas}\"}}")?;
    out.flush()?;

    let mut io = Vivo { input, pos: 0, punched: Vec::new(), llaves, escritura };
    let mut leidos = vec![false; m.drum.len()];
    let mut escritos = vec![false; m.drum.len()];
    let mut perforados = 0;
    let mut ult_fila = 0;
    let mut proximo = 0u64;
    let mut stop = None;
    let mut ultima = 0u32;
    let mut ultima_dec = -1i32;

    // `i` es la última instrucción obedecida (registro I del pupitre); `fin`
    // trae el motivo legible y el tipo de parada para las lámparas.
    let mut emitir = |m: &Machine, io: &Vivo, i: u32, fd: i32, leidos: &mut [bool], escritos: &mut [bool], perforados: &mut usize, fin: Option<(&str, &str)>| -> std::io::Result<()> {
        let tomar = |v: &mut [bool]| -> Vec<u16> {
            let r = v.iter().enumerate().filter(|(_, &x)| x).map(|(i, _)| i as u16).collect();
            v.fill(false);
            r
        };
        let (r, w) = (tomar(leidos), tomar(escritos));
        let s = |i| m.store.short(i);
        write!(
            out,
            "{{\"t\":{},\"n\":{},\"fila\":{},\"c\":{},\"a\":{},\"al\":[{}],\"b\":[{}],\"cur\":{},\"sec\":{},\"pos\":{},\"tok\":{},\"modo\":{},\"banco\":{},\"r\":[{}],\"w\":[{}],\"p\":[{}]",
            m.time_us, m.executed, io.pos, m.c,
            { let a = m.acc.to_f64(); if a.is_finite() { a } else { 0.0 } },
            lista(&m.acc.to_long()), lista(&m.b[1..]),
            s(101), s(108), s(102), s(103), s(104), s(107),
            lista(&r), lista(&w), lista(&io.punched[*perforados..])
        )?;
        write!(out, ",\"i\":{i},\"fd\":{fd},\"bt\":{},\"st\":{},\"sh\":{}", m.bt, m.st, m.shift)?;
        *perforados = io.punched.len();
        if let Some((motivo, parada)) = fin {
            write!(out, ",\"fin\":\"{motivo}\",\"parada\":\"{parada}\"")?;
        }
        writeln!(out, "}}")?;
        out.flush()
    };

    let sectores = m.drum.len();
    for _ in 0..max {
        let palabra = m.store.medium(m.c as usize);
        let decodificada = opcodes::decode(((palabra >> 13) & 0x7f) as u8);
        ultima_dec = decodificada.map_or(-1, |d| d.dec as i32);
        if let Some(d) = decodificada {
            let s = m.t as usize;
            if s < sectores {
                match d.dec {
                    68 => leidos[s] = true,
                    69 if escritura => escritos[s] = true,
                    _ => {}
                }
            }
        }
        ultima = palabra;
        if let Err(s) = m.step(&mut io) {
            stop = Some(s);
            break;
        }
        let leyo_pregunta = m.c >= 640 && io.pos > ult_fila;
        if m.time_us >= proximo || io.punched.len() > perforados || leyo_pregunta {
            ult_fila = io.pos;
            emitir(&m, &io, ultima, ultima_dec, &mut leidos, &mut escritos, &mut perforados, None)?;
            while proximo <= m.time_us {
                proximo += cada_us;
            }
        }
    }
    let estado = m.store.medium((m.c.wrapping_sub(1) & 1023) as usize) & 1023;
    let motivo = match stop {
        Some(Stop::Stop99) if m.c == 17 => "el tele-input rechazó la suma de control".to_string(),
        Some(Stop::Stop99) if estado == 0 => "parada 99".to_string(),
        Some(Stop::Stop99) => format!("990 {estado}"),
        Some(Stop::PrepulseStop { bin_fn }) => format!("parada prepulsable ({bin_fn:03o})"),
        Some(Stop::Paridad { .. }) => "parada por paridad".to_string(),
        Some(Stop::Overflow) => "desborde del acumulador".to_string(),
        Some(Stop::OptionalStop) => "parada opcional".to_string(),
        Some(Stop::NonExistentSector { sector }) => format!("sector de tambor inexistente ({sector})"),
        Some(Stop::LoopStop) => "parada por bucle".to_string(),
        Some(Stop::Single) => "parada paso a paso (SINGLE)".to_string(),
        Some(Stop::Manual) => "modo manual".to_string(),
        Some(Stop::InputExhausted { channel }) => format!("se acabó la cinta del canal {channel}"),
        Some(Stop::Unimplemented { dec }) => format!("instrucción {dec:02} no emulada"),
        None => format!("límite de {max} instrucciones"),
    };
    let parada = match stop {
        Some(Stop::Stop99) => "99",
        Some(Stop::PrepulseStop { .. }) => "prepulsable",
        Some(Stop::Overflow) => "desborde",
        Some(Stop::OptionalStop) => "opcional",
        Some(Stop::NonExistentSector { .. }) => "sector",
        Some(Stop::LoopStop) => "bucle",
        Some(Stop::Single) => "single",
        Some(Stop::Manual) => "manual",
        Some(Stop::InputExhausted { .. }) => "cinta",
        Some(Stop::Unimplemented { .. }) => "no_emulada",
        Some(Stop::Paridad { .. }) => "paridad",
        None => "limite",
    };
    emitir(&m, &io, ultima, ultima_dec, &mut leidos, &mut escritos, &mut perforados, Some((&motivo, parada)))?;
    Ok(())
}
