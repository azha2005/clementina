//! Interfaz WebAssembly (y biblioteca nativa testeable) del emulador de
//! Clementina y del compilador de Mercury Autocode.
//!
//! No usa dependencias externas ni `wasm-bindgen`: expone funciones `extern "C"`
//! que operan sobre una máquina persistente y buffers en memoria lineal.

mod input_routine;

use mercury_core::acc::Float;
use mercury_core::asm;
use mercury_core::io::Io;
use mercury_core::midi;
use mercury_core::opcodes::{self, AddrType};
use mercury_core::store::{Drum, SHORTS_PER_PAGE};
use mercury_core::{tape, Config, Machine, Stop};
use std::fmt::Write as _;
use std::sync::{Mutex, OnceLock};

/// Códigos de parada compartidos con el pupitre web.
pub const STOP_NONE: u32 = 0;
pub const STOP_99: u32 = 1;
pub const STOP_PREPULSE: u32 = 2;
pub const STOP_OVERFLOW: u32 = 3;
pub const STOP_OPTIONAL: u32 = 4;
pub const STOP_NON_EXISTENT_SECTOR: u32 = 5;
pub const STOP_LOOP: u32 = 6;
pub const STOP_SINGLE: u32 = 7;
pub const STOP_MANUAL: u32 = 8;
pub const STOP_INPUT_EXHAUSTED: u32 = 9;
pub const STOP_UNIMPLEMENTED: u32 = 10;
pub const STOP_PARIDAD: u32 = 11;

/// Periféricos de la sesión web en memoria lineal.
pub struct WasmIo {
    pub input: Vec<u16>,
    pub input_pos: usize,
    pub output: Vec<u16>,
    pub switches: u16,
    pub function_keys: u16,
    pub address_keys: u16,
    pub handswitch_stop: bool,
    pub single_mode: bool,
    pub manual_mode: bool,
    pub write_current: bool,
    pub inhibit_parities: bool,
    pub hoots: Vec<u64>,
    pub display_long: [u16; 4],
    pub has_display: bool,
}

impl Default for WasmIo {
    fn default() -> Self {
        Self {
            input: Vec::new(),
            input_pos: 0,
            output: Vec::new(),
            switches: 0,
            function_keys: 0,
            address_keys: 0,
            handswitch_stop: false,
            single_mode: false,
            manual_mode: false,
            write_current: true,
            inhibit_parities: false,
            hoots: Vec::new(),
            display_long: [0; 4],
            has_display: false,
        }
    }
}

impl Io for WasmIo {
    fn read(&mut self, channel: u8, _device: u16) -> Option<u16> {
        if channel != 1 {
            return None;
        }
        let v = *self.input.get(self.input_pos)?;
        self.input_pos += 1;
        Some(v)
    }

    fn write(&mut self, channel: u8, _device: u16, value: u16) {
        if channel == 1 {
            self.output.push(value & 0x1f);
        }
    }

    fn handswitches(&mut self) -> u16 {
        self.switches & 0x3ff
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
        self.display_long = long;
        self.has_display = true;
    }
}

pub fn stop_to_code(stop: Option<Stop>) -> (u32, u32) {
    match stop {
        None => (STOP_NONE, 0),
        Some(Stop::Stop99) => (STOP_99, 0),
        Some(Stop::PrepulseStop { bin_fn }) => (STOP_PREPULSE, bin_fn as u32),
        Some(Stop::Overflow) => (STOP_OVERFLOW, 0),
        Some(Stop::OptionalStop) => (STOP_OPTIONAL, 0),
        Some(Stop::NonExistentSector { sector }) => (STOP_NON_EXISTENT_SECTOR, sector as u32),
        Some(Stop::LoopStop) => (STOP_LOOP, 0),
        Some(Stop::Single) => (STOP_SINGLE, 0),
        Some(Stop::Manual) => (STOP_MANUAL, 0),
        Some(Stop::InputExhausted { channel }) => (STOP_INPUT_EXHAUSTED, channel as u32),
        Some(Stop::Unimplemented { dec }) => (STOP_UNIMPLEMENTED, dec as u32),
        Some(Stop::Paridad { sector, corto }) => {
            let almacen = sector.map_or(0, |s| u32::from(s) + 1);
            (STOP_PARIDAD, (almacen << 12) | u32::from(corto))
        }
    }
}

const BYTES_MAPA_TAMBOR: usize = 1024 / 8;

fn bitset_marcar(bits: &mut [u8; BYTES_MAPA_TAMBOR], sector: usize) {
    if sector / 8 < bits.len() {
        bits[sector >> 3] |= 1 << (sector & 7);
    }
}

fn tambor_desde_imagen(datos: &[u8], sectores: usize) -> Result<Drum, String> {
    let mut tambor = Drum::new(sectores);
    if sectores == 1024 && datos.len() == 512 * SHORTS_PER_PAGE * 2 {
        let mut imagen = datos.to_vec();
        imagen.resize(sectores * SHORTS_PER_PAGE * 2, 0);
        tambor.cargar_imagen(&imagen)?;
    } else {
        tambor.cargar_imagen(datos)?;
    }
    Ok(tambor)
}

/// Estado completo de una sesión interactiva de Clementina en memoria lineal.
pub struct WasmMachine {
    pub m: Machine,
    pub io: WasmIo,
    pub batch_drum_read: [u8; BYTES_MAPA_TAMBOR],
    pub batch_drum_write: [u8; BYTES_MAPA_TAMBOR],
    pub drum_written: [u8; BYTES_MAPA_TAMBOR],
    pub batch_core_pages: u32,
    pub last_drum_sector_read: i32,
    pub last_instr: u32,
    pub last_fn_dec: i32,
    pub has_last_instr: bool,
    pub last_stop_code: u32,
    pub last_stop_arg: u32,
    pub powered: bool,
    pub scratch: Vec<u8>,
    pub last_error: Vec<u8>,
    pub result_buf: Vec<u8>,
    pub cinta_vuelta: u16,
    pub autocode_header: Vec<u8>,
    pub autocode_data: Vec<u8>,
    pub hoots_drain: Vec<f64>,
}

impl Default for WasmMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmMachine {
    pub fn new() -> Self {
        let mut s = Self {
            m: Machine::new(Config::default()),
            io: WasmIo::default(),
            batch_drum_read: [0; BYTES_MAPA_TAMBOR],
            batch_drum_write: [0; BYTES_MAPA_TAMBOR],
            drum_written: [0; BYTES_MAPA_TAMBOR],
            batch_core_pages: 0,
            last_drum_sector_read: -1,
            last_instr: 0,
            last_fn_dec: -1,
            has_last_instr: false,
            last_stop_code: STOP_NONE,
            last_stop_arg: 0,
            powered: false,
            scratch: Vec::new(),
            last_error: Vec::new(),
            result_buf: Vec::new(),
            cinta_vuelta: 0,
            autocode_header: Vec::new(),
            autocode_data: Vec::new(),
            hoots_drain: Vec::new(),
        };
        s.reset(false);
        s
    }

    fn set_error(&mut self, msg: impl Into<String>) {
        self.last_error = msg.into().into_bytes();
    }

    pub fn clear_batch_telemetry(&mut self) {
        self.batch_drum_read.fill(0);
        self.batch_drum_write.fill(0);
        self.batch_core_pages = 0;
    }

    pub fn reset(&mut self, full_input_routine: bool) {
        let prev_switches = self.io.switches;
        let prev_j = self.io.function_keys;
        let prev_d = self.io.address_keys;
        let prev_hs = self.io.handswitch_stop;
        let prev_single = self.io.single_mode;
        let prev_manual = self.io.manual_mode;
        let prev_write = self.io.write_current;
        let prev_inhibit = self.io.inhibit_parities;

        self.m = Machine::new(self.m.config.clone());
        self.io = WasmIo {
            switches: prev_switches,
            function_keys: prev_j,
            address_keys: prev_d,
            handswitch_stop: prev_hs,
            single_mode: prev_single,
            manual_mode: prev_manual,
            write_current: prev_write,
            inhibit_parities: prev_inhibit,
            ..Default::default()
        };
        self.clear_batch_telemetry();
        self.drum_written.fill(0);
        self.last_drum_sector_read = -1;
        self.last_instr = 0;
        self.last_fn_dec = -1;
        self.has_last_instr = false;
        self.last_stop_code = STOP_NONE;
        self.last_stop_arg = 0;
        self.powered = false;
        self.last_error.clear();
        self.result_buf.clear();
        self.cinta_vuelta = 0;
        self.autocode_header.clear();
        self.autocode_data.clear();
        self.hoots_drain.clear();

        if full_input_routine {
            input_routine::cargar_completo(&mut self.m);
            for s in 0..62 {
                bitset_marcar(&mut self.drum_written, s);
            }
        } else {
            input_routine::cargar_tele_input(&mut self.m);
            for s in 0..2 {
                bitset_marcar(&mut self.drum_written, s);
            }
        }
    }

    pub fn load_written(&mut self, src: &str) -> Result<(), String> {
        let prog = asm::assemble(src).map_err(|e| e.to_string())?;
        self.reset(true);
        prog.load(&mut self.m.store);
        self.m.goto(prog.start);
        self.powered = true;
        Ok(())
    }

    fn instalar_tambor(&mut self, tambor: Drum) {
        self.drum_written.fill(0);
        for s in 0..tambor.len() {
            if tambor.sector(s).iter().any(|&palabra| palabra != 0) {
                bitset_marcar(&mut self.drum_written, s);
            }
        }
        self.m.drum = tambor;
    }

    /// Núcleos y tambor guardados por la consola. Si alguna imagen es
    /// inválida no cambia nada; si no, la máquina queda como después de OFF.
    pub fn restaurar_maquina(&mut self, nucleos: &[u8], tambor: &[u8]) -> Result<(), String> {
        let tambor = tambor_desde_imagen(tambor, self.m.config.drum_sectors)?;
        self.m.store.cargar_imagen(nucleos)?;
        self.instalar_tambor(tambor);
        self.power_off();
        Ok(())
    }

    pub fn load_written_drum(&mut self, src: &str, datos: &[u8]) -> Result<(), String> {
        let tambor = tambor_desde_imagen(datos, self.m.config.drum_sectors)?;
        self.load_written(src)?;
        self.instalar_tambor(tambor);
        Ok(())
    }

    pub fn inspect_midi(&mut self, datos: &[u8]) -> Result<(), String> {
        let smf = leer_midi(datos)?;
        let mut json = String::from("{\"pistas\":[");
        for (i, pista) in smf.pistas.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            write!(json, "{{\"pista\":{i},\"notas\":{},\"canales\":{:?},\"nombre\":", pista.notas, pista.canales).unwrap();
            escribir_cadena_json(&mut json, &pista.nombre);
            json.push('}');
        }
        json.push_str("]}");
        self.result_buf = json.into_bytes();
        Ok(())
    }

    pub fn load_midi(
        &mut self, datos: &[u8], guion: &str, pista: Option<usize>,
        canal: Option<u8>, transponer: Option<i32>, separacion_ms: u32,
    ) -> Result<(), String> {
        let (imagen, resumen) = convertir_midi(datos, guion, pista, canal, transponer, separacion_ms)?;
        self.load_written_drum(midi::REPRODUCTOR, &imagen)?;
        self.result_buf = resumen.into_bytes();
        Ok(())
    }

    pub fn load_autocode(&mut self, src: &str) -> Result<(), String> {
        let img = mercury_autocode::compilar(src).map_err(|e| e.to_string())?;
        let datos = img.datos.as_deref().map(limpiar_datos_autocode);
        let filas = datos.as_deref().map(tape::encode).transpose()
            .map_err(|ch| format!("carácter inválido en datos: '{ch}'"))?;
        self.reset(false);
        img.cargar(&mut self.m);
        for (s, _) in &img.tambor {
            bitset_marcar(&mut self.drum_written, *s);
        }
        self.autocode_header = img.salida_traduccion.into_bytes();
        if let (Some(d), Some(filas)) = (datos, filas) {
            self.autocode_data = d.into_bytes();
            self.io.input = filas;
        }
        self.m.goto(img.arranque);
        self.powered = true;
        Ok(())
    }

    pub fn load_input_routine_tape(&mut self, src: &str) -> Result<(), String> {
        let filas = tape::encode(src)
            .map_err(|ch| format!("el carácter '{ch}' no existe en el código Ferranti"))?;
        self.reset(true);
        self.io.input = filas;
        Ok(())
    }

    pub fn set_tape_binary(&mut self, bytes: &[u8]) {
        self.io.input = bytes.iter().map(|&b| u16::from(b) & 0x1f).collect();
        self.io.input_pos = 0;
    }

    pub fn set_tape_text(&mut self, text: &str) -> Result<(), String> {
        let filas = tape::encode(text)
            .map_err(|ch| format!("el carácter '{ch}' no existe en el código Ferranti"))?;
        self.io.input = filas;
        self.io.input_pos = 0;
        Ok(())
    }

    pub fn append_tape_text(&mut self, text: &str) -> Result<(), String> {
        let filas = tape::encode(text)
            .map_err(|ch| format!("el carácter '{ch}' no existe en el código Ferranti"))?;
        self.io.input.extend(filas);
        Ok(())
    }

    pub fn set_switches(&mut self, f: u16, j: u16, d: u16, flags: u32) {
        self.io.switches = f & 0x3ff;
        self.io.function_keys = j & 0x3ff;
        self.io.address_keys = d & 0x3ff;
        self.io.handswitch_stop = (flags & 1) != 0;
        self.io.single_mode = (flags & 2) != 0;
        self.io.manual_mode = (flags & 4) != 0;
        self.io.write_current = (flags & 8) != 0;
        self.io.inhibit_parities = (flags & 16) != 0;
    }

    pub fn initial_transfer(&mut self) {
        self.clear_batch_telemetry();
        self.m.initial_transfer_con_paridades(self.io.inhibit_parities);
        (self.last_stop_code, self.last_stop_arg) = stop_to_code(self.m.stop());
        if self.last_stop_code == STOP_NONE {
            bitset_marcar(&mut self.batch_drum_read, 0);
            self.last_drum_sector_read = 0;
            self.batch_core_pages |= 1;
        }
        self.powered = true;
    }

    pub fn power_off(&mut self) {
        // Los núcleos de ferrita y el tambor no son volátiles; los registros de
        // trabajo se limpian como Supuesto: al cortar la alimentación.
        self.powered = false;
        self.m.b = [0; 8];
        self.m.bt = 0;
        self.m.st = 0;
        self.m.acc = Float::default();
        self.m.t = 0;
        self.m.mitad_alta_cortos = false;
        self.m.shift = 0;
        self.m.goto(0);
        self.io.hoots.clear();
        self.hoots_drain.clear();
        self.io.display_long = [0; 4];
        self.io.has_display = false;
        self.clear_batch_telemetry();
        self.has_last_instr = false;
        self.last_instr = 0;
        self.last_fn_dec = -1;
        self.last_stop_code = STOP_NONE;
        self.last_stop_arg = 0;
    }

    fn registrar_paginas_instruccion(&mut self, word: u32, fetched_from_c: bool) -> (Option<u8>, u16, u16) {
        let here = self.m.c & 0x3ff;
        if fetched_from_c {
            self.batch_core_pages |= 1 << ((here >> 6) & 31);
        }
        let bin_fn = ((word >> 13) & 0x7f) as u8;
        let bd = ((word >> 10) & 7) as usize;
        let n = (word & 0x3ff) as u16;
        let dc = opcodes::decode(bin_fn);
        let Some(dc) = dc else {
            return (None, n, self.m.t);
        };
        let dec = dc.dec;
        let bmod = if opcodes::is_b_modifiable(dec) && bd != 0 {
            self.m.b[bd]
        } else {
            0
        };
        let ma = n.wrapping_add(bmod) & 0x3ff;
        match opcodes::addr_type(dec) {
            AddrType::Short | AddrType::ShortFirstQuarter => {
                let sa = self.m.direccion_corta(dc, n, bmod).unwrap();
                let page = (sa >> 7) & 31;
                self.batch_core_pages |= 1 << page;
            }
            AddrType::Long => {
                let page = ((ma as u32) >> 5) & 31;
                self.batch_core_pages |= 1 << page;
            }
            _ => {}
        }
        if matches!(dec, 68 | 69) {
            let page = (ma as u32) & 31;
            self.batch_core_pages |= 1 << page;
        }
        (Some(dec), ma, self.m.t)
    }

    pub fn step_tracked(&mut self, via_prepulse: bool) -> u32 {
        let manual = self.io.manual_mode;
        let (word, fetched_from_c) = if via_prepulse && manual {
            let w = ((u32::from(self.io.function_keys & 0x3ff)) << 10)
                | u32::from(self.io.address_keys & 0x3ff);
            (w, false)
        } else {
            (self.m.store.medium((self.m.c & 0x3ff) as usize), true)
        };

        let mut dec = None;
        let mut sector_at_entry = self.m.t as usize;
        let already_halted_same_mode = !via_prepulse
            && match self.m.stop() {
                Some(Stop::Single) => self.io.single_mode,
                Some(Stop::Manual) => self.io.manual_mode,
                Some(_) => true,
                None => false,
            };

        let will_enter_manual_or_single = !via_prepulse
            && !already_halted_same_mode
            && (self.io.manual_mode || self.io.single_mode);

        if !already_halted_same_mode && !will_enter_manual_or_single {
            let (d, _ma, sec) = self.registrar_paginas_instruccion(word, fetched_from_c);
            dec = d;
            sector_at_entry = sec as usize;
            self.last_instr = word;
            self.last_fn_dec = dec.map_or(-1, i32::from);
            self.has_last_instr = true;
        }

        let exec_before = self.m.executed;
        let res = if via_prepulse {
            self.m.prepulse(&mut self.io)
        } else {
            self.m.step(&mut self.io)
        };
        let exec_grew = self.m.executed > exec_before;

        // 68/69 sólo transfieren si no cayeron en excepción de parada opcional ni sector inexistente.
        if exec_grew && matches!(res, Ok(()) | Err(Stop::Single)) {
            if dec == Some(68) && sector_at_entry < self.m.drum.len() {
                bitset_marcar(&mut self.batch_drum_read, sector_at_entry);
                self.last_drum_sector_read = sector_at_entry as i32;
            } else if dec == Some(69) && self.io.write_current && sector_at_entry < self.m.drum.len() {
                bitset_marcar(&mut self.batch_drum_write, sector_at_entry);
                bitset_marcar(&mut self.drum_written, sector_at_entry);
            }
        }

        let (code, arg) = match res {
            Ok(()) => (STOP_NONE, 0),
            Err(s) => stop_to_code(Some(s)),
        };
        self.last_stop_code = code;
        self.last_stop_arg = arg;
        code
    }

    pub fn prepulse(&mut self) -> u32 {
        self.clear_batch_telemetry();
        self.powered = true;
        self.step_tracked(true)
    }

    pub fn step_batch(&mut self, max_steps: u32, max_us: u32) -> u32 {
        self.ejecutar_lote(max_steps, max_us, None)
    }

    fn ejecutar_lote(&mut self, max_steps: u32, max_us: u32, entrada: Option<(u16, usize)>) -> u32 {
        self.clear_batch_telemetry();
        if !self.powered {
            return self.last_stop_code;
        }
        let t0 = self.m.time_us;
        let steps = max_steps.max(1);
        for i in 0..steps {
            if let Some((direccion, filas)) = entrada {
                if self.m.c == direccion && self.io.input_pos >= filas {
                    break;
                }
            }
            let code = self.step_tracked(false);
            if code != STOP_NONE {
                return code;
            }
            if max_us > 0 && i + 1 < steps && self.m.time_us.saturating_sub(t0) >= u64::from(max_us) {
                break;
            }
        }
        STOP_NONE
    }

    pub fn drain_hoots(&mut self) -> u32 {
        self.hoots_drain.clear();
        self.hoots_drain.extend(self.io.hoots.drain(..).map(|t| t as f64));
        self.hoots_drain.len() as u32
    }
}

static GLOBAL: OnceLock<Mutex<WasmMachine>> = OnceLock::new();

fn limpiar_datos_autocode(raw: &str) -> String {
    raw.lines()
        .map(|l| format!("{}\n", l.split("//").next().unwrap_or("").trim()))
        .collect()
}

/// Melodía original de cuatro notas para probar el conversor sin un archivo.
pub const MIDI_PRUEBA: &[u8] = b"MThd\x00\x00\x00\x06\x00\x00\x00\x01\x00\x60MTrk\x00\x00\x00\x24\
\x00\x90\x3c\x60\x60\x80\x3c\x00\
\x00\x90\x40\x60\x60\x80\x40\x00\
\x00\x90\x43\x60\x60\x80\x43\x00\
\x00\x90\x48\x60\x60\x80\x48\x00\
\x00\xff\x2f\x00";

fn convertir_midi(
    datos: &[u8], guion: &str, pista: Option<usize>, canal: Option<u8>,
    transponer: Option<i32>, separacion_ms: u32,
) -> Result<(Vec<u8>, String), String> {
    let smf = leer_midi(datos)?;
    if pista.is_some_and(|p| p >= smf.pistas.len())
        || canal.is_some_and(|c| c > 15)
        || transponer.is_some_and(|t| !(-127..=127).contains(&t))
        || separacion_ms > 1000 {
        return Err("opciones de MIDI fuera de rango".into());
    }
    if guion.len() > 128 * 1024 {
        return Err("el guion supera 128 KiB".into());
    }
    let bloques = midi::leer_guion(guion)?;
    let tramos = midi::melodia(&smf.notas, pista, canal, u64::from(separacion_ms) * 1000);
    if tramos.is_empty() {
        return Err("no hay notas en la pista y el canal elegidos".into());
    }
    const DOS_HORAS_US: u64 = 7_200_000_000;
    if tramos.iter().any(|t| t.fin > DOS_HORAS_US) || bloques.iter().any(|b| b.desde > DOS_HORAS_US) {
        return Err("el MIDI o el guion supera las dos horas".into());
    }
    let transponer = transponer.unwrap_or_else(|| midi::transposicion_automatica(&tramos));
    let partitura = midi::armar(&tramos, &bloques, &midi::Opciones { transponer }, Config::default().drum_sectors)?;
    if partitura.sin_altura == tramos.len() {
        return Err("ninguna nota entra en el registro de HOOT con esa transposición".into());
    }
    let imagen = midi::imagen_tambor(&partitura, Config::default().drum_sectors)?;
    let resumen = format!(
        "{{\"transposicion\":{transponer},\"duracion_us\":{},\"sectores\":{},\"notas_omitidas\":{}}}",
        partitura.fin_us, partitura.sectores.len(), partitura.sin_altura,
    );
    Ok((imagen, resumen))
}

fn leer_midi(datos: &[u8]) -> Result<midi::Smf, String> {
    if datos.len() > 4 * 1024 * 1024 {
        return Err("el MIDI supera 4 MiB".into());
    }
    midi::leer_smf(if datos.is_empty() { MIDI_PRUEBA } else { datos })
}

fn escribir_cadena_json(json: &mut String, valor: &str) {
    json.push('"');
    for c in valor.chars() {
        match c {
            '\\' => json.push_str("\\\\"),
            '"' => json.push_str("\\\""),
            '\0'..='\u{1f}' => write!(json, "\\u{:04x}", u32::from(c)).unwrap(),
            _ => json.push(c),
        }
    }
    json.push('"');
}

fn with_machine<R>(f: impl FnOnce(&mut WasmMachine) -> R) -> R {
    let lock = GLOBAL.get_or_init(|| Mutex::new(WasmMachine::new()));
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

fn read_scratch_str(wm: &WasmMachine, len: usize) -> Result<String, String> {
    let n = len.min(wm.scratch.len());
    String::from_utf8(wm.scratch[..n].to_vec()).map_err(|_| "texto UTF-8 inválido".into())
}

#[no_mangle]
pub extern "C" fn wasm_alloc_scratch(len: usize) -> *mut u8 {
    with_machine(|wm| {
        wm.scratch.resize(len, 0);
        wm.scratch.as_mut_ptr()
    })
}

#[no_mangle]
pub extern "C" fn wasm_error_ptr() -> *const u8 {
    with_machine(|wm| wm.last_error.as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_error_len() -> usize {
    with_machine(|wm| wm.last_error.len())
}

#[no_mangle]
pub extern "C" fn wasm_result_ptr() -> *const u8 {
    with_machine(|wm| wm.result_buf.as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_result_len() -> usize {
    with_machine(|wm| wm.result_buf.len())
}

#[no_mangle]
pub extern "C" fn wasm_get_cinta_vuelta() -> u32 {
    with_machine(|wm| wm.cinta_vuelta.into())
}

fn operacion_scratch(f: impl FnOnce(&mut WasmMachine, &[u8]) -> Result<(), String>) -> u32 {
    with_machine(|wm| {
        wm.last_error.clear();
        wm.result_buf.clear();
        wm.cinta_vuelta = 0;
        let scratch = std::mem::take(&mut wm.scratch);
        let resultado = f(wm, &scratch);
        wm.scratch = scratch;
        match resultado {
            Ok(()) => 1,
            Err(e) => {
                wm.set_error(e);
                0
            }
        }
    })
}

fn dividir_scratch(datos: &[u8], len: usize) -> Result<(&[u8], &[u8]), String> {
    if len > datos.len() {
        return Err("longitud fuera del búfer de entrada".into());
    }
    Ok(datos.split_at(len))
}

#[no_mangle]
pub extern "C" fn wasm_load_drum_image(len: usize) -> u32 {
    operacion_scratch(|wm, datos| {
        let (datos, _) = dividir_scratch(datos, len)?;
        let tambor = tambor_desde_imagen(datos, wm.m.config.drum_sectors)?;
        wm.instalar_tambor(tambor);
        Ok(())
    })
}

/// Deja la imagen del tambor en el búfer de resultado (`wasm_result_ptr`).
#[no_mangle]
pub extern "C" fn wasm_imagen_tambor() -> usize {
    with_machine(|wm| {
        wm.result_buf = wm.m.drum.imagen();
        wm.result_buf.len()
    })
}

/// Deja la imagen de los núcleos en el búfer de resultado.
#[no_mangle]
pub extern "C" fn wasm_imagen_nucleos() -> usize {
    with_machine(|wm| {
        wm.result_buf = wm.m.store.imagen();
        wm.result_buf.len()
    })
}

/// Recupera una máquina guardada: núcleos y tambor seguidos en el búfer de
/// entrada. Queda como después de OFF, que conserva los dos.
#[no_mangle]
pub extern "C" fn wasm_restaurar_maquina(len_nucleos: usize) -> u32 {
    operacion_scratch(|wm, datos| {
        let (nucleos, tambor) = dividir_scratch(datos, len_nucleos)?;
        wm.restaurar_maquina(nucleos, tambor)
    })
}

/// Entrada: código escrito (UTF-8) seguido por la imagen de tambor.
#[no_mangle]
pub extern "C" fn wasm_load_written_drum(len_codigo: usize) -> u32 {
    operacion_scratch(|wm, datos| {
        let (codigo, tambor) = dividir_scratch(datos, len_codigo)?;
        let codigo = std::str::from_utf8(codigo).map_err(|_| "código UTF-8 inválido")?;
        wm.load_written_drum(codigo, tambor)
    })
}

/// Prepara las filas de una cinta de programa sin alterar núcleos, tambor
/// ni lector. Entrada: código escrito y, si existe, imagen de tambor.
#[no_mangle]
pub extern "C" fn wasm_preparar_cinta_programa(len_codigo: usize) -> u32 {
    operacion_scratch(|wm, datos| {
        let (codigo, imagen) = dividir_scratch(datos, len_codigo)?;
        let codigo = std::str::from_utf8(codigo).map_err(|_| "código UTF-8 inválido")?;
        let programa = asm::assemble(codigo).map_err(|e| e.to_string())?;
        let tambor = if imagen.is_empty() {
            None
        } else {
            Some(tambor_desde_imagen(imagen, wm.m.config.drum_sectors)?)
        };
        let cinta = tape::tele_input::cinta_programa(&programa, tambor.as_ref())?;
        wm.result_buf = cinta.filas;
        wm.cinta_vuelta = cinta.vuelta;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn wasm_preparar_cinta_autocode(len_codigo: usize) -> u32 {
    operacion_scratch(|wm, datos| {
        let (codigo, _) = dividir_scratch(datos, len_codigo)?;
        let codigo = std::str::from_utf8(codigo).map_err(|_| "código UTF-8 inválido")?;
        let img = mercury_autocode::compilar(codigo).map_err(|e| e.to_string())?;
        let mut tambor = Drum::new(Config::default().drum_sectors);
        for (sector, pagina) in img.tambor {
            tambor.sector_mut(sector).copy_from_slice(&pagina);
        }
        let cinta = tape::tele_input::cinta_imagen(&img.cortos, img.arranque, Some(&tambor))?;
        wm.result_buf = cinta.filas;
        wm.cinta_vuelta = cinta.vuelta;
        wm.autocode_header = img.salida_traduccion.into_bytes();
        wm.autocode_data = img.datos.as_deref().map(limpiar_datos_autocode).unwrap_or_default().into_bytes();
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn wasm_inspect_midi(len: usize) -> u32 {
    operacion_scratch(|wm, datos| wm.inspect_midi(dividir_scratch(datos, len)?.0))
}

/// Entrada: MIDI seguido por el guion (UTF-8). -1 indica pista o canal
/// automático y i32::MIN indica transposición automática.
#[no_mangle]
pub extern "C" fn wasm_load_midi(len_midi: usize, pista: i32, canal: i32, transponer: i32, separacion_ms: u32) -> u32 {
    operacion_midi(len_midi, pista, canal, transponer, separacion_ms, false)
}

/// Devuelve el resumen JSON, un byte nulo y las filas de tele-input.
#[no_mangle]
pub extern "C" fn wasm_preparar_cinta_midi(len_midi: usize, pista: i32, canal: i32, transponer: i32, separacion_ms: u32) -> u32 {
    operacion_midi(len_midi, pista, canal, transponer, separacion_ms, true)
}

fn operacion_midi(len_midi: usize, pista: i32, canal: i32, transponer: i32, separacion_ms: u32, como_cinta: bool) -> u32 {
    operacion_scratch(|wm, datos| {
        let (midi, guion) = dividir_scratch(datos, len_midi)?;
        let guion = std::str::from_utf8(guion).map_err(|_| "guion UTF-8 inválido")?;
        let pista = (pista != -1).then(|| usize::try_from(pista)).transpose().map_err(|_| "pista fuera de rango")?;
        let canal = (canal != -1).then(|| u8::try_from(canal)).transpose().map_err(|_| "canal fuera de rango")?;
        let transponer = (transponer != i32::MIN).then_some(transponer);
        if !como_cinta {
            return wm.load_midi(midi, guion, pista, canal, transponer, separacion_ms);
        }
        let (imagen, resumen) = convertir_midi(midi, guion, pista, canal, transponer, separacion_ms)?;
        let mut tambor = Drum::new(Config::default().drum_sectors);
        tambor.cargar_imagen(&imagen)?;
        let programa = asm::assemble(midi::REPRODUCTOR).map_err(|e| e.to_string())?;
        let cinta = tape::tele_input::cinta_programa(&programa, Some(&tambor))?;
        wm.result_buf = resumen.into_bytes();
        wm.result_buf.push(0);
        wm.result_buf.extend(cinta.filas);
        wm.cinta_vuelta = cinta.vuelta;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn wasm_autocode_header_ptr() -> *const u8 {
    with_machine(|wm| wm.autocode_header.as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_autocode_header_len() -> usize {
    with_machine(|wm| wm.autocode_header.len())
}

#[no_mangle]
pub extern "C" fn wasm_autocode_data_ptr() -> *const u8 {
    with_machine(|wm| wm.autocode_data.as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_autocode_data_len() -> usize {
    with_machine(|wm| wm.autocode_data.len())
}

#[no_mangle]
pub extern "C" fn wasm_reset(full_input_routine: u32) {
    with_machine(|wm| wm.reset(full_input_routine != 0));
}

#[no_mangle]
pub extern "C" fn wasm_elegir_instalacion(uba_1966: u32) {
    with_machine(|wm| {
        wm.m.config = if uba_1966 != 0 { Config::clementina_1966() } else { Config::default() };
        wm.reset(false);
    });
}

#[no_mangle]
pub extern "C" fn wasm_get_drum_sectors() -> usize {
    with_machine(|wm| wm.m.drum.len())
}

#[no_mangle]
pub extern "C" fn wasm_get_mitad_alta_cortos() -> u32 {
    with_machine(|wm| u32::from(wm.m.mitad_alta_cortos))
}

#[no_mangle]
pub extern "C" fn wasm_load_written(len: usize) -> u32 {
    with_machine(|wm| match read_scratch_str(wm, len).and_then(|s| wm.load_written(&s)) {
        Ok(()) => 1,
        Err(e) => {
            wm.set_error(e);
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn wasm_load_autocode(len: usize) -> u32 {
    with_machine(|wm| match read_scratch_str(wm, len).and_then(|s| wm.load_autocode(&s)) {
        Ok(()) => 1,
        Err(e) => {
            wm.set_error(e);
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn wasm_load_input_routine_tape(len: usize) -> u32 {
    with_machine(|wm| {
        match read_scratch_str(wm, len).and_then(|s| wm.load_input_routine_tape(&s)) {
            Ok(()) => 1,
            Err(e) => {
                wm.set_error(e);
                0
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn wasm_set_tape_binary(len: usize) {
    with_machine(|wm| {
        let n = len.min(wm.scratch.len());
        let bytes = wm.scratch[..n].to_vec();
        wm.set_tape_binary(&bytes);
    });
}

#[no_mangle]
pub extern "C" fn wasm_set_tape_text(len: usize) -> u32 {
    with_machine(|wm| match read_scratch_str(wm, len).and_then(|s| wm.set_tape_text(&s)) {
        Ok(()) => 1,
        Err(e) => {
            wm.set_error(e);
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn wasm_append_tape_text(len: usize) -> u32 {
    with_machine(|wm| match read_scratch_str(wm, len).and_then(|s| wm.append_tape_text(&s)) {
        Ok(()) => 1,
        Err(e) => {
            wm.set_error(e);
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn wasm_clear_tape() {
    with_machine(|wm| {
        wm.io.input.clear();
        wm.io.input_pos = 0;
    });
}

#[no_mangle]
pub extern "C" fn wasm_clear_output() {
    with_machine(|wm| wm.io.output.clear());
}

#[no_mangle]
pub extern "C" fn wasm_set_switches(f: u32, j: u32, d: u32, flags: u32) {
    with_machine(|wm| wm.set_switches(f as u16, j as u16, d as u16, flags));
}

#[no_mangle]
pub extern "C" fn wasm_initial_transfer() {
    with_machine(|wm| wm.initial_transfer());
}

#[no_mangle]
pub extern "C" fn wasm_invertir_bit_nucleos(corto: u32, bit: u32) -> u32 {
    u32::from(bit <= 10 && with_machine(|wm| wm.m.store.invertir_bit(corto as usize, bit as u8)))
}

#[no_mangle]
pub extern "C" fn wasm_invertir_bit_tambor(sector: u32, corto: u32, bit: u32) -> u32 {
    u32::from(bit <= 10 && with_machine(|wm| wm.m.drum.invertir_bit(sector as usize, corto as usize, bit as u8)))
}

#[no_mangle]
pub extern "C" fn wasm_prepulse() -> u32 {
    with_machine(|wm| wm.prepulse())
}

#[no_mangle]
pub extern "C" fn wasm_step_batch(max_steps: u32, max_us: u32) -> u32 {
    with_machine(|wm| wm.step_batch(max_steps, max_us))
}

/// Cede el control al llegar a la entrada del programa, antes de ejecutarla.
#[no_mangle]
pub extern "C" fn wasm_pasos_hasta_entrada(max_pasos: u32, direccion: u32, filas: u32) -> u32 {
    with_machine(|wm| wm.ejecutar_lote(max_pasos, 0, Some((direccion as u16, filas as usize))))
}

#[no_mangle]
pub extern "C" fn wasm_power_off() {
    with_machine(|wm| wm.power_off());
}

#[no_mangle]
pub extern "C" fn wasm_get_b(idx: u32) -> u32 {
    with_machine(|wm| u32::from(wm.m.b.get(idx as usize).copied().unwrap_or(0)))
}

#[no_mangle]
pub extern "C" fn wasm_get_bt() -> u32 {
    with_machine(|wm| u32::from(wm.m.bt))
}

#[no_mangle]
pub extern "C" fn wasm_get_st() -> u32 {
    with_machine(|wm| u32::from(wm.m.st))
}

#[no_mangle]
pub extern "C" fn wasm_get_c() -> u32 {
    with_machine(|wm| u32::from(wm.m.c))
}

#[no_mangle]
pub extern "C" fn wasm_get_t() -> u32 {
    with_machine(|wm| u32::from(wm.m.t))
}

#[no_mangle]
pub extern "C" fn wasm_get_shift() -> u32 {
    with_machine(|wm| wm.m.shift)
}

#[no_mangle]
pub extern "C" fn wasm_get_acc_f64() -> f64 {
    with_machine(|wm| {
        let v = wm.m.acc.to_f64();
        if v.is_finite() { v } else { 0.0 }
    })
}

#[no_mangle]
pub extern "C" fn wasm_get_acc_long(idx: u32) -> u32 {
    with_machine(|wm| u32::from(wm.m.acc.to_long().get(idx as usize).copied().unwrap_or(0)))
}

#[no_mangle]
pub extern "C" fn wasm_get_display_long(idx: u32) -> u32 {
    with_machine(|wm| {
        u32::from(wm.io.display_long.get(idx as usize).copied().unwrap_or(0))
    })
}

#[no_mangle]
pub extern "C" fn wasm_has_display() -> u32 {
    with_machine(|wm| u32::from(wm.io.has_display))
}

#[no_mangle]
pub extern "C" fn wasm_get_last_instr() -> u32 {
    with_machine(|wm| wm.last_instr)
}

#[no_mangle]
pub extern "C" fn wasm_get_last_fn_dec() -> i32 {
    with_machine(|wm| wm.last_fn_dec)
}

#[no_mangle]
pub extern "C" fn wasm_has_last_instr() -> u32 {
    with_machine(|wm| u32::from(wm.has_last_instr))
}

#[no_mangle]
pub extern "C" fn wasm_get_time_us() -> f64 {
    with_machine(|wm| wm.m.time_us as f64)
}

#[no_mangle]
pub extern "C" fn wasm_get_executed() -> f64 {
    with_machine(|wm| wm.m.executed as f64)
}

#[no_mangle]
pub extern "C" fn wasm_get_stop_code() -> u32 {
    with_machine(|wm| wm.last_stop_code)
}

#[no_mangle]
pub extern "C" fn wasm_get_stop_arg() -> u32 {
    with_machine(|wm| wm.last_stop_arg)
}

#[no_mangle]
pub extern "C" fn wasm_get_stop_flip_flop() -> u32 {
    with_machine(|wm| u32::from(wm.m.stop_flip_flop()))
}

#[no_mangle]
pub extern "C" fn wasm_get_input_pos() -> u32 {
    with_machine(|wm| wm.io.input_pos as u32)
}

#[no_mangle]
pub extern "C" fn wasm_get_input_len() -> u32 {
    with_machine(|wm| wm.io.input.len() as u32)
}

#[no_mangle]
pub extern "C" fn wasm_input_ptr() -> *const u16 {
    with_machine(|wm| wm.io.input.as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_get_output_len() -> u32 {
    with_machine(|wm| wm.io.output.len() as u32)
}

#[no_mangle]
pub extern "C" fn wasm_output_ptr() -> *const u16 {
    with_machine(|wm| wm.io.output.as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_get_batch_core_pages() -> u32 {
    with_machine(|wm| wm.batch_core_pages)
}

#[no_mangle]
pub extern "C" fn wasm_get_last_drum_sector_read() -> i32 {
    with_machine(|wm| wm.last_drum_sector_read)
}

#[no_mangle]
pub extern "C" fn wasm_batch_drum_read_ptr() -> *const u8 {
    with_machine(|wm| wm.batch_drum_read.as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_batch_drum_write_ptr() -> *const u8 {
    with_machine(|wm| wm.batch_drum_write.as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_drum_written_ptr() -> *const u8 {
    with_machine(|wm| wm.drum_written.as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_get_store_short(addr: u32) -> u32 {
    with_machine(|wm| u32::from(wm.m.store.short(addr as usize)))
}

#[no_mangle]
pub extern "C" fn wasm_drain_hoots_len() -> u32 {
    with_machine(|wm| wm.drain_hoots())
}

#[no_mangle]
pub extern "C" fn wasm_drain_hoots_ptr() -> *const f64 {
    with_machine(|wm| wm.hoots_drain.as_ptr())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_perfora_y_toca_en_la_maquina() {
        let mut wm = WasmMachine::new();
        wm.inspect_midi(MIDI_PRUEBA).unwrap();
        let info = std::str::from_utf8(&wm.result_buf).unwrap();
        assert!(info.contains("\"notas\":4"));
        wm.load_midi(MIDI_PRUEBA, "HOLA", Some(0), Some(0), None, 15).unwrap();
        assert_ne!(wm.drum_written[0] & 2, 0);
        assert_eq!(wm.step_batch(1_000_000, 0), STOP_99);
        assert!(tape::decode(&wm.io.output).contains("HOLA"));
        assert!(wm.io.hoots.len() > 100);
    }

    #[test]
    fn imagen_de_tambor_invalida_conserva_el_programa() {
        let mut wm = WasmMachine::new();
        wm.load_midi(MIDI_PRUEBA, "", None, None, None, 15).unwrap();
        let pagina = *wm.m.drum.sector(1);
        let c = wm.m.c;
        let mut imagen = vec![0; 512 * 256];
        *imagen.last_mut().unwrap() = 4;
        assert!(wm.load_written_drum("990 0", &imagen).is_err());
        assert_eq!(wm.m.c, c);
        assert_eq!(wm.m.drum.sector(1), &pagina);
        *imagen.last_mut().unwrap() = 0;
        assert!(wm.load_written_drum("990 0", &imagen).is_ok());
        assert_eq!(wm.m.drum.sector(1), &[0; 128]);
    }

    #[test]
    fn la_maquina_guardada_vuelve_apagada_con_nucleos_y_tambor() {
        let mut wm = WasmMachine::new();
        wm.load_written("990 0").unwrap();
        wm.m.drum.sector_mut(300)[5] = 777;
        wm.m.store.set_short(1500, 333);
        let nucleos = wm.m.store.imagen();
        let tambor = wm.m.drum.imagen();

        let mut otra = WasmMachine::new();
        otra.reset(true);
        otra.m.b[1] = 9;
        assert!(otra.restaurar_maquina(&nucleos, &tambor[..100]).is_err());
        assert_eq!(otra.m.b[1], 9);
        otra.restaurar_maquina(&nucleos, &tambor).unwrap();
        assert_eq!(otra.m.drum.sector(300)[5], 777);
        assert_eq!(otra.m.store.short(1500), 333);
        assert_eq!(otra.m.b[1], 0);
        assert!(!otra.powered);
        assert_ne!(otra.drum_written[300 / 8] & (1 << (300 % 8)), 0);
    }

    #[test]
    fn la_instalacion_de_1966_conserva_y_muestra_los_sectores_altos() {
        let mut wm = WasmMachine::new();
        wm.m.config = Config::clementina_1966();
        let mut imagen = vec![0; 1024 * 256];
        imagen[700 * 256] = 7;
        wm.load_written_drum("670 700\n680 2\n650 0\n610 0\n990 0", &imagen).unwrap();
        assert_eq!(wm.step_batch(10, 0), STOP_99);
        assert_eq!(wm.m.store.page(2)[0], 7);
        assert_ne!(wm.drum_written[700 / 8] & (1 << (700 % 8)), 0);
        assert_ne!(wm.batch_drum_read[700 / 8] & (1 << (700 % 8)), 0);
        assert_ne!(wm.batch_core_pages & (1 << 16), 0);
        wm.load_written_drum("990 0", &imagen[..512 * 256]).unwrap();
        assert_eq!(wm.m.drum.len(), 1024);
        assert_eq!(wm.m.drum.sector(700)[0], 0);
        assert!(!wm.m.mitad_alta_cortos);
    }

    #[test]
    fn fibonacci_escrito() {
        let mut wm = WasmMachine::new();
        wm.load_written(include_str!("../../../examples/fibonacci.mer")).unwrap();
        assert_eq!(wm.step_batch(100_000, 0), STOP_99);
        assert_eq!(
            tape::decode(&wm.io.output),
            "001\n001\n002\n003\n005\n008\n013\n021\n034\n055\n089\n144\n233\n377\n"
        );
    }

    #[test]
    fn autocode_int_y_contour() {
        let mut wm = WasmMachine::new();
        let src_int = format!("a = 1\n{}", include_str!("../../../samples/autocode/int.mer"));
        wm.load_autocode(&src_int).unwrap();
        assert_eq!(wm.step_batch(50_000_000, 0), STOP_99);
        let out_int = tape::decode(&wm.io.output);
        let filas: Vec<&str> = out_int.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(filas.len(), 90);
        assert!(filas.last().unwrap().contains("0.328286"), "{:?}", filas.last());

        let mut wm2 = WasmMachine::new();
        wm2.load_autocode(include_str!("../../../samples/autocode/contour.mer")).unwrap();
        assert_eq!(wm2.step_batch(200_000_000, 0), STOP_INPUT_EXHAUSTED);
        let out_c = tape::decode(&wm2.io.output);
        let lineas: Vec<&str> = out_c.lines().filter(|l| l.starts_with('.')).collect();
        assert_eq!(lineas.len(), 52);
        assert_eq!(lineas[1], ".4 5  6   7          7     6       5       4  3  2 .");
    }

    #[test]
    fn input_routine_y_quickies_reales() {
        let mut wm = WasmMachine::new();
        let programa = "C1P2\nR1\nQ19\nQ12\n300 6\nQ10\n990 0\nE v/1\n";
        wm.load_input_routine_tape(programa).unwrap();
        wm.append_tape_text("2  ").unwrap();
        wm.set_switches(0, 0, 0, 8);
        wm.initial_transfer();
        assert_eq!(wm.step_batch(2_000_000, 0), STOP_99);
        let salida = tape::decode(&wm.io.output);
        assert!(salida.contains("1.41421"), "salida: {salida:?}");
        // Verifica telemetría de escritura y lectura de sectores del Input Routine.
        assert!(wm.drum_written[464 >> 3] & (1 << (464 & 7)) != 0);
    }

    #[test]
    fn controles_7_5_en_vivo() {
        let mut wm = WasmMachine::new();

        // 1) INITIAL TRANSFER sin cinta -> InputExhausted retenido en página 0.
        wm.reset(false);
        wm.set_switches(4, 0, 0, 8);
        wm.initial_transfer();
        assert_eq!(wm.step_batch(1000, 0), STOP_INPUT_EXHAUSTED);
        assert_eq!(wm.m.stop_flip_flop(), false);

        // 2) WRITE CURRENT OFF con cinta mínima de tele-input -> LoopStop en 0.60 (medio 60).
        wm.reset(false);
        wm.set_switches(4, 0, 0, 0); // write_current = false
        let par = |w: u16| [(w >> 5) as u8, (w & 31) as u8];
        let palabras = asm::assemble_sector("0 990 5").unwrap();
        let mut cinta = vec![0, 0, 13, 28, 8, 0, 0, 2];
        cinta.extend(par(palabras[0]));
        cinta.extend(par(palabras[1]));
        cinta.extend([31, 31, 26, 0, 5, 2]);
        wm.set_tape_binary(&cinta);
        wm.initial_transfer();
        assert_eq!(wm.step_batch(100_000, 0), STOP_LOOP);
        assert_eq!(wm.m.c, 60);

        // 3) SINGLE con doble prepulso en 62 y HANDSWITCH STOP + MANUAL.
        wm.load_written("620 7\n620 8\n990 0").unwrap();
        wm.set_switches(0, 0, 64, 8 | 2 | 1); // write | single | handswitch_stop en 1.0 (64)
        assert_eq!(wm.step_batch(10, 0), STOP_SINGLE);
        assert_eq!(wm.prepulse(), STOP_OPTIONAL); // 1er beat de 620 7 en 64 con handswitch_stop
        assert!(wm.io.output.is_empty());
        assert_eq!(wm.prepulse(), STOP_OPTIONAL); // 2do beat completa 620 7 y para tras él
        assert_eq!(wm.io.output, vec![7]);
        assert_eq!(wm.m.stop_flip_flop(), true);

        // Quitar handswitch_stop y pasar a MANUAL: ejecutar 105 42 manual + prepulse.
        let fn_10 = mercury_core::opcodes::encode(10, false).unwrap() as u16;
        let j_105 = (fn_10 << 3) | 5;
        wm.set_switches(0, j_105, 42, 8 | 4); // manual = true
        assert_eq!(wm.prepulse(), STOP_NONE);
        assert_eq!(wm.m.b[5], 42);
    }

    #[test]
    fn clementine_hoots_display_y_power_off() {
        let mut wm = WasmMachine::new();
        wm.load_written(include_str!("../../../examples/clementine.mer")).unwrap();
        assert_eq!(wm.step_batch(5_000_000, 0), STOP_99);
        assert!(wm.drain_hoots() > 3000);
        assert_eq!(wm.drain_hoots(), 0);

        let mut wm2 = WasmMachine::new();
        wm2.load_written("400 v9\n640 v9\n990 0\n+5.5 (9").unwrap();
        assert_eq!(wm2.step_batch(1000, 0), STOP_99);
        assert!(wm2.io.has_display);
        assert!((Float::from_long(wm2.io.display_long).to_f64() - 5.5).abs() < 1e-8);
        let mem_antes = wm2.m.store.medium(64);
        wm2.power_off();
        assert!(!wm2.powered);
        assert_eq!(wm2.m.store.medium(64), mem_antes);
        assert!(!wm2.io.has_display);
    }
}
