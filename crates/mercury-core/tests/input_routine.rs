//! El Input Routine original (1959), transcripto del manuscrito, corriendo
//! en el emulador.

use mercury_core::asm::assemble_sector;
use mercury_core::io::BufferIo;
use mercury_core::{Config, Machine, Stop};

fn sector(n: usize) -> [u16; 128] {
    if (13..=63).contains(&n) && n != 38 {
        let archivo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../input_routine/enlazados")
            .join(format!("sector{n:02}.txt"));
        let src = std::fs::read_to_string(&archivo)
            .unwrap_or_else(|e| panic!("{}: {e}", archivo.display()));
        return assemble_sector(&src).unwrap_or_else(|e| panic!("sector {n}: {e}"));
    }
    let src = match n {
        0 => include_str!("../../../input_routine/sector00.txt"),
        1 => include_str!("../../../input_routine/sector01.txt"),
        2 => include_str!("../../../input_routine/sector02.txt"),
        3 => include_str!("../../../input_routine/sector03.txt"),
        4 => include_str!("../../../input_routine/sector04.txt"),
        5 => include_str!("../../../input_routine/sector05.txt"),
        6 => include_str!("../../../input_routine/sector06.txt"),
        7 => include_str!("../../../input_routine/sector07.txt"),
        8 => include_str!("../../../input_routine/sector08.txt"),
        9 => include_str!("../../../input_routine/sector09.txt"),
        10 => include_str!("../../../input_routine/sector10.txt"),
        11 => include_str!("../../../input_routine/sector11.txt"),
        12 => include_str!("../../../input_routine/sector12.txt"),
        38 => include_str!("../../../input_routine/sector38.txt"),
        _ => panic!("sector {n} sin transcribir"),
    };
    assemble_sector(src).unwrap_or_else(|e| panic!("sector {n}: {e}"))
}

fn maquina(sectores: &[usize]) -> Machine {
    let mut m = Machine::new(Config::default());
    for &s in sectores {
        *m.drum.sector_mut(s) = sector(s);
    }
    m
}

#[test]
fn ensamblan_los_sectores_numerados() {
    // Los últimos dos sectores están libres según la tabla de R67.
    for n in 0..=61 {
        assert!(sector(n).iter().any(|&corto| corto != 0), "sector {n}");
    }
    for n in [62, 63] {
        assert_eq!(sector(n), [0; 128]);
    }
}

#[test]
fn una_cinta_traduce_dos_quickies_y_provee_los_datos_del_programa() {
    let sectores: Vec<_> = (0..=63).collect();
    let mut m = maquina(&sectores);
    let texto = "C1P2\nR1\nQ19\nQ1\n990 5\nE v/1\n4  ";
    let cinta = mercury_core::tape::encode(texto).unwrap();
    let mut io = BufferIo::with_tape(&cinta);
    m.initial_transfer();
    assert_eq!(m.run(&mut io, 150_000), Some(Stop::Stop99));
    assert_eq!(m.store.medium((m.c - 1) as usize), 5);
    assert_eq!(io.input_pos[1], cinta.len());
    assert!((m.acc.to_f64() - 0.25).abs() < 1e-8, "{:?}", m.acc);
}

#[test]
fn el_input_routine_traduce_y_ejecuta_una_cinta_con_etiquetas() {
    let sectores: Vec<_> = (0..=46).collect();
    let mut m = maquina(&sectores);
    let texto = "C1P2\nR1\n590 v2\n990 1\n400 v3 (2\n620 7\n990 5\n+3.25 (3\nE v/1\n";
    let cinta = mercury_core::tape::encode(texto).unwrap();
    let mut io = BufferIo::with_tape(&cinta);
    m.initial_transfer();
    assert_eq!(m.run(&mut io, 100_000), Some(Stop::Stop99));
    assert_eq!(m.c, 2 * 64 + 5);
    assert_eq!(m.store.short(2 * (2 * 64 + 4) + 1), 5);
    assert_eq!(io.output[1].last(), Some(&7));
    assert!((m.acc.to_f64() - 3.25).abs() < 1e-8, "{:?}", m.acc);
}

#[test]
fn el_input_routine_arma_un_salto_entre_capitulos() {
    let sectores: Vec<_> = (0..=46).collect();
    let mut m = maquina(&sectores);
    let texto = "C1P2\nR1\nA v/2\nC2P3\nR2\n620 7\n990 5\nE v/1\n";
    let cinta = mercury_core::tape::encode(texto).unwrap();
    let mut io = BufferIo::with_tape(&cinta);
    m.initial_transfer();
    assert_eq!(m.run(&mut io, 100_000), Some(Stop::Stop99));
    assert_eq!(m.c, 3 * 64 + 2);
    assert_eq!(io.output[1].last(), Some(&7));
}

#[test]
fn el_input_routine_inserta_y_reubica_quickies_desde_el_tambor() {
    use mercury_core::{acc::Float, asm, tape};

    let sectores: Vec<_> = (0..=63).collect();
    let casos = [
        (1, 4.0, 0.25, "", ""),
        (2, 4.0, 0.5, "", ""),
        (4, 1.0, 1.0_f64.exp(), "", ""),
        (5, 0.5, 0.5_f64.tan(), "", ""),
        (6, 0.5, 0.5_f64.sin(), "", ""),
        (7, 0.5, 0.5_f64.cos(), "", ""),
        (8, 0.0, 0.0, "", "\n-42  "),
        (9, 12.34, 12.34, "", "\n   12.34  "),
        (10, 12.34, 12.34, "", "\n 1.23400, +1  "),
        (11, 0.0, 0.0, "", "\n982  "),
        (12, 4.0, 2.0, "", ""),
        (14, 2.0, 2.0_f64.ln(), "", ""),
        (15, 0.0, 3.0_f64.atan2(2.0), "", ""),
        (16, 0.5, 0.5_f64.asin(), "", ""),
        (18, 0.0, 0.0, "-42 ", ""),
        (19, 0.0, 0.0125, "1.25,-2  ", ""),
    ];

    for (numero, argumento, esperado, datos, impresion) in casos {
        let archivo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../input_routine/quickies/{numero:02}.mer"));
        let fuente = std::fs::read_to_string(archivo).unwrap();
        let aislada = asm::assemble(&fuente).unwrap();
        let descartados = usize::from(fuente.trim_end().ends_with("; salida de prueba")) * 2;
        let fin = aislada.shorts.last().unwrap().0 + 1 - descartados;
        for pagina in [2, 8] {
            let mut m = maquina(&sectores);
            let texto = format!("C1P{pagina}\nR1\nQ{numero}\n990 5\nE v/1\n");
            let mut io = BufferIo::with_tape(&tape::encode(&texto).unwrap());
            m.initial_transfer();
            let entrada = pagina * 64;
            let salida = entrada + (fin - 256).div_ceil(2) as u16;
            for _ in 0..150_000 {
                if m.c == entrada && m.store.medium(salida as usize) == 5 {
                    break;
                }
                m.step(&mut io).unwrap_or_else(|e| panic!("Q{numero} en p. {pagina}, C={}: {e:?}", m.c));
            }
            assert_eq!(m.c, entrada, "Q{numero} en p. {pagina}");
            assert_eq!(m.store.medium(salida as usize), 5, "Q{numero}: 990 5 de la cinta");
            if pagina == 2 {
                for &(i, palabra) in aislada.shorts.iter().filter(|(i, _)| *i < fin) {
                    assert_eq!(m.store.short(i), palabra, "Q{numero}, corto {i}");
                }
            }

            // Carga los argumentos en el punto de entrada del programa,
            // una vez que el cargador terminó de traducir y reubicar la cinta.
            let mut io = BufferIo::with_tape(&tape::encode(datos).unwrap());
            m.acc = Float::from_f64(argumento);
            m.store.set_long(16, Float::from_f64(3.0).to_long());
            m.store.set_long(17, Float::from_f64(2.0).to_long());
            m.store.set_short(516, 4);
            m.store.set_short(517, 2);
            m.b[1] = 256;
            m.b[5] = 173;
            m.b[6] = 419;
            m.b[7] = if matches!(numero, 8 | 11) { 982 } else { 6 };
            assert_eq!(m.run(&mut io, 100_000), Some(Stop::Stop99), "Q{numero} en p. {pagina}");
            assert_eq!(m.c, salida + 1, "Q{numero} en p. {pagina}");
            assert_eq!(tape::decode(&io.output[1]), impresion, "Q{numero} en p. {pagina}");
            if numero == 18 {
                assert_eq!(m.b[7], 982);
            } else if impresion.is_empty() {
                let obtenido = m.acc.to_f64();
                assert!((obtenido - esperado).abs() < 1e-6, "Q{numero} en p. {pagina}: {obtenido} en vez de {esperado}");
            }
        }
    }
}

#[test]
fn arranca_con_y_sin_borrado_hasta_la_entrada_del_capitulo_1() {
    for (llaves, borrar) in [(0, true), (1, false)] {
        let mut m = maquina(&[0, 2, 12, 38]);
        for n in 13..=25 {
            *m.drum.sector_mut(n) = datos_de_prueba(n as u16);
        }
        for n in [64, 127, 128, 511] {
            *m.drum.sector_mut(n) = datos_de_prueba(1);
        }
        m.store.page_mut(31).fill(1023);
        m.initial_transfer();
        let mut io = BufferIo { switches: llaves, ..Default::default() };
        for _ in 0..10_000 {
            if m.c == 2 * 64 + 26 {
                break;
            }
            m.step(&mut io).unwrap_or_else(|e| panic!("arranque, C={}: {e:?}", m.c));
        }
        assert_eq!(m.c, 2 * 64 + 26);
        for p in 2..=14 {
            assert_eq!(m.store.page(p), &datos_de_prueba((p + 11) as u16));
        }
        assert_eq!(m.store.page(31), &[0; 128]);
        for n in [64, 127, 128, 511] {
            let esperado = if borrar { [0; 128] } else { datos_de_prueba(1) };
            assert_eq!(m.drum.sector(n), &esperado, "sector {n}");
        }
    }
}

#[test]
fn perfora_el_sac_del_sector_3() {
    for n in [0, 1, 10, 99, 100, 511, 512, 999, 1000, 1023] {
        let mut m = maquina(&[]);
        m.store.page_mut(1).copy_from_slice(&sector(3));
        m.c = 64 + 12;
        m.b[1] = 64 + 63;
        m.b[5] = 432;
        m.b[6] = 123;
        m.b[7] = n;
        let mut io = BufferIo::default();
        for _ in 0..1_000 {
            if m.c == 64 + 63 {
                break;
            }
            m.step(&mut io).unwrap();
        }
        assert_eq!(m.c, 64 + 63);
        assert_eq!(mercury_core::tape::decode(&io.output[1]), n.to_string());
        assert_eq!((m.b[5], m.b[6]), (432, 123));
    }
}

#[test]
fn rescata_el_almacen_al_tambor() {
    let mut m = maquina(&[0, 2, 9]);
    for pagina in 1..=31 {
        m.store.page_mut(pagina).copy_from_slice(&datos_de_prueba((pagina % 10) as u16));
    }
    *m.drum.sector_mut(479) = datos_de_prueba(2);
    m.initial_transfer();
    let mut io = BufferIo { switches: 1 << 4, ..Default::default() };
    m.step(&mut io).unwrap();
    io.switches = 0;
    let mut digitos = [1, 3, 2].into_iter();
    for _ in 0..20_000 {
        // Son tres pulsaciones sucesivas. El sector 2 espera la liberación
        // y hace la conversión a decimal siguiendo los bits de las llaves.
        if m.c == 29 && io.switches == 0 {
            io.switches = 1 << digitos.next().expect("pidió un cuarto dígito");
        }
        if m.c == 33 {
            io.switches = 0;
        }
        m.step(&mut io).unwrap();
        if !io.hoots.is_empty() {
            break;
        }
    }
    assert_eq!(digitos.len(), 0);
    assert_eq!(m.drum.sector(132), &datos_de_prueba(2));
    for pagina in 1..=31 {
        let datos = datos_de_prueba((pagina % 10) as u16);
        assert_eq!(m.drum.sector(132 + pagina), &datos);
        assert_eq!(m.store.page(pagina), &datos);
    }
    assert!(!io.hoots.is_empty());
}

#[test]
fn post_mortem_consulta_la_tabla_inversa_del_sector_11() {
    for (funcion, esperado) in [
        (0o001, 0x395), // 59: dirección media
        (0o041, 0x204), // 40: dirección larga
        (0o051, 0x100), // 00: dirección corta
        (0o025, 0x136), // 63: dirección corta del primer cuarto
        (0o151, 0x100), // 00: segundo cuarto
        (0o003, 0),    // sin asignar
    ] {
        let mut m = maquina(&[8, 11]);
        m.store.page_mut(0).copy_from_slice(&sector(11));
        m.c = 63;
        // El sector 7 deja en el sac el código más 392. Para 120–127
        // quita además el bit de segundo cuarto antes de consultar la tabla.
        m.b[7] = funcion + 392 - if funcion >= 120 { 64 } else { 0 };
        let mut io = BufferIo::default();
        for _ in 0..4 {
            m.step(&mut io).unwrap();
        }
        assert_eq!(m.b[7], esperado, "función binaria {funcion:o}");
        assert_eq!(m.store.page(0), &sector(8));
        assert_eq!(m.c, 63);
    }
}

#[test]
fn post_mortem_perfora_una_pagina_en_forma_escrita() {
    let mut m = maquina(&[6, 7, 8, 9, 10, 11]);
    m.store.page_mut(0).copy_from_slice(&sector(6));
    m.store.page_mut(31).copy_from_slice(&assemble_sector("0 590 1.31\n1 620 7\n2 990 0").unwrap());
    m.c = 63;
    m.b[6] = 6; // llave 5, como la entrega el arranque
    m.b[7] = 31;
    let mut io = BufferIo::default();
    assert_eq!(m.run(&mut io, 100_000), Some(Stop::Stop99), "C={}", m.c);
    let listado = mercury_core::tape::decode(&io.output[1]);
    assert!(listado.starts_with("\n\nP 31\n590  1.31\n620  0.7\n990"), "{listado:?}");
}

#[test]
fn arranque_sin_llaves_entra_en_el_sector_2() {
    // Con las llaves en cero el arranque trae el sector 2 a la página 0 y lo
    // entra en la línea 63. Se pone allí un 990 7 de marca.
    let mut m = maquina(&[0]);
    let marca = mercury_core::asm::assemble_sector("63 990 7").unwrap();
    *m.drum.sector_mut(2) = marca;
    m.initial_transfer();
    let mut io = BufferIo::default();
    assert_eq!(m.run(&mut io, 1000), Some(Stop::Stop99));
    assert_eq!(m.c, 64 & 0x3ff); // se detuvo después de la línea 63
    assert_eq!(m.b[6], 0);
}

#[test]
fn arranque_elige_el_sector_por_la_llave_mas_alta() {
    // Llave 1 (valor 2): rutinas de los ingenieros, sector 511, B6 = 2.
    // Llave 2 (valor 4) y llave 0: tele-input, sector 1, B6 = 3.
    for (llaves, sector_esperado, b6) in [(2u16, 511usize, 2u16), (5, 1, 3), (1, 2, 1)] {
        let mut m = maquina(&[0]);
        *m.drum.sector_mut(sector_esperado) = mercury_core::asm::assemble_sector("63 990 7").unwrap();
        m.initial_transfer();
        let mut io = BufferIo { switches: llaves, ..Default::default() };
        assert_eq!(m.run(&mut io, 1000), Some(Stop::Stop99), "llaves {llaves}");
        assert_eq!(m.store.short(127), 7, "llaves {llaves}: no entró en el sector {sector_esperado}");
        assert_eq!(m.b[6], b6, "llaves {llaves}");
    }
}

/// Perfora con el tele-output del sector 0 los sectores `primero..=ultimo`
/// del tambor de `m`, hasta la cola de borrados.
fn perforar(m: &mut Machine, primero: u16, ultimo: u16) -> Vec<u16> {
    m.initial_transfer();
    let mut io = BufferIo { switches: 512, ..Default::default() };
    assert_eq!(m.run(&mut io, 1000), Some(Stop::Stop99));
    io.switches = primero;
    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert_eq!(m.run(&mut io, 1000), Some(Stop::Stop99));
    io.switches = ultimo;
    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert_eq!(m.run(&mut io, 1_000_000), Some(Stop::Stop99));
    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert_eq!(m.run(&mut io, 1000), Some(Stop::LoopStop));
    io.output[1].clone()
}

/// Arranca con la llave 2 (tele-input) y la cinta dada.
fn leer(m: &mut Machine, cinta: &[u16], pasos: u64) -> (Option<Stop>, BufferIo) {
    m.initial_transfer();
    let mut io = BufferIo::with_tape(cinta);
    io.switches = 4;
    let r = m.run(&mut io, pasos);
    (r, io)
}

#[test]
fn tele_output_con_la_llave_9() {
    // Llave 9 y sector 0: entra en el tele-output del propio sector 0, que se
    // detiene dos veces para leer el primer y el último sector de las llaves,
    // los perfora en binario con su suma de control, y se detiene otra vez
    // antes de perforar el aviso final.
    let mut m = maquina(&[0]);
    for (k, v) in m.drum.sector_mut(5).iter_mut().enumerate() {
        *v = (k as u16 * 37 + 11) & 0x3ff;
    }
    let datos = *m.drum.sector(5);
    m.initial_transfer();
    let mut io = BufferIo { switches: 512, ..Default::default() };
    assert_eq!(m.run(&mut io, 1000), Some(Stop::Stop99)); // 990 0 de la línea 15
    io.switches = 5;
    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert_eq!(m.run(&mut io, 1000), Some(Stop::Stop99)); // 990 0 de la línea 17
    io.switches = 5;
    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert_eq!(m.run(&mut io, 100_000), Some(Stop::Stop99)); // 990 1.0 de la línea 55
    let p = &io.output[1];
    // 100 filas en blanco, el encabezado y 128 pares de caracteres.
    assert!(p[..100].iter().all(|c| *c == 0));
    assert_eq!(&p[100..105], &[28, 8, 0, 4, 0]);
    let mut suma: u16 = 0;
    for k in 0..128 {
        let palabra = (p[105 + 2 * k] << 5) | p[106 + 2 * k];
        assert_eq!(palabra, datos[k], "palabra {k}");
        suma = suma.wrapping_add(palabra);
    }
    let control = (p[361] << 5) | p[362];
    assert_eq!((suma + control) & 0x3ff, 0, "suma de control");
    assert_eq!(p[363], 26);
    assert_eq!((p[364] << 5) | p[365], 5);
    assert_eq!(p[366], 2);
    // La pulgada en blanco que separa sectores se perfora al volver a la
    // línea 21; después del último no hay.
    assert_eq!(p.len(), 367);
    // Al seguir: aviso de parada en bucle, 10 borrados y el bucle 590 60.
    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert_eq!(m.run(&mut io, 1000), Some(Stop::LoopStop));
    assert_eq!(m.c, 60);
    assert_eq!(m.prepulse(&mut io), Err(Stop::LoopStop));
    assert_eq!(&io.output[1][367..], &[30, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31]);
}

fn datos_de_prueba(semilla: u16) -> [u16; 128] {
    let mut s = [0u16; 128];
    for (k, v) in s.iter_mut().enumerate() {
        *v = (k as u16 * 211 + semilla * 97 + 5) & 0x3ff;
    }
    s
}

#[test]
fn tele_input_lee_lo_que_perfora_el_tele_output() {
    // Ida y vuelta entre dos rutinas transcriptas por separado: el sector 0
    // perfora los sectores 5 y 6 y el sector 1 los vuelve a escribir en un
    // tambor vacío, con suma de control y verificación del tambor. El aviso
    // 30 del final deja la máquina tocando la bocina en bucle.
    let mut origen = maquina(&[0]);
    *origen.drum.sector_mut(5) = datos_de_prueba(1);
    *origen.drum.sector_mut(6) = datos_de_prueba(2);
    let cinta = perforar(&mut origen, 5, 6);

    let mut m = maquina(&[0, 1]);
    let (r, io) = leer(&mut m, &cinta, 2_000_000);
    assert_eq!(r, None, "no debería detenerse");
    // Leyó hasta el aviso 30; los borrados de la cola quedan sin leer.
    assert_eq!(cinta[io.input_pos[1] - 1], 30);
    assert_eq!(io.input_pos[1], cinta.len() - 10);
    assert_eq!(*m.drum.sector(5), datos_de_prueba(1));
    assert_eq!(*m.drum.sector(6), datos_de_prueba(2));
    assert!(!io.hoots.is_empty(), "la bocina del aviso 30");
}

#[test]
fn tele_input_se_detiene_si_la_suma_no_da() {
    let mut origen = maquina(&[0]);
    *origen.drum.sector_mut(5) = datos_de_prueba(3);
    let mut cinta = perforar(&mut origen, 5, 5);
    cinta[150] ^= 1; // un bit cambiado en la información principal
    let mut m = maquina(&[0, 1]);
    let (r, _) = leer(&mut m, &cinta, 2_000_000);
    assert_eq!(r, Some(Stop::Stop99));
    assert_eq!(m.c, 17, "990 0 de la línea 16");
    assert_eq!(*m.drum.sector(5), [0; 128], "no escribió el sector");
}

#[test]
fn tele_input_salta_lee_sectores_y_acepta_1023() {
    // 28: dos pares de caracteres al corto 256 (la línea 2.0) con suma 1023,
    // que no se verifica; 27: el sector 9 a la página 3; 29: saltar a la
    // línea 3.0 = 192, donde quedó el 990 7 del sector 9.
    let par = |w: u16| [w >> 5, w & 31];
    let palabras = assemble_sector("0 990 5").unwrap();
    let mut cinta = vec![0, 0, 13, 28, 8, 0, 0, 2];
    cinta.extend(par(palabras[0]));
    cinta.extend(par(palabras[1]));
    cinta.extend([31, 31]);
    cinta.extend([27, 0, 9, 3, 29, 6, 0]);
    let mut m = maquina(&[0, 1]);
    *m.drum.sector_mut(9) = assemble_sector("0 990 7").unwrap();
    let (r, _) = leer(&mut m, &cinta, 100_000);
    assert_eq!(r, Some(Stop::Stop99));
    assert_eq!(m.c, 193);
    assert_eq!(m.store.short(257), 5, "la carga con suma 1023");
    assert_eq!(m.store.short(385), 7);
}
