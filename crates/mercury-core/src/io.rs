//! Interfaz con los periféricos. El núcleo no hace E/S por sí mismo: todo pasa
//! por este trait, para poder conectarlo a una consola de texto, a una interfaz
//! web (WASM) o a los tests.

/// Periféricos de la máquina.
///
/// Canales (CS 225 A §3.9): el 1 de entrada es el lector de cinta principal
/// (instrucción 60) y el 1 de salida la perforadora principal (62 y 63). Las
/// instrucciones 90–97 pueden usar los canales 1–7; en los canales 4–7 el
/// número `device` elige uno de hasta 1024 dispositivos.
pub trait Io {
    /// Lee el siguiente carácter de un canal de entrada. `None` si no hay más
    /// cinta (la máquina real quedaría esperando; aquí se detiene).
    fn read(&mut self, channel: u8, device: u16) -> Option<u16>;

    /// Envía un carácter (hasta 10 bits) a un canal de salida.
    fn write(&mut self, channel: u8, device: u16, value: u16);

    /// Llaves de mano F del pupitre, leídas por la instrucción 61.
    fn handswitches(&mut self) -> u16 {
        0
    }

    /// Llaves de función J del pupitre (10 bits: 7 de función + 3 de B),
    /// usadas en modo MANUAL (CS 225 A §7.5).
    fn function_keys(&mut self) -> u16 {
        0
    }

    /// Llaves de dirección D del pupitre (10 bits), usadas en la parada opcional
    /// (iv) y en modo MANUAL (CS 225 A §7.5).
    fn address_keys(&mut self) -> u16 {
        0
    }

    /// Llave HANDSWITCH STOP: activa la parada opcional (iv) en el registro medio
    /// indicado por las llaves ADDRESS (CS 225 A §7.5).
    fn handswitch_stop(&mut self) -> bool {
        false
    }

    /// Llave SINGLE/CONTINUOUS: `true` = SINGLE, `false` = CONTINUOUS (§7.5).
    fn single_mode(&mut self) -> bool {
        false
    }

    /// Llave MANUAL/AUTOMATIC: `true` = MANUAL, `false` = AUTO (§7.5).
    fn manual_mode(&mut self) -> bool {
        false
    }

    /// Llave WRITE CURRENT: permite grabar el tambor con la instrucción 69 (§7.5).
    /// Por omisión encendida (`true`).
    fn write_current(&mut self) -> bool {
        true
    }

    /// Llave INHIBIT PARITIES (§7.5). Por omisión se comprueba la paridad.
    fn inhibit_parities(&mut self) -> bool {
        false
    }

    /// Instrucción 58 ("hoot"): un impulso al parlante en el instante `t_us`
    /// (microsegundos de máquina). Repetido a ritmo constante produce una nota.
    fn hoot(&mut self, _t_us: u64) {}

    /// Instrucción 64: muestra un número largo en los monitores del pupitre.
    fn display(&mut self, _long: [u16; 4]) {}

    /// Instrucción 56: punto en la salida gráfica de la Universidad de Manchester.
    fn graph_point(&mut self, _x: u16, _y: u16) {}
}

/// Periféricos nulos: sin cinta, la salida se descarta.
#[derive(Default)]
pub struct NullIo;

impl Io for NullIo {
    fn read(&mut self, _channel: u8, _device: u16) -> Option<u16> {
        None
    }
    fn write(&mut self, _channel: u8, _device: u16, _value: u16) {}
}

/// Periféricos en memoria, útiles para tests y para la consola: una cinta de
/// entrada por canal y lo perforado en cada canal de salida.
#[derive(Debug, Clone)]
pub struct BufferIo {
    pub input: [Vec<u16>; 8],
    pub input_pos: [usize; 8],
    pub output: [Vec<u16>; 8],
    pub switches: u16,
    pub function_keys: u16,
    pub address_keys: u16,
    pub handswitch_stop: bool,
    pub single_mode: bool,
    pub manual_mode: bool,
    pub write_current: bool,
    pub inhibit_parities: bool,
    pub hoots: Vec<u64>,
    pub displays: Vec<[u16; 4]>,
}

impl Default for BufferIo {
    fn default() -> Self {
        Self {
            input: Default::default(),
            input_pos: [0; 8],
            output: Default::default(),
            switches: 0,
            function_keys: 0,
            address_keys: 0,
            handswitch_stop: false,
            single_mode: false,
            manual_mode: false,
            write_current: true,
            inhibit_parities: false,
            hoots: Vec::new(),
            displays: Vec::new(),
        }
    }
}

impl BufferIo {
    pub fn with_tape(chars: &[u16]) -> Self {
        let mut io = Self::default();
        io.input[1] = chars.to_vec();
        io
    }
}

impl Io for BufferIo {
    fn read(&mut self, channel: u8, _device: u16) -> Option<u16> {
        let ch = (channel & 7) as usize;
        let v = *self.input[ch].get(self.input_pos[ch])?;
        self.input_pos[ch] += 1;
        Some(v)
    }

    fn write(&mut self, channel: u8, _device: u16, value: u16) {
        self.output[(channel & 7) as usize].push(value);
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
        self.displays.push(long);
    }
}
