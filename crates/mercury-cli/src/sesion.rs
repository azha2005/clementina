//! Sesión interactiva del pupitre para la consola de texto (`clementina`).

use crate::{disasm, ConsoleIo};
use mercury_core::asm::assemble_at;
use mercury_core::{tape, Machine, Stop};
use std::io::{BufRead, Write};

pub(crate) enum Accion {
    Seguir,
    Salir,
}

pub(crate) fn motivo_parada(stop: Option<Stop>, max: u64) -> String {
    match stop {
        Some(Stop::Stop99) => "parada (99)".to_string(),
        Some(Stop::PrepulseStop { bin_fn }) => format!("parada prepulsable (función {bin_fn:03o})"),
        Some(Stop::Paridad { sector: None, corto }) => format!("parada por paridad (núcleos, corto {corto})"),
        Some(Stop::Paridad { sector: Some(sector), corto }) => format!("parada por paridad (sector {sector}, corto {corto})"),
        Some(Stop::Overflow) => "desborde del exponente del acumulador".to_string(),
        Some(Stop::OptionalStop) => "parada opcional (HANDSWITCH STOP)".to_string(),
        Some(Stop::NonExistentSector { sector }) => format!("sector de tambor inexistente ({sector})"),
        Some(Stop::LoopStop) => "parada por bucle".to_string(),
        Some(Stop::Single) => "paso a paso (SINGLE)".to_string(),
        Some(Stop::Manual) => "modo MANUAL".to_string(),
        Some(Stop::InputExhausted { channel }) => format!("se acabó la cinta del canal {channel}"),
        Some(Stop::Unimplemented { dec }) => format!("instrucción {dec:02} no emulada todavía"),
        None => format!("límite de {max} instrucciones"),
    }
}

/// Lee un valor de 10 bits o dirección media: decimal, `0x...`, `0b...` o `p.l`.
pub(crate) fn parsear_dir(s: &str) -> Result<u16, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("falta un número o dirección p.l".into());
    }
    let v = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).map_err(|_| format!("hexadecimal inválido: {s}"))?
    } else if let Some(bin) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        u16::from_str_radix(bin, 2).map_err(|_| format!("binario inválido: {s}"))?
    } else if let Some((p, l)) = s.split_once('.') {
        let p: u16 = p.parse().map_err(|_| format!("página inválida en '{s}'"))?;
        let l: u16 = l.parse().map_err(|_| format!("línea inválida en '{s}'"))?;
        if p >= 16 || l >= 64 {
            return Err(format!("dirección fuera de rango (0.00 a 15.63): {s}"));
        }
        p * 64 + l
    } else {
        s.parse::<u16>().map_err(|_| format!("número inválido: {s}"))?
    };
    if v > 1023 {
        return Err(format!("valor fuera de 10 bits (0..1023): {s}"));
    }
    Ok(v)
}

fn mostrar_parada<W: Write>(m: &Machine, actual: Stop, salida: &mut W) -> std::io::Result<()> {
    let flip = if m.stop_flip_flop() { "STOP FLIP FLOP" } else { "sin flip-flop" };
    let nota = if matches!(actual, Stop::NonExistentSector { .. } | Stop::LoopStop) {
        " — sólo 'it' o 'salir'"
    } else {
        ""
    };
    writeln!(
        salida,
        "\n── {} en C = {}.{:02} ({}) [{flip}]{nota}",
        motivo_parada(Some(actual), 0),
        m.c / 64,
        m.c % 64,
        disasm(m.store.medium(m.c as usize)),
    )
}

fn mostrar_estado<W: Write>(m: &Machine, io: &ConsoleIo, actual: Stop, salida: &mut W) -> std::io::Result<()> {
    writeln!(
        salida,
        "  C = {}.{:02} ({})  [{}]",
        m.c / 64,
        m.c % 64,
        disasm(m.store.medium(m.c as usize)),
        motivo_parada(Some(actual), 0)
    )?;
    writeln!(
        salida,
        "  A = {}   B1–B7 = {:?}   Bt = {}   St = {}   T = {}",
        m.acc.to_f64(),
        &m.b[1..],
        m.bt,
        m.st,
        m.t
    )?;
    writeln!(
        salida,
        "  STOP FLIP FLOP = {}   F = {}   J = {}   D = {}.{:02} ({})",
        if m.stop_flip_flop() { "ON" } else { "OFF" },
        io.switches,
        io.function_keys,
        io.address_keys / 64,
        io.address_keys % 64,
        io.address_keys
    )?;
    writeln!(
        salida,
        "  HANDSWITCH STOP = {}   SINGLE = {}   MANUAL = {}   WRITE CURRENT = {}   INHIBIT PARITIES = {}",
        if io.handswitch_stop { "ON" } else { "OFF" },
        if io.single_mode { "SINGLE" } else { "CONTINUOUS" },
        if io.manual_mode { "MANUAL" } else { "AUTO" },
        if io.write_current { "ON" } else { "OFF" },
        if io.inhibit_parities { "ON" } else { "OFF" }
    )
}

fn mostrar_ayuda<W: Write>(salida: &mut W) -> std::io::Result<()> {
    writeln!(
        salida,
        "\
Órdenes del pupitre:
  p | prepulse        aprieta PREPULSE (línea vacía también)
  c | continuar       pasa a AUTO/CONTINUOUS y sigue si la parada es prepulsable
  llaves N | f N      pone las llaves F (decimal, 0x..., 0b... o p.l)
  dir N | d N         pone las llaves ADDRESS (D)
  func N | j N        pone las llaves FUNCTION (J)
  parada on|off|DIR   enciende/apaga HANDSWITCH STOP (o la fija en DIR)
  single | continuo   cambia la llave SINGLE/CONTINUOUS
  manual on|off|INST  cambia MANUAL/AUTO o ejecuta una instrucción escrita (p. ej. 101 42)
  escritura on|off    cambia WRITE CURRENT
  inhibir on|off      cambia INHIBIT PARITIES
  fallo nucleos CORTO BIT | fallo tambor SECTOR CORTO BIT
                      invierte un bit de diagnóstico (10 es el bit de paridad)
  probar31 B N        prueba B'b = S + N (mnemónico del manual de la UBA)
  it                  aprieta INITIAL TRANSFER
  cinta TEXTO         agrega texto al lector (acepta \\n)
  estado | reg        muestra C, registros y llaves
  salir | q           termina la ejecución"
    )
}

fn ensamblar_manual(texto: &str) -> Result<(u16, u16), String> {
    let prog = assemble_at(texto, 0).map_err(|e| format!("instrucción inválida: {e}"))?;
    if prog.shorts.len() != 2 || prog.shorts[0].0 != 0 || prog.shorts[1].0 != 1 {
        return Err("se esperaba una sola instrucción de 20 bits (p. ej. 101 42)".into());
    }
    Ok((prog.shorts[0].1, prog.shorts[1].1))
}

/// Pausa la corrida en una parada y atiende las órdenes del operador hasta que
/// la máquina pueda seguir o el operador decida salir.
pub(crate) fn consultar_operador<R: BufRead, W: Write>(
    m: &mut Machine,
    io: &mut ConsoleIo,
    stop: Stop,
    entrada: &mut R,
    salida: &mut W,
    al_perforar: &mut impl FnMut(&ConsoleIo, u64),
) -> Result<Accion, String> {
    let io_err = |e: std::io::Error| format!("error de E/S en la sesión: {e}");
    let mut actual = stop;
    mostrar_parada(m, actual, salida).map_err(io_err)?;

    loop {
        write!(salida, "pupitre> ").map_err(io_err)?;
        salida.flush().map_err(io_err)?;
        let mut linea = String::new();
        let leidos = entrada.read_line(&mut linea).map_err(io_err)?;
        if leidos == 0 {
            return Ok(Accion::Salir);
        }
        let linea = linea.trim();
        let (cmd, resto) = match linea.split_once(char::is_whitespace) {
            Some((c, r)) => (c, r.trim()),
            None => (linea, ""),
        };
        let cmd_min = cmd.to_ascii_lowercase();

        match cmd_min.as_str() {
            "" | "p" | "prepulse" => {
                if matches!(actual, Stop::NonExistentSector { .. } | Stop::LoopStop) {
                    writeln!(
                        salida,
                        "PREPULSE no levanta esta parada (sólo INITIAL TRANSFER, orden 'it')."
                    )
                    .map_err(io_err)?;
                    continue;
                }
                let r = m.prepulse(io);
                al_perforar(io, m.time_us);
                match r {
                    Ok(()) if io.manual_mode => {
                        actual = Stop::Manual;
                        writeln!(salida, "  [manual] instruccion ejecutada (C = {}.{:02})", m.c / 64, m.c % 64)
                            .map_err(io_err)?;
                    }
                    Ok(()) => return Ok(Accion::Seguir),
                    Err(nuevo) => {
                        actual = nuevo;
                        mostrar_parada(m, actual, salida).map_err(io_err)?;
                    }
                }
            }
            "c" | "continuar" => {
                io.single_mode = false;
                io.manual_mode = false;
                if let Some(s) = m.stop() {
                    if matches!(s, Stop::NonExistentSector { .. } | Stop::LoopStop) {
                        writeln!(salida, "No se puede continuar desde esta parada (usar 'it' o 'salir').")
                            .map_err(io_err)?;
                        continue;
                    }
                    if s.is_prepulsable() {
                        let r = m.prepulse(io);
                        al_perforar(io, m.time_us);
                        match r {
                            Ok(()) => return Ok(Accion::Seguir),
                            Err(nuevo) => {
                                actual = nuevo;
                                mostrar_parada(m, actual, salida).map_err(io_err)?;
                                continue;
                            }
                        }
                    }
                    writeln!(salida, "No se puede continuar desde {}.", motivo_parada(Some(s), 0))
                        .map_err(io_err)?;
                } else if matches!(actual, Stop::InputExhausted { .. }) && io.pos >= io.input.len() {
                    writeln!(salida, "Falta cinta de entrada (usar 'cinta TEXTO' o 'salir').").map_err(io_err)?;
                } else {
                    return Ok(Accion::Seguir);
                }
            }
            "llaves" | "f" => match parsear_dir(resto) {
                Ok(v) => {
                    io.switches = v & 0x3ff;
                    writeln!(salida, "  F = {} (0b{:010b})", io.switches, io.switches).map_err(io_err)?;
                }
                Err(e) => writeln!(salida, "error: {e}").map_err(io_err)?,
            },
            "dir" | "d" => match parsear_dir(resto) {
                Ok(v) => {
                    io.address_keys = v & 0x3ff;
                    writeln!(salida, "  D = {}.{:02} ({})", io.address_keys / 64, io.address_keys % 64, io.address_keys)
                        .map_err(io_err)?;
                }
                Err(e) => writeln!(salida, "error: {e}").map_err(io_err)?,
            },
            "func" | "j" => match parsear_dir(resto) {
                Ok(v) => {
                    io.function_keys = v & 0x3ff;
                    writeln!(salida, "  J = {} (0b{:010b})", io.function_keys, io.function_keys).map_err(io_err)?;
                }
                Err(e) => writeln!(salida, "error: {e}").map_err(io_err)?,
            },
            "parada" => match resto.to_ascii_lowercase().as_str() {
                "on" => {
                    io.handswitch_stop = true;
                    writeln!(salida, "  HANDSWITCH STOP = ON en D = {}.{:02}", io.address_keys / 64, io.address_keys % 64)
                        .map_err(io_err)?;
                }
                "off" => {
                    io.handswitch_stop = false;
                    writeln!(salida, "  HANDSWITCH STOP = OFF").map_err(io_err)?;
                }
                _ => match parsear_dir(resto) {
                    Ok(dir) => {
                        io.address_keys = dir & 0x3ff;
                        io.handswitch_stop = true;
                        writeln!(
                            salida,
                            "  HANDSWITCH STOP = ON en D = {}.{:02} ({})",
                            io.address_keys / 64,
                            io.address_keys % 64,
                            io.address_keys
                        )
                        .map_err(io_err)?;
                    }
                    Err(e) => writeln!(salida, "error: {e}").map_err(io_err)?,
                },
            },
            "single" => {
                io.single_mode = true;
                writeln!(salida, "  SINGLE/CONTINUOUS = SINGLE").map_err(io_err)?;
            }
            "continuo" => {
                io.single_mode = false;
                writeln!(salida, "  SINGLE/CONTINUOUS = CONTINUOUS").map_err(io_err)?;
                if matches!(actual, Stop::Single) {
                    return Ok(Accion::Seguir);
                }
            }
            "manual" => match resto.to_ascii_lowercase().as_str() {
                "on" => {
                    io.manual_mode = true;
                    writeln!(salida, "  MANUAL/AUTO = MANUAL").map_err(io_err)?;
                }
                "off" => {
                    io.manual_mode = false;
                    writeln!(salida, "  MANUAL/AUTO = AUTO").map_err(io_err)?;
                }
                "" => writeln!(salida, "uso: manual on|off|<instrucción>").map_err(io_err)?,
                _ => match ensamblar_manual(resto) {
                    Ok((hi, lo)) => {
                        let r = m.manual_instruction(hi, lo, io);
                        al_perforar(io, m.time_us);
                        match r {
                            Ok(()) => {
                                writeln!(salida, "  [manual] ejecutada: {resto}").map_err(io_err)?;
                            }
                            Err(nuevo) => {
                                actual = nuevo;
                                mostrar_parada(m, actual, salida).map_err(io_err)?;
                            }
                        }
                    }
                    Err(e) => writeln!(salida, "error: {e}").map_err(io_err)?,
                },
            },
            "escritura" => match resto.to_ascii_lowercase().as_str() {
                "on" => {
                    io.write_current = true;
                    writeln!(salida, "  WRITE CURRENT = ON").map_err(io_err)?;
                }
                "off" => {
                    io.write_current = false;
                    writeln!(salida, "  WRITE CURRENT = OFF").map_err(io_err)?;
                }
                _ => writeln!(salida, "uso: escritura on|off").map_err(io_err)?,
            },
            "inhibir" => match resto.to_ascii_lowercase().as_str() {
                "on" | "off" => {
                    io.inhibit_parities = resto.eq_ignore_ascii_case("on");
                    writeln!(salida, "  INHIBIT PARITIES = {}", resto.to_ascii_uppercase()).map_err(io_err)?;
                }
                _ => writeln!(salida, "uso: inhibir on|off").map_err(io_err)?,
            },
            "fallo" => {
                let partes = resto.split_whitespace().collect::<Vec<_>>();
                let cambiado = match partes.as_slice() {
                    ["nucleos", corto, bit] => {
                        corto.parse().ok().zip(bit.parse().ok()).is_some_and(|(corto, bit)| {
                            m.store.invertir_bit(corto, bit)
                        })
                    }
                    ["tambor", sector, corto, bit] => {
                        sector.parse().ok().zip(corto.parse().ok()).zip(bit.parse().ok())
                            .is_some_and(|((sector, corto), bit)| m.drum.invertir_bit(sector, corto, bit))
                    }
                    _ => false,
                };
                let mensaje = if cambiado {
                    "  Bit invertido. La paridad se comprobará al leer."
                } else {
                    "uso: fallo nucleos CORTO BIT | fallo tambor SECTOR CORTO BIT (bit de 0 a 10)"
                };
                writeln!(salida, "{mensaje}").map_err(io_err)?;
            }
            "probar31" => {
                let partes = resto.split_whitespace().collect::<Vec<_>>();
                let ejecutada = match partes.as_slice() {
                    [b, n] => b.parse().ok().zip(parsear_dir(n).ok())
                        .is_some_and(|(b, n)| m.probar_31b(b, n)),
                    _ => false,
                };
                if ejecutada {
                    writeln!(salida, "  31b (diagnóstico), resultado de 10 bits = {}", m.bt).map_err(io_err)?;
                } else {
                    writeln!(salida, "uso: probar31 B N (B de 0 a 7, N de 0 a 1023)").map_err(io_err)?;
                }
            }
            "it" => {
                m.initial_transfer_con_paridades(io.inhibit_parities);
                if let Some(nuevo) = m.stop() {
                    actual = nuevo;
                    mostrar_parada(m, actual, salida).map_err(io_err)?;
                    continue;
                }
                writeln!(salida, "  INITIAL TRANSFER ejecutado (C = 0.00)").map_err(io_err)?;
                if !io.single_mode && !io.manual_mode {
                    return Ok(Accion::Seguir);
                }
                actual = if io.manual_mode { Stop::Manual } else { Stop::Single };
            }
            "cinta" => {
                if resto.is_empty() {
                    writeln!(salida, "uso: cinta TEXTO").map_err(io_err)?;
                    continue;
                }
                let texto = resto.replace("\\n", "\n");
                match tape::encode(&texto) {
                    Ok(filas) => {
                        let n = filas.len();
                        io.input.extend(filas);
                        writeln!(salida, "  cinta: +{n} filas en el lector").map_err(io_err)?;
                        if matches!(actual, Stop::InputExhausted { .. }) && !io.single_mode && !io.manual_mode {
                            return Ok(Accion::Seguir);
                        }
                    }
                    Err(ch) => writeln!(salida, "error: el carácter '{ch}' no existe en el código de cinta").map_err(io_err)?,
                }
            }
            "estado" | "reg" => mostrar_estado(m, io, actual, salida).map_err(io_err)?,
            "ayuda" | "h" | "?" => mostrar_ayuda(salida).map_err(io_err)?,
            "salir" | "q" => return Ok(Accion::Salir),
            _ => writeln!(salida, "orden desconocida: '{cmd}' (escribí 'ayuda')").map_err(io_err)?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mercury_core::asm::{assemble, assemble_sector};
    use mercury_core::Config;
    use std::io::Cursor;

    fn correr_sesion(m: &mut Machine, io: &mut ConsoleIo, ordenes: &str) -> (Option<Stop>, String) {
        let mut entrada = Cursor::new(ordenes.as_bytes().to_vec());
        let mut salida = Vec::new();
        let mut stop = None;
        for _ in 0..10_000 {
            if let Err(s) = m.step(io) {
                let mut nada = |_: &ConsoleIo, _: u64| {};
                match consultar_operador(m, io, s, &mut entrada, &mut salida, &mut nada).unwrap() {
                    Accion::Seguir => continue,
                    Accion::Salir => {
                        stop = m.stop().or(Some(s));
                        break;
                    }
                }
            }
        }
        (stop, String::from_utf8(salida).unwrap())
    }

    #[test]
    fn parsea_direcciones_y_literales() {
        assert_eq!(parsear_dir("64").unwrap(), 64);
        assert_eq!(parsear_dir("1.05").unwrap(), 69);
        assert_eq!(parsear_dir("0x1f").unwrap(), 31);
        assert_eq!(parsear_dir("0b1010").unwrap(), 10);
        assert!(parsear_dir("16.0").is_err());
        assert!(parsear_dir("1024").is_err());
    }

    #[test]
    fn cambia_llaves_en_dos_paradas_99_y_sigue_con_prepulse() {
        let prog = assemble(
            "
            @64
            990 0
            610 10
            990 1
            610 10+
            990 2
        ",
        )
        .unwrap();
        let mut m = Machine::new(Config::default());
        prog.load(&mut m.store);
        m.goto(prog.start);
        let mut io = ConsoleIo::default();

        let (stop, _) = correr_sesion(&mut m, &mut io, "llaves 17\np\nllaves 42\np\nsalir\n");
        assert_eq!(stop, Some(Stop::Stop99));
        assert_eq!(m.store.short(20), 17);
        assert_eq!(m.store.short(21), 42);
    }

    #[test]
    fn single_paso_a_paso_parada_opcional_y_manual() {
        let prog = assemble(
            "
            @64
            104 7
            105 9
            106 11
            990 0
        ",
        )
        .unwrap();
        let mut m = Machine::new(Config::default());
        prog.load(&mut m.store);
        m.goto(prog.start);
        let mut io = ConsoleIo {
            single_mode: true,
            ..Default::default()
        };

        let (stop, out) = correr_sesion(
            &mut m,
            &mut io,
            "p\nestado\nparada 65\ncontinuo\nmanual 103 99\np\nsalir\n",
        );
        assert_eq!(stop, Some(Stop::Stop99));
        assert_eq!(m.b[3], 99);
        assert_eq!(m.b[4], 7);
        assert_eq!(m.b[5], 9);
        assert_eq!(m.b[6], 11);
        assert!(out.contains("parada opcional"), "{out}");
    }

    #[test]
    fn it_levanta_bucle_y_cinta_reanuda_entrada() {
        let prog = assemble("@64\n590 64\n").unwrap();
        let mut m = Machine::new(Config::default());
        *m.drum.sector_mut(0) = assemble_sector("0 600 10\n1 990 0").unwrap();
        prog.load(&mut m.store);
        m.goto(prog.start);
        let mut io = ConsoleIo::default();

        let (stop, out) = correr_sesion(&mut m, &mut io, "p\nit\ncinta A\nsalir\n");
        assert_eq!(stop, Some(Stop::Stop99));
        assert!(out.contains("PREPULSE no levanta esta parada"), "{out}");
        assert_eq!(m.store.short(20), tape::encode("A").unwrap()[0]);
    }
}
