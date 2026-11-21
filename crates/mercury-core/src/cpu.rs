//! Unidad de control: búsqueda, decodificación y ejecución de instrucciones.
//!
//! Fuente: CS 225 A cap. 1 y 3 y Apéndice V. Las ambigüedades del manual y las
//! decisiones tomadas están anotadas como `Supuesto:`.

use crate::acc::{self, AddMode, Float, MulMode};
use crate::io::Io;
use crate::opcodes::{self, AddrType};
use crate::store::{signed10, Drum, Store, SHORT_MASK};

/// Motivo por el que la máquina se detuvo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// (i) Instrucción 99. Con el botón *prepulse* continúa en la siguiente.
    Stop99,
    /// (i) Parada prepulsable 1 000 000 o código de función no descrito (§3.13).
    PrepulseStop { bin_fn: u8 },
    /// (ii) Paridad incorrecta en un corto de núcleos o de un sector del tambor.
    Paridad { sector: Option<u16>, corto: u16 },
    /// (iii) El exponente de un resultado superó 255 (§1.7.2). Con *prepulse*
    /// continúa en la instrucción siguiente.
    Overflow,
    /// (iv) Parada opcional: HANDSWITCH STOP encendido al obedecer la
    /// instrucción del registro indicado por las llaves ADDRESS (§7.5).
    OptionalStop,
    /// (v) Transferencia de tambor (68 o 69) con un sector inexistente (§7.5).
    /// Sólo INITIAL TRANSFER la levanta; PREPULSE no tiene efecto.
    NonExistentSector { sector: u16 },
    /// (vi) Parada por bucle: una instrucción transfiere el control a sí misma
    /// sin cambiar el estado que gobierna el salto, p. ej. `590 *` (§7.5).
    /// Sólo INITIAL TRANSFER la levanta; PREPULSE no tiene efecto.
    LoopStop,
    /// (vii) Llave SINGLE/CONTINUOUS en SINGLE: la unidad de control espera
    /// otro PREPULSE (o pasar a CONTINUOUS) (§7.5).
    Single,
    /// Llave MANUAL/AUTOMATIC en MANUAL: las instrucciones automáticas no
    /// avanzan; PREPULSE obedece las llaves FUNCTION y ADDRESS (§7.5).
    Manual,
    /// Se acabó la cinta en un canal de entrada (la máquina real queda retenida).
    InputExhausted { channel: u8 },
    /// Instrucción de equipo especial no emulado todavía (tarjetas, cinta magnética).
    Unimplemented { dec: u8 },
}

impl Stop {
    /// Enciende la lámpara STOP FLIP FLOP del pupitre (CS 225 A §7.5: paradas (i) a (iv)).
    pub fn lights_stop_flip_flop(self) -> bool {
        matches!(
            self,
            Self::Stop99 | Self::PrepulseStop { .. } | Self::Paridad { .. } | Self::Overflow | Self::OptionalStop
        )
    }

    /// Indica si la parada puede levantarse con el botón PREPULSE (§7.5).
    pub fn is_prepulsable(self) -> bool {
        matches!(
            self,
            Self::Stop99
                | Self::PrepulseStop { .. }
                | Self::Paridad { .. }
                | Self::Overflow
                | Self::OptionalStop
                | Self::Single
                | Self::Manual
        )
    }
}

/// Configuración de la instalación.
#[derive(Debug, Clone)]
pub struct Config {
    /// Sectores de tambor. Clementina: 512 en 1962, 1024 en 1966.
    pub drum_sectors: usize,
    /// Órdenes 65 y 66 cableadas por el Instituto de Cálculo para datos cortos.
    pub modificacion_uba: bool,
    /// Tiempo entre caracteres del lector de cinta (µs). 200 car/s → 5 ms (§3.15).
    pub reader_char_us: u64,
    /// Tiempo entre caracteres de la perforadora (µs).
    /// Supuesto: 33⅓ car/s (perforadora tipo Creed), sin confirmar para Clementina.
    pub punch_char_us: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self { drum_sectors: 512, modificacion_uba: false, reader_char_us: 5_000, punch_char_us: 30_000 }
    }
}

impl Config {
    pub fn clementina_1966() -> Self {
        Self { drum_sectors: 1024, modificacion_uba: true, ..Self::default() }
    }
}

/// Período de rotación del tambor: dos sectores por pista, cada uno de 7,68 ms
/// más 960 µs de separación (§3.14, fig. 1).
pub const DRUM_SECTOR_US: u64 = 7_680;
pub const DRUM_GAP_US: u64 = 960;
pub const DRUM_REV_US: u64 = 2 * (DRUM_SECTOR_US + DRUM_GAP_US);

/// Estado completo de la máquina.
#[derive(Clone)]
pub struct Machine {
    pub store: Store,
    pub drum: Drum,
    /// Registros B0–B7 (10 bits). B0 vale siempre cero; B7 es el *sac* (S).
    pub b: [u16; 8],
    /// Registro de prueba B (Bt).
    pub bt: u16,
    /// Registro de prueba del sac (St).
    pub st: u16,
    /// Acumulador.
    pub acc: Float,
    /// Dirección de control C: registro medio de la próxima instrucción.
    pub c: u16,
    /// Sector de tambor seleccionado (T).
    pub t: u16,
    /// Mitad superior seleccionada para operandos cortos en la variante UBA.
    pub mitad_alta_cortos: bool,
    /// Diferencia de exponentes de la última suma o resta (para la instrucción 48).
    pub shift: u32,
    /// Tiempo de máquina transcurrido, en microsegundos.
    pub time_us: u64,
    /// Instrucciones ejecutadas.
    pub executed: u64,
    pub config: Config,
    /// Instante en que cada canal vuelve a estar listo: 0–7 entrada, 8–15 salida.
    ready_at: [u64; 16],
    /// Parada retenida en la unidad de control hasta PREPULSE o INITIAL TRANSFER.
    halted: Option<Stop>,
    /// Dirección media de una instrucción 60, 62 o 63 detenida tras el primer
    /// pulso en modo SINGLE o parada opcional (§7.5: piden dos prepulsos / paran dos veces).
    pending_io_beat: Option<u16>,
    transferencia_inicial_pendiente: bool,
}

impl Machine {
    pub fn new(config: Config) -> Self {
        Self {
            store: Store::default(),
            drum: Drum::new(config.drum_sectors),
            b: [0; 8],
            bt: 0,
            st: 0,
            acc: Float::default(),
            c: 0,
            t: 0,
            mitad_alta_cortos: false,
            shift: 0,
            time_us: 0,
            executed: 0,
            config,
            ready_at: [0; 16],
            halted: None,
            pending_io_beat: None,
            transferencia_inicial_pendiente: false,
        }
    }

    /// Parada actual de la unidad de control, si está detenida.
    pub fn stop(&self) -> Option<Stop> {
        self.halted
    }

    /// Estado de la lámpara STOP FLIP FLOP (§7.5: paradas (i) a (iv)).
    pub fn stop_flip_flop(&self) -> bool {
        self.halted.is_some_and(Stop::lights_stop_flip_flop)
    }

    /// Posiciona el contador de control `C` y levanta cualquier parada retenida.
    pub fn goto(&mut self, addr: u16) {
        self.c = addr & 0x3ff;
        self.halted = None;
        self.pending_io_beat = None;
        self.transferencia_inicial_pendiente = false;
    }

    /// Sac (= B7).
    pub fn sac(&self) -> u16 {
        self.b[7]
    }

    /// Dirección efectiva de un operando corto. `bmod` ya tiene aplicada la
    /// regla que distingue un registro operado de un modificador de dirección.
    pub fn direccion_corta(&self, dc: opcodes::Decoded, n: u16, bmod: u16) -> Option<usize> {
        let tipo = opcodes::addr_type(dc.dec);
        if !matches!(tipo, AddrType::Short | AddrType::ShortFirstQuarter) {
            return None;
        }
        let cuarto = if dc.second_quarter { 1024 } else { 0 };
        let direccion = (cuarto + i32::from(n) + signed10(bmod)).rem_euclid(2048) as usize;
        // Del Dago (2014), tabla 1. Supuesto: B modifica dentro de la mitad
        // elegida. 63 conserva su direccionamiento por faltar en esa tabla.
        let alta = self.config.modificacion_uba && self.mitad_alta_cortos && tipo == AddrType::Short;
        Some(direccion + if alta { 2048 } else { 0 })
    }

    /// Ejecuta la operación de 31b descrita en el manual de la UBA de 1962,
    /// §4.2. Entrada de diagnóstico por mnemónico, al faltar el código de
    /// función y su tiempo. No consume una instrucción ni cambia C.
    pub fn probar_31b(&mut self, b: u8, n: u16) -> bool {
        if b > 7 || n > SHORT_MASK {
            return false;
        }
        let suma = self.sac().wrapping_add(n) & SHORT_MASK;
        self.set_reg(b as usize, suma);
        self.bt = suma;
        true
    }

    fn reg(&self, r: usize) -> u16 {
        if r == 0 {
            0
        } else {
            self.b[r]
        }
    }

    fn set_reg(&mut self, r: usize, v: u16) {
        if r != 0 {
            self.b[r] = v & SHORT_MASK;
        }
    }

    /// Ejecuta la instrucción en C (salvo que la máquina esté detenida o en SINGLE/MANUAL).
    pub fn step(&mut self, io: &mut dyn Io) -> Result<(), Stop> {
        if let Some(s) = self.halted {
            match s {
                Stop::Single if !io.single_mode() => self.halted = None,
                Stop::Manual if !io.manual_mode() => self.halted = None,
                _ => return Err(s),
            }
        }
        if io.manual_mode() {
            self.halted = Some(Stop::Manual);
            return Err(Stop::Manual);
        }
        if io.single_mode() {
            self.halted = Some(Stop::Single);
            return Err(Stop::Single);
        }
        self.step_once(io)
    }

    /// Botón PREPULSE (CS 225 A §7.5): en MANUAL obedece las llaves FUNCTION y ADDRESS;
    /// tras una parada prepulsable (i–iv, vii) ejecuta un paso y sigue.
    /// No levanta paradas por sector inexistente (v) ni por bucle (vi).
    pub fn prepulse(&mut self, io: &mut dyn Io) -> Result<(), Stop> {
        if io.manual_mode() {
            let f = io.function_keys();
            let a = io.address_keys();
            return self.manual_instruction(f, a, io);
        }
        if let Some(s) = self.halted {
            if !s.is_prepulsable() {
                return Err(s);
            }
            self.halted = None;
        }
        if self.transferencia_inicial_pendiente {
            self.initial_transfer_con_paridades(io.inhibit_parities());
            return self.halted.map_or(Ok(()), Err);
        }
        self.step_once(io)
    }

    fn step_once(&mut self, io: &mut dyn Io) -> Result<(), Stop> {
        let here = self.c & 0x3ff;
        if !io.inhibit_parities() {
            if let Err(stop) = self.verificar_nucleos(here as usize * 2, 2) {
                self.halted = Some(stop);
                return Err(stop);
            }
        }
        let word = self.store.medium(here as usize);
        let bin_fn = ((word >> 13) & 0x7f) as u8;
        let bd = ((word >> 10) & 7) as usize;
        let dec = opcodes::decode(bin_fn).map(|d| d.dec);
        let optional_match = io.handswitch_stop() && here == (io.address_keys() & 0x3ff);
        let single = io.single_mode();

        // Excepción §7.5: la parada opcional sobre 68 o 69 detiene sin transferir el tambor.
        if optional_match && matches!(dec, Some(68 | 69)) {
            self.pending_io_beat = None;
            self.c = (here + 1) & 0x3ff;
            self.executed += 1;
            // Supuesto: el ciclo de control consume 60 µs sin transferencia de tambor.
            self.time_us += 60;
            self.halted = Some(Stop::OptionalStop);
            return Err(Stop::OptionalStop);
        }

        // Excepción §7.5: en SINGLE o con parada opcional, 60, 62 y 63 requieren dos
        // prepulsos / paran dos veces. Supuesto: la primera parada ocurre antes de
        // completar la operación de E/S y la segunda al completarla.
        if matches!(dec, Some(60 | 62 | 63)) && (optional_match || single) && self.pending_io_beat != Some(here) {
            self.pending_io_beat = Some(here);
            let stop = if optional_match { Stop::OptionalStop } else { Stop::Single };
            self.halted = Some(stop);
            return Err(stop);
        }

        let was_second_beat = self.pending_io_beat == Some(here);
        self.pending_io_beat = None;
        self.c = (here + 1) & 0x3ff;

        match self.execute(word, io) {
            Ok(()) => {
                // Parada por bucle (vi, §7.5): un salto hacia la propia instrucción que no
                // cambia el estado que gobierna la condición. 380 * y 18b * (b != 0) modifican
                // S o B y el Input Routine los usa como espera en varios sectores.
                let pure_jump = matches!(dec, Some(8 | 9 | 28 | 29 | 48 | 49 | 59))
                    || (dec == Some(18) && bd == 0);
                if pure_jump && self.c == here {
                    self.halted = Some(Stop::LoopStop);
                    return Err(Stop::LoopStop);
                }
                if optional_match {
                    self.halted = Some(Stop::OptionalStop);
                    return Err(Stop::OptionalStop);
                }
                if single {
                    self.halted = Some(Stop::Single);
                    return Err(Stop::Single);
                }
                Ok(())
            }
            Err(Stop::InputExhausted { channel }) => {
                // La máquina real queda retenida en la instrucción de entrada.
                self.c = here;
                self.executed = self.executed.saturating_sub(1);
                if was_second_beat {
                    self.pending_io_beat = Some(here);
                }
                Err(Stop::InputExhausted { channel })
            }
            Err(s @ (Stop::NonExistentSector { .. } | Stop::Unimplemented { .. })) => {
                self.c = here;
                self.executed = self.executed.saturating_sub(1);
                self.halted = Some(s);
                Err(s)
            }
            Err(s @ Stop::Paridad { .. }) => {
                // Supuesto: el acceso fallido se vuelve a intentar con PREPULSE.
                self.c = here;
                if was_second_beat {
                    self.pending_io_beat = Some(here);
                }
                self.halted = Some(s);
                Err(s)
            }
            Err(s) => {
                self.halted = Some(s);
                Err(s)
            }
        }
    }

    /// Ejecuta hasta que la máquina se detenga o se cumpla el límite.
    pub fn run(&mut self, io: &mut dyn Io, max_instructions: u64) -> Option<Stop> {
        for _ in 0..max_instructions {
            if let Err(s) = self.step(io) {
                return Some(s);
            }
        }
        None
    }

    /// Botón *initial transfer* (CS 225 A §7.4): copia el sector 0 del tambor
    /// a la página 0, levanta cualquier parada y pasa el control al registro 0,
    /// donde el *Input Routine* empieza leyendo las llaves de mano.
    pub fn initial_transfer(&mut self) {
        self.initial_transfer_con_paridades(false);
    }

    pub fn initial_transfer_con_paridades(&mut self, inhibir: bool) {
        self.c = 0;
        // Durán (2018, pág. 17): un programa arranca en la mitad baja.
        // Supuesto: INITIAL TRANSFER restablece esa selección.
        self.mitad_alta_cortos = false;
        self.pending_io_beat = None;
        self.halted = if self.drum.is_empty() {
            Some(Stop::NonExistentSector { sector: 0 })
        } else if inhibir {
            None
        } else {
            self.verificar_tambor(0).err()
        };
        self.transferencia_inicial_pendiente = matches!(self.halted, Some(Stop::Paridad { .. }));
        if self.halted.is_none() {
            self.store.page_mut(0).copy_from_slice(self.drum.sector(0));
        }
    }

    /// Instrucción manual (§7.5, llave manual/automático en "manual"): obedece
    /// la palabra de 20 bits armada en las llaves de función y de dirección.
    pub fn manual_instruction(&mut self, function_keys: u16, address_keys: u16, io: &mut dyn Io) -> Result<(), Stop> {
        self.halted = None;
        self.pending_io_beat = None;
        self.transferencia_inicial_pendiente = false;
        let word = (((function_keys & SHORT_MASK) as u32) << 10) | (address_keys & SHORT_MASK) as u32;
        let r = self.execute(word, io);
        if let Err(s) = r {
            if !matches!(s, Stop::InputExhausted { .. }) {
                self.halted = Some(s);
            }
        }
        r
    }

    fn wait_device(&mut self, slot: usize, period: u64) {
        if self.time_us < self.ready_at[slot] {
            self.time_us = self.ready_at[slot];
        }
        self.ready_at[slot] = self.time_us + period;
    }

    fn drum_transfer_time(&self, sector: u16) -> u64 {
        let start = (sector as u64 & 1) * (DRUM_SECTOR_US + DRUM_GAP_US);
        let pos = self.time_us % DRUM_REV_US;
        let wait = (start + DRUM_REV_US - pos) % DRUM_REV_US;
        wait + DRUM_SECTOR_US
    }

    fn verificar_nucleos(&self, inicio: usize, len: usize) -> Result<(), Stop> {
        self.store.fallo_paridad(inicio, len).map_or(Ok(()), |corto| {
            Err(Stop::Paridad { sector: None, corto: corto as u16 })
        })
    }

    fn verificar_tambor(&self, sector: u16) -> Result<(), Stop> {
        self.drum.fallo_paridad(sector as usize).map_or(Ok(()), |corto| {
            Err(Stop::Paridad { sector: Some(sector), corto: corto as u16 })
        })
    }

    /// Operaciones comunes a los registros B y al sac (códigos x0–x7):
    /// 0 copia, 2 suma, 3 resta, 4 mitad sin signo menos, 5 "y", 6 no-equivalencia,
    /// 7 sólo prueba. Todas módulo 1024.
    fn b_op(&mut self, op: u8, r: usize, operand: u16, sac: bool) {
        let cur = if sac { self.b[7] } else { self.reg(r) };
        let res = match op {
            0 => operand,
            2 => cur.wrapping_add(operand),
            3 | 7 => cur.wrapping_sub(operand),
            4 => (cur >> 1).wrapping_sub(operand),
            5 => cur & operand,
            6 => cur ^ operand,
            _ => unreachable!("b_op {op}"),
        } & SHORT_MASK;
        if sac {
            if op != 7 {
                self.b[7] = res;
            }
            self.st = res;
        } else {
            if op != 7 {
                self.set_reg(r, res);
            }
            self.bt = res;
        }
    }

    fn jump_if(&mut self, cond: bool, target: u16) {
        if cond {
            self.c = target & 0x3ff;
        }
    }

    /// Ejecuta una palabra de instrucción de 20 bits.
    pub fn execute(&mut self, word: u32, io: &mut dyn Io) -> Result<(), Stop> {
        let bin_fn = ((word >> 13) & 0x7f) as u8;
        let bd = ((word >> 10) & 7) as usize;
        let n = (word & 0x3ff) as u16;
        let Some(dc) = opcodes::decode(bin_fn) else {
            self.executed += 1;
            self.time_us += 60;
            return Err(Stop::PrepulseStop { bin_fn });
        };
        let dec = dc.dec;

        let bmod = if opcodes::is_b_modifiable(dec) && bd != 0 { self.b[bd] } else { 0 };
        // Dirección modificada para registros medios y largos (módulo 1024, §1.8).
        let ma = (n.wrapping_add(bmod)) & 0x3ff;
        let sa = self.direccion_corta(dc, n, bmod).unwrap_or(0);

        // CS 225 A §1.9. Supuesto: un fallo cancela la transferencia de página
        // entera; la etapa exacta en la que para no está documentada aquí.
        let grabar = dec == 69 && io.write_current();
        if !io.inhibit_parities() {
            match dec {
                0 | 2..=7 | 20 | 22..=27 | 63 => self.verificar_nucleos(sa, 1)?,
                40 | 42..=47 | 50..=56 | 64 => self.verificar_nucleos(ma as usize * 4, 4)?,
                68 => self.verificar_tambor(self.t)?,
                69 if grabar && (self.t as usize) < self.drum.len() => {
                    self.verificar_nucleos((ma as usize & 31) * 128, 128)?;
                }
                _ => {}
            }
        }
        self.executed += 1;

        let mut t = 60u64;
        match dec {
            // --- Instrucciones B (no B-modificadas; el dígito B nombra el registro) ---
            0 | 2..=7 => {
                let h = self.store.short(sa);
                self.b_op(dec, bd, h, false);
            }
            1 => self.store.set_short(sa, self.reg(bd)),
            8 => self.jump_if(self.bt != 0, ma),
            9 => self.jump_if(self.bt & 0x200 == 0, ma),
            10 | 12..=17 => self.b_op(dec - 10, bd, n, false),
            18 => {
                let tested = self.bt;
                let v = (self.reg(bd) + 1) & SHORT_MASK;
                self.set_reg(bd, v);
                self.bt = v;
                self.jump_if(tested != 0, n);
            }

            // --- Instrucciones del sac (B7); el dígito B modifica la dirección ---
            20 | 22..=27 => {
                let h = self.store.short(sa);
                self.b_op(dec - 20, 7, h, true);
            }
            21 => self.store.set_short(sa, self.b[7]),
            // Supuesto: 28 es "St ≠ 0" como en la tabla del Apéndice V
            // (la pág. 15 dice "St = 0", que no concuerda con el resto).
            28 => self.jump_if(self.st != 0, ma),
            29 => self.jump_if(self.st & 0x200 == 0, ma),
            // Supuesto: 36 es "S ≠ n" como en el Apéndice V (la pág. 15 dice "S/2 ≠ n").
            30 | 32..=37 => self.b_op(dec - 30, 7, ma, true),
            38 => {
                let tested = self.st;
                let v = (self.b[7] + 1) & SHORT_MASK;
                self.b[7] = v;
                self.st = v;
                self.jump_if(tested != 0, ma);
            }

            // --- Acumulador ---
            40 => {
                self.acc = Float::from_long(self.store.long(ma as usize));
                t = 120;
            }
            41 => {
                self.store.set_long(ma as usize, self.acc.to_long());
                t = 120;
            }
            42..=47 => {
                t = 180;
                let l = Float::from_long(self.store.long(ma as usize));
                let mode = match dec {
                    42 | 43 => AddMode::Rounded,
                    44 | 45 => AddMode::Unrounded,
                    _ => AddMode::Unstandardised,
                };
                let (r, shift) = match acc::add(self.acc, l, dec % 2 == 1, mode) {
                    Ok(v) => v,
                    Err(_) => {
                        self.time_us += t;
                        return Err(Stop::Overflow);
                    }
                };
                self.acc = r;
                self.shift = shift;
            }
            48 => self.jump_if(self.shift <= 31, ma),
            49 => self.jump_if(self.acc.frac >= 0, ma),
            50..=55 => {
                t = 300;
                let l = Float::from_long(self.store.long(ma as usize));
                let mode = match dec {
                    50 | 51 => MulMode::Rounded,
                    52 | 53 => MulMode::Unrounded,
                    _ => MulMode::LeastSignificant,
                };
                self.acc = match acc::mul(self.acc, l, dec % 2 == 1, mode) {
                    Ok(v) => v,
                    Err(_) => {
                        self.time_us += t;
                        return Err(Stop::Overflow);
                    }
                };
            }
            56 => {
                let l = self.store.long(ma as usize);
                io.graph_point(l[2] % 256, l[3] % 256);
                t = 120;
            }
            57 => {}
            58 => io.hoot(self.time_us),
            59 => self.c = ma,

            // --- Entrada/salida de cinta de papel y pupitre ---
            60 => {
                let ch = io.read(1, 0).ok_or(Stop::InputExhausted { channel: 1 })?;
                self.wait_device(1, self.config.reader_char_us);
                self.store.set_short(sa, ch & 0x1f);
                t = 120;
            }
            61 => self.store.set_short(sa, io.handswitches()),
            62 => {
                self.wait_device(8 + 1, self.config.punch_char_us);
                // B-modificable: el programa de contornos de H. Whitfield
                // (samples/autocode/contour.mer) elige el carácter con 625.
                io.write(1, 0, ma & 0x1f);
                t = 120;
            }
            63 => {
                self.wait_device(8 + 1, self.config.punch_char_us);
                io.write(1, 0, self.store.short(sa) & 0x1f);
                t = 120;
            }
            64 => {
                io.display(self.store.long(ma as usize));
                t = 120;
            }
            65 | 66 => {
                if self.config.modificacion_uba {
                    self.mitad_alta_cortos = dec == 65;
                }
                // En el manual general son órdenes del obturador gráfico.
                // Supuesto: la variante UBA conserva el ciclo de 60 µs.
            }

            // --- Tambor ---
            67 => self.t = ma,
            68 | 69 => {
                let page = ma as usize & 31;
                let sector = self.t as usize;
                if dec == 68 {
                    let src = *self.drum.get(sector).ok_or(Stop::NonExistentSector { sector: self.t })?;
                    self.store.page_mut(page).copy_from_slice(&src);
                } else {
                    if self.drum.get(sector).is_none() {
                        return Err(Stop::NonExistentSector { sector: self.t });
                    }
                    if grabar {
                        self.drum.sector_mut(sector).copy_from_slice(self.store.page(page));
                    }
                    // Supuesto: con WRITE CURRENT en OFF (§7.5) la transferencia 69
                    // tarda lo mismo pero el sector queda intacto.
                }
                t = self.drum_transfer_time(self.t);
            }

            // --- Exponente del acumulador ---
            70 | 72..=77 => {
                let operand = (self.acc.exp_bits() + n) & SHORT_MASK;
                self.b_op(dec - 70, bd, operand, false);
            }
            71 => self.acc.exp = signed10(self.reg(bd)),
            78 => {
                t = 180;
                self.acc = match acc::destandardise(self.acc, signed10(ma)) {
                    Ok(v) => v,
                    Err(_) => {
                        self.time_us += t;
                        return Err(Stop::Overflow);
                    }
                };
            }

            // --- Tarjetas, impresora de líneas y cinta magnética (no emulados aún) ---
            80..=89 => return Err(Stop::Unimplemented { dec }),

            // --- E/S multicanal hacia el sac (§3.9) ---
            90..=97 => {
                let ch = bd as u8;
                let operand = if dec == 91 {
                    0
                } else if ch == 0 {
                    (self.acc.exp_bits() + n) & SHORT_MASK
                } else {
                    let device = if ch >= 4 { n } else { 0 };
                    let i = io.read(ch, device).ok_or(Stop::InputExhausted { channel: ch })?;
                    self.wait_device(bd, self.config.reader_char_us);
                    if ch <= 3 {
                        (i + n) & SHORT_MASK
                    } else {
                        i & SHORT_MASK
                    }
                };
                match dec {
                    90 => self.b_op(0, 7, operand, true),
                    91 => {
                        let s = self.b[7];
                        match ch {
                            0 => self.acc.exp = signed10((s + n) & SHORT_MASK),
                            1..=3 => {
                                self.wait_device(8 + bd, self.config.punch_char_us);
                                io.write(ch, 0, (s + n) & SHORT_MASK);
                            }
                            _ => {
                                self.wait_device(8 + bd, self.config.punch_char_us);
                                io.write(ch, n, s);
                            }
                        }
                    }
                    _ => self.b_op(dec - 90, 7, operand, true),
                }
                if ch != 0 {
                    t = 120;
                }
            }

            99 => {
                self.time_us += t;
                return Err(Stop::Stop99);
            }
            _ => return Err(Stop::Unimplemented { dec }),
        }
        self.time_us += t;
        Ok(())
    }
}
