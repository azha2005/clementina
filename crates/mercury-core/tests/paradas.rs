//! Pruebas de las paradas de CS 225 A §7.5, PREPULSE, INITIAL TRANSFER y llaves del pupitre.

use mercury_core::asm::{assemble, assemble_sector};
use mercury_core::io::BufferIo;
use mercury_core::opcodes;
use mercury_core::{Config, Machine, Stop};

fn preparar(src: &str) -> Machine {
    let prog = assemble(src).expect("ensamblado");
    let mut m = Machine::new(Config::default());
    prog.load(&mut m.store);
    m.goto(prog.start);
    m
}

#[test]
fn parada_99_y_prepulsable_siguen_con_prepulse() {
    let mut m = preparar(
        "
        @64
        990 0
        570 0
        990 1
        990 2
        105 42
        990 0
    ",
    );
    // En la línea 65 ponemos la parada prepulsable 1 000 000 (§3.13).
    m.store.set_medium(65, opcodes::make_instruction(opcodes::PREPULSE_STOP_BIN, 0, 0));
    let mut io = BufferIo::default();

    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    assert_eq!(m.c, 65);
    assert!(m.stop_flip_flop());
    // Sin PREPULSE queda retenida.
    assert_eq!(m.step(&mut io), Err(Stop::Stop99));
    assert_eq!(m.c, 65);

    // PREPULSE ejecuta la 65 (parada prepulsable).
    assert_eq!(m.prepulse(&mut io), Err(Stop::PrepulseStop { bin_fn: opcodes::PREPULSE_STOP_BIN }));
    assert_eq!(m.c, 66);
    assert!(m.stop_flip_flop());
    assert_eq!(m.step(&mut io), Err(Stop::PrepulseStop { bin_fn: opcodes::PREPULSE_STOP_BIN }));

    // Dos 990 seguidos: cada PREPULSE avanza uno.
    assert_eq!(m.prepulse(&mut io), Err(Stop::Stop99));
    assert_eq!(m.c, 67);
    assert_eq!(m.prepulse(&mut io), Err(Stop::Stop99));
    assert_eq!(m.c, 68);

    // PREPULSE ejecuta 105 42 y devuelve el control al modo automático.
    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert!(!m.stop_flip_flop());
    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    assert_eq!(m.b[5], 42);
}

#[test]
fn desborde_enciende_stop_flip_flop_y_sigue_en_la_siguiente_con_prepulse() {
    let mut m = preparar(
        "
        @16
        +1,70
        @64
        400 16
        500 16
        105 77
        990 0
    ",
    );
    let mut io = BufferIo::default();

    assert_eq!(m.run(&mut io, 10), Some(Stop::Overflow));
    assert_eq!(m.c, 66);
    assert_eq!(m.time_us, 120 + 300);
    assert!(m.stop_flip_flop());
    assert_eq!(m.step(&mut io), Err(Stop::Overflow));

    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert!(!m.stop_flip_flop());
    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    assert_eq!(m.b[5], 77);
}

#[test]
fn parada_opcional_detiene_despues_de_obedecer_la_instruccion() {
    let mut m = preparar(
        "
        @64
        105 11
        106 22
        105 33
        990 0
    ",
    );
    let mut io = BufferIo {
        handswitch_stop: true,
        address_keys: 65,
        ..Default::default()
    };

    assert_eq!(m.run(&mut io, 10), Some(Stop::OptionalStop));
    assert_eq!(m.c, 66);
    assert_eq!(m.b[5], 11);
    assert_eq!(m.b[6], 22);
    assert_eq!(m.stop(), Some(Stop::OptionalStop));
    assert!(m.stop_flip_flop());
    assert_eq!(m.step(&mut io), Err(Stop::OptionalStop));

    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert!(!m.stop_flip_flop());
    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    assert_eq!(m.b[5], 33);
}

#[test]
fn parada_opcional_en_60_62_63_para_dos_veces() {
    // 600 10 (lee al corto 20)
    let mut m = preparar("@64\n600 10\n990 0\n");
    let mut io = BufferIo::with_tape(&[19]);
    io.handswitch_stop = true;
    io.address_keys = 64;
    assert_eq!(m.run(&mut io, 10), Some(Stop::OptionalStop));
    assert_eq!(m.c, 64);
    assert_eq!(io.input_pos[1], 0);
    assert_eq!(m.store.short(20), 0);
    assert!(m.stop_flip_flop());

    assert_eq!(m.prepulse(&mut io), Err(Stop::OptionalStop));
    assert_eq!(m.c, 65);
    assert_eq!(io.input_pos[1], 1);
    assert_eq!(m.store.short(20), 19);
    assert!(m.stop_flip_flop());

    assert_eq!(m.prepulse(&mut io), Err(Stop::Stop99));
    assert_eq!(m.c, 66);

    // 620 23 (perfora literal 23)
    let mut m = preparar("@64\n620 23\n990 0\n");
    let mut io = BufferIo {
        handswitch_stop: true,
        address_keys: 64,
        ..Default::default()
    };
    assert_eq!(m.run(&mut io, 10), Some(Stop::OptionalStop));
    assert_eq!(m.c, 64);
    assert!(io.output[1].is_empty());
    assert!(m.stop_flip_flop());

    assert_eq!(m.prepulse(&mut io), Err(Stop::OptionalStop));
    assert_eq!(m.c, 65);
    assert_eq!(io.output[1], vec![23]);
    assert!(m.stop_flip_flop());

    assert_eq!(m.prepulse(&mut io), Err(Stop::Stop99));

    // 630 10 (perfora el corto 20)
    let mut m = preparar("@10\n=15\n@64\n630 10\n990 0\n");
    let mut io = BufferIo {
        handswitch_stop: true,
        address_keys: 64,
        ..Default::default()
    };
    assert_eq!(m.run(&mut io, 10), Some(Stop::OptionalStop));
    assert_eq!(m.c, 64);
    assert!(io.output[1].is_empty());
    assert!(m.stop_flip_flop());

    assert_eq!(m.prepulse(&mut io), Err(Stop::OptionalStop));
    assert_eq!(m.c, 65);
    assert_eq!(io.output[1], vec![15]);
    assert!(m.stop_flip_flop());

    assert_eq!(m.prepulse(&mut io), Err(Stop::Stop99));
}

#[test]
fn parada_opcional_en_68_y_69_no_transfiere_tambor() {
    let mut m = preparar("@64\n670 5\n680 3\n990 0\n");
    *m.drum.sector_mut(5) = [111; 128];
    m.store.page_mut(3).fill(222);
    let mut io = BufferIo {
        handswitch_stop: true,
        address_keys: 65,
        ..Default::default()
    };
    assert_eq!(m.run(&mut io, 10), Some(Stop::OptionalStop));
    assert_eq!(m.c, 66);
    assert_eq!(m.store.page(3), &[222; 128]);
    assert_eq!(m.drum.sector(5), &[111; 128]);
    assert!(m.stop_flip_flop());
    assert_eq!(m.prepulse(&mut io), Err(Stop::Stop99));

    let mut m = preparar("@64\n670 6\n690 4\n990 0\n");
    *m.drum.sector_mut(6) = [333; 128];
    m.store.page_mut(4).fill(444);
    let mut io = BufferIo {
        handswitch_stop: true,
        address_keys: 65,
        ..Default::default()
    };
    assert_eq!(m.run(&mut io, 10), Some(Stop::OptionalStop));
    assert_eq!(m.c, 66);
    assert_eq!(m.drum.sector(6), &[333; 128]);
    assert!(m.stop_flip_flop());
    assert_eq!(m.prepulse(&mut io), Err(Stop::Stop99));
}

#[test]
fn sector_inexistente_en_68_y_69_solo_se_levanta_con_initial_transfer() {
    for instr in ["680 2", "690 2"] {
        let mut m = preparar(&format!("@64\n670 512\n{instr}\n990 0\n"));
        *m.drum.sector_mut(0) = assemble_sector("0 990 7").unwrap();
        let mut io = BufferIo::default();

        assert_eq!(m.run(&mut io, 10), Some(Stop::NonExistentSector { sector: 512 }), "{instr}");
        assert_eq!(m.t, 512);
        assert_eq!(m.c, 65);
        assert!(!m.stop_flip_flop());
        // PREPULSE no levanta la parada (v).
        assert_eq!(m.prepulse(&mut io), Err(Stop::NonExistentSector { sector: 512 }));
        assert_eq!(m.step(&mut io), Err(Stop::NonExistentSector { sector: 512 }));

        // Sólo INITIAL TRANSFER la levanta.
        m.initial_transfer();
        assert_eq!(m.stop(), None);
        assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
        assert_eq!(m.c, 1);
    }

    // Con 1024 sectores el sector 512 existe.
    let prog = assemble("@64\n670 512\n680 2\n990 0\n").unwrap();
    let mut m = Machine::new(Config { drum_sectors: 1024, ..Default::default() });
    prog.load(&mut m.store);
    m.goto(prog.start);
    *m.drum.sector_mut(512) = [77; 128];
    let mut io = BufferIo::default();
    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    assert_eq!(m.store.page(2), &[77; 128]);
}

#[test]
fn parada_por_bucle_y_excepciones_de_contadores() {
    // 590 * y saltos condicionales tomados hacia sí mismos.
    let bucles = [
        "@64\n590 64\n",
        "@64\n101 1\n080 65\n",
        "@64\n300 1\n280 65\n",
        "@16\n+1\n@64\n400 16\n490 65\n",
        "@64\n101 1\n180 65\n", // bd = 0 no modifica ningún B distinto de cero
    ];
    for (caso, src) in bucles.iter().enumerate() {
        let mut m = preparar(src);
        *m.drum.sector_mut(0) = assemble_sector("0 990 7").unwrap();
        let mut io = BufferIo::default();
        assert_eq!(m.run(&mut io, 20), Some(Stop::LoopStop), "caso {caso}");
        assert!(!m.stop_flip_flop());
        // PREPULSE no levanta la parada (vi).
        assert_eq!(m.prepulse(&mut io), Err(Stop::LoopStop));
        assert_eq!(m.step(&mut io), Err(Stop::LoopStop));

        // INITIAL TRANSFER la levanta.
        m.initial_transfer();
        assert_eq!(m.stop(), None);
        assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    }

    // Salto condicional no tomado hacia sí mismo: no es bucle.
    let mut m = preparar("@64\n300 0\n280 65\n990 0\n");
    let mut io = BufferIo::default();
    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));

    // Contadores 380 * y 18b * (b != 0) modifican el estado en cada vuelta y salen.
    let mut m = preparar("@64\n300 -3\n380 65\n990 0\n");
    let mut io = BufferIo::default();
    assert_eq!(m.run(&mut io, 20), Some(Stop::Stop99));
    assert_eq!(m.sac(), 1);

    let mut m = preparar("@64\n101 -3\n181 65\n990 0\n");
    let mut io = BufferIo::default();
    assert_eq!(m.run(&mut io, 20), Some(Stop::Stop99));
    assert_eq!(m.b[1], 1);
}

#[test]
fn modo_single_una_instruccion_por_prepulse_y_dos_en_60_62_63() {
    let mut m = preparar(
        "
        @64
        105 9
        600 10
        630 10
        620 30
        106 12
        990 0
    ",
    );
    let mut io = BufferIo {
        single_mode: true,
        ..Default::default()
    };

    // En SINGLE, `run` frena de inmediato sin ejecutar ninguna instrucción.
    assert_eq!(m.run(&mut io, 100), Some(Stop::Single));
    assert_eq!(m.executed, 0);
    assert_eq!(m.c, 64);
    assert!(!m.stop_flip_flop());

    // 105 9 (un prepulso)
    assert_eq!(m.prepulse(&mut io), Err(Stop::Single));
    assert_eq!(m.c, 65);
    assert_eq!(m.b[5], 9);
    assert_eq!(m.executed, 1);

    // 600 10: primer prepulso retiene antes de leer; segundo intenta leer.
    assert_eq!(m.prepulse(&mut io), Err(Stop::Single));
    assert_eq!(m.c, 65);
    assert_eq!(m.executed, 1);
    assert_eq!(m.prepulse(&mut io), Err(Stop::InputExhausted { channel: 1 }));
    assert_eq!(m.c, 65);
    assert_eq!(m.executed, 1);

    // Al agregar cinta, el siguiente prepulso completa el segundo pulso de 600 10.
    io.input[1].push(17);
    assert_eq!(m.prepulse(&mut io), Err(Stop::Single));
    assert_eq!(m.c, 66);
    assert_eq!(m.store.short(20), 17);
    assert_eq!(m.executed, 2);

    // 630 10: dos prepulsos
    assert_eq!(m.prepulse(&mut io), Err(Stop::Single));
    assert_eq!(m.c, 66);
    assert!(io.output[1].is_empty());
    assert_eq!(m.prepulse(&mut io), Err(Stop::Single));
    assert_eq!(m.c, 67);
    assert_eq!(io.output[1], vec![17]);

    // 620 30: dos prepulsos
    assert_eq!(m.prepulse(&mut io), Err(Stop::Single));
    assert_eq!(m.c, 67);
    assert_eq!(io.output[1].len(), 1);
    assert_eq!(m.prepulse(&mut io), Err(Stop::Single));
    assert_eq!(m.c, 68);
    assert_eq!(io.output[1], vec![17, 30]);

    // Pasar a CONTINUOUS reanuda sin prepulso.
    io.single_mode = false;
    assert_eq!(m.run(&mut io, 100), Some(Stop::Stop99));
    assert_eq!(m.b[6], 12);
    assert_eq!(m.c, 70);
}

#[test]
fn write_current_off_impide_escribir_y_tele_input_cae_en_bucle() {
    let mut m = preparar("@64\n670 10\n690 2\n990 0\n");
    *m.drum.sector_mut(10) = [123; 128];
    m.store.page_mut(2).fill(456);
    let mut io = BufferIo {
        write_current: false,
        ..Default::default()
    };
    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    assert_eq!(m.drum.sector(10), &[123; 128]);

    // Con los sectores 0 y 1 del Input Routine y WRITE CURRENT en OFF, el
    // tele-input detecta que el tambor no se grabó y cae en el bucle 280 60.
    let mut m = Machine::new(Config::default());
    *m.drum.sector_mut(0) = assemble_sector(include_str!("../../../input_routine/sector00.txt")).unwrap();
    *m.drum.sector_mut(1) = assemble_sector(include_str!("../../../input_routine/sector01.txt")).unwrap();
    let par = |w: u16| [w >> 5, w & 31];
    let palabras = assemble_sector("0 990 5").unwrap();
    let mut cinta = vec![0, 0, 13, 28, 8, 0, 0, 2];
    cinta.extend(par(palabras[0]));
    cinta.extend(par(palabras[1]));
    cinta.extend([31, 31, 26, 0, 5, 2]);
    let mut io = BufferIo::with_tape(&cinta);
    io.switches = 4; // llave 2: tele-input
    io.write_current = false;

    m.initial_transfer();
    assert_eq!(m.run(&mut io, 100_000), Some(Stop::LoopStop));
    assert_eq!(m.c, 60);
    assert_eq!(m.drum.sector(5), &[0; 128]);
}

#[test]
fn modo_manual_obedece_llaves_con_prepulse() {
    let mut m = preparar("@64\n106 77\n990 0\n");
    let funcion_105 = ((opcodes::encode(10, false).unwrap() as u16) << 3) | 5;
    let mut io = BufferIo {
        manual_mode: true,
        function_keys: funcion_105,
        address_keys: 42,
        ..Default::default()
    };

    assert_eq!(m.step(&mut io), Err(Stop::Manual));
    assert_eq!(m.c, 64);
    assert_eq!(m.b[5], 0);

    // En MANUAL, PREPULSE obedece la instrucción armada en FUNCTION y ADDRESS sin avanzar C.
    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert_eq!(m.b[5], 42);
    assert_eq!(m.c, 64);
    assert_eq!(m.b[6], 0);
    assert_eq!(m.step(&mut io), Err(Stop::Manual));

    // Al pasar a AUTO continúa el programa en C.
    io.manual_mode = false;
    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    assert_eq!(m.b[5], 42);
    assert_eq!(m.b[6], 77);
    assert_eq!(m.c, 66);
}
