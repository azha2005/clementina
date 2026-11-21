//! Programas completos ejecutados en la máquina emulada.

use mercury_core::acc::Float;
use mercury_core::asm::assemble;
use mercury_core::io::BufferIo;
use mercury_core::{tape, Config, Machine, Stop};

fn boot(src: &str, io_tape: &[u16]) -> (Machine, BufferIo) {
    let prog = assemble(src).expect("ensamblado");
    let mut m = Machine::new(Config::default());
    prog.load(&mut m.store);
    m.c = prog.start;
    (m, BufferIo::with_tape(io_tape))
}

#[test]
fn add_hundred_times_cs225a_p12() {
    // Ejemplo 1 de la pág. 12: sumar L32 al acumulador cien veces.
    let src = "
        @32
        +1.5
        @64
        106 100
        420 32  (1
        136 1
        080 v1
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 10_000), Some(Stop::Stop99));
    assert!((m.acc.to_f64() - 150.0).abs() < 1e-6, "{}", m.acc.to_f64());
    assert_eq!(m.b[6], 0);
}

#[test]
fn x_squared_minus_y_squared_cs225a_p10() {
    let src = "
        @32
        +7
        +3
        @64
        400 32
        500 32
        410 40
        400 34
        510 34
        420 40
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 100), Some(Stop::Stop99));
    // El redondeo "forzar un 1" deja el último bit en 1: 40 + 2^-23.
    assert!((m.acc.to_f64() - 40.0).abs() < 1e-6, "{} {:?}", m.acc.to_f64(), m.acc);
    // Tiempos de §3.11: 120 + 300 + 120 + 120 + 300 + 180 + 60 = 1200 µs.
    assert_eq!(m.time_us, 1200);
}

#[test]
fn count_and_test_18_tests_before_counting() {
    let src = "
        106 -5
        580 0   (1
        186 v1
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 100), Some(Stop::Stop99));
    assert_eq!(io.hoots.len(), 6);
    assert_eq!(io.hoots[1] - io.hoots[0], 120);
    assert_eq!(m.b[6], 1);
}

#[test]
fn tape_echo_until_end_of_tape() {
    let input = tape::encode("MERCURY 1961\n").unwrap();
    let src = "
        600 10  (1
        630 10
        590 v1
    ";
    let (mut m, mut io) = boot(src, &input);
    assert_eq!(m.run(&mut io, 10_000), Some(Stop::InputExhausted { channel: 1 }));
    assert_eq!(io.output[1], input);
    // Retenida en la instrucción de lectura, como la máquina real.
    assert_eq!(m.c, 64);
    // La perforadora es el cuello de botella: un carácter cada 30 ms.
    assert!(m.time_us >= 30_000 * (input.len() as u64 - 1));
}

#[test]
fn drum_round_trip() {
    let src = "
        @2.0
        +2.5
        @64
        670 300         ; T = sector 300
        690 2           ; tambor ← página 2
        670 300
        680 5           ; página 5 ← tambor
        400 5.0
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 100), Some(Stop::Stop99));
    assert_eq!(m.acc.to_f64(), 2.5);
    // Cada transferencia dura al menos 7,68 ms y a lo sumo una vuelta más.
    assert!(m.time_us >= 2 * 7_680);
    assert!(m.time_us <= 2 * (7_680 + 17_280) + 300);
}

#[test]
fn handswitches_and_negative_b_modification() {
    let src = "
        610 20          ; H20 = llaves
        107 -1          ; B7 (sac) = -1
        000 20          ; B0 = ... (B0 no existe: sólo fija Bt)
        207 21          ; S = H(21 + ½·(-1)) = H20+
        990 0
        @20
        =0
        =777
    ";
    let (mut m, mut io) = boot(src, &[]);
    io.switches = 0b1010101010;
    assert_eq!(m.run(&mut io, 100), Some(Stop::Stop99));
    assert_eq!(m.b[0], 0);
    assert_eq!(m.bt, 0b1010101010);
    assert_eq!(m.sac(), 777);
}

#[test]
fn exponent_overflow_stops_machine() {
    let src = "
        @16
        +1,70
        @64
        400 16
        500 16
        500 16
        500 16
        500 16
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 100), Some(Stop::Overflow));
}

#[test]
fn exponent_instructions_70_71() {
    let src = "
        @16
        +12
        @64
        400 16
        701 0           ; B1 = E = 4
        121 3           ; B1 = 7
        711 0           ; E = B1
        410 18
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 100), Some(Stop::Stop99));
    assert_eq!(Float::from_long(m.store.long(9)).to_f64(), 96.0);
}

#[test]
fn fibonacci_example_program() {
    let src = include_str!("../../../examples/fibonacci.mer");
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 100_000), Some(Stop::Stop99));
    let text = tape::decode(&io.output[1]);
    let expected = "001\n001\n002\n003\n005\n008\n013\n021\n034\n055\n089\n144\n233\n377\n";
    assert_eq!(text, expected);
}

#[test]
fn clementine_melody_pitches() {
    let src = include_str!("../../../examples/clementine.mer");
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 1_000_000), Some(Stop::Stop99));
    // Primera nota: Do4 con k = 30 → período 240 + 120·30 = 3840 µs (≈ 260 Hz), 52 impulsos.
    assert_eq!(io.hoots[1] - io.hoots[0], 3840);
    assert_eq!(io.hoots[51] - io.hoots[50], 3840);
    // La cuarta nota es Sol3 (k = 41): 5160 µs.
    let start_g3 = 52 + 52 + 156;
    assert_eq!(io.hoots[start_g3 + 1] - io.hoots[start_g3], 5160);
    assert_eq!(io.hoots.len(), 3155);
}

#[test]
fn multichannel_911_example_from_section_3_9() {
    // §3.9: si Sac contiene 6, "911 8" envía el espacio (valor 14) al canal 1.
    let (mut m, mut io) = boot("307 6\n911 8\n990 0\n", &[]);
    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    assert_eq!(io.output[1], vec![14]);
}

#[test]
fn multichannel_channel_0_is_exponent() {
    // 900 n: S = St = E + n.  910 n: E = S + n.
    let src = "
        @16
        +12             ; exponente 4
        @64
        400 16
        900 3           ; S = 7
        910 1           ; E = 8
        410 18
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    assert_eq!(m.sac(), 7);
    assert_eq!(Float::from_long(m.store.long(9)).to_f64(), 12.0 * 16.0);
    // Con dígito B = 0 tarda 60 µs (§3.12 nota de 90–97).
    assert_eq!(m.time_us, 120 + 60 + 60 + 120 + 60);
}

#[test]
fn second_quarter_short_store_and_load() {
    // 600 es un registro medio del segundo cuarto (cortos 1200/1201).
    let src = "
        107 345
        017 600+        ; H600+ = B7 → función 01*
        003 600+        ; B3 = H600+ → función 00* (instr. B: dígito B = 3)
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
    assert_eq!(m.store.short(1201), 345);
    assert_eq!(m.b[3], 345);
}

#[test]
fn unstandardised_add_46_and_least_significant_product_54() {
    let src = "
        @16
        +2
        +1
        @64
        400 16
        460 18          ; 2 + 1 sin estandarizar: 0.011·2³
        410 20
        400 16
        540 18          ; mitad menos significativa de 2 × 1
        410 22
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 20), Some(Stop::Stop99));
    let s = Float::from_long(m.store.long(10));
    assert_eq!(s.to_f64(), 3.0);
    assert!(!s.is_standard(), "{s:?}");
    // 2 = 0.1·2², 1 = 0.1·2¹: producto exacto, nada en la mitad baja.
    let p = Float::from_long(m.store.long(11));
    assert_eq!(p.frac, 0);
    assert_eq!(p.exp, 2 + 1 - 29);
}

#[test]
fn sac_count_and_test_38_and_exponent_group_72() {
    let src = "
        @16
        +12             ; exponente 4
        @64
        400 16
        307 -3
        580 0   (1
        380 v1          ; prueba St antes de contar
        101 10
        721 2           ; B1 = B1 + (E + 2) = 16
        771 16          ; Bt = B1 - (E + 16) = -4
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 50), Some(Stop::Stop99));
    assert_eq!(io.hoots.len(), 4);
    assert_eq!(m.sac(), 1);
    assert_eq!(m.b[1], 16);
    assert_eq!(m.bt, (1024 - 4) as u16);
}

/// Cinta 2 del Museo de Informática, leída de las fotos (docs/cinta/TRANSCRIPCION.md),
/// con el espacio superpuesto repuesto.
const CINTA_MUSEO_2: [u16; 49] = [
    8, 2, 31, 14, 3, 18, 14, 7, 18, 3, 12, 9, 24, 4, 22, 10, 6, 28, 24, 5, 27, 14, 8, 2,
    19, 12, 4, 1, 12, 4, 16, 12, 4, 7, 12, 4, 8, 2,
    3, 4, 31, 13, 6, 12, 4, 27, 3, 8, 2,
];

/// Cinta 1, desde el CR inicial hasta donde está cortada.
const CINTA_MUSEO_1: [u16; 34] = [
    8, 2, 14, 3, 18, 14, 7, 18, 3, 12, 9, 24, 4, 22, 10, 6, 28, 24, 5, 27, 14, 8, 2,
    19, 12, 4, 1, 12, 4, 8, 2, 3, 4, 31,
];

#[test]
fn primos_reproduces_museum_tape_2() {
    let src = include_str!("../../../examples/primos.mer");
    let (mut m, mut io) = boot(src, &[]);
    io.switches = 10;
    assert_eq!(m.run(&mut io, 1_000_000), Some(Stop::Stop99));
    assert_eq!(io.output[1], CINTA_MUSEO_2.to_vec());
    assert_eq!(tape::ita2::decode(&io.output[1]), "\nCALCULANDO PRIMOS:\n2, 3, 5, 7, \n- FIN -\n");
}

#[test]
fn primos_with_limit_5_matches_museum_tape_1() {
    let src = include_str!("../../../examples/primos.mer");
    let (mut m, mut io) = boot(src, &[]);
    io.switches = 5;
    assert_eq!(m.run(&mut io, 1_000_000), Some(Stop::Stop99));
    let out = &io.output[1];
    // La cinta 1 no tiene el cambio a letras antes de CALCULANDO; el resto coincide
    // hasta el corte.
    let mut ours: Vec<u16> = out.clone();
    ours.remove(2);
    assert_eq!(&ours[..CINTA_MUSEO_1.len()], &CINTA_MUSEO_1[..]);
}

#[test]
fn primos_up_to_100() {
    let src = include_str!("../../../examples/primos.mer");
    let (mut m, mut io) = boot(src, &[]);
    io.switches = 100;
    assert_eq!(m.run(&mut io, 10_000_000), Some(Stop::Stop99));
    let text = tape::ita2::decode(&io.output[1]);
    assert!(text.contains("2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97, \n"), "{text}");
}

#[test]
fn primos_in_ferranti_code() {
    let src = include_str!("../../../examples/primos_ferranti.mer");
    let (mut m, mut io) = boot(src, &[]);
    io.switches = 10;
    assert_eq!(m.run(&mut io, 1_000_000), Some(Stop::Stop99));
    assert_eq!(tape::decode(&io.output[1]), "\nCALCULANDO PRIMOS\n2, 3, 5, 7, \n- FIN -\n");
    // Debe coincidir con lo que codifica el módulo de cinta a partir del texto.
    let expected = tape::encode("\nCALCULANDO PRIMOS\n2, 3, 5, 7, \n- FIN -\n").unwrap();
    assert_eq!(tape::decode(&expected), tape::decode(&io.output[1]));
}

#[test]
fn punch_literal_is_b_modified() {
    // 62 es B-modificable: 625 10 con B5 = 3 perfora 13. El programa de
    // contornos de H. Whitfield (samples/autocode/contour.mer) elige así el
    // carácter de cada curva de nivel.
    let src = "
        105 3
        625 10
        620 7
        990 0
    ";
    let (mut m, mut io) = boot(src, &[]);
    assert_eq!(m.run(&mut io, 100), Some(Stop::Stop99));
    assert_eq!(io.output[1], vec![13, 7]);
}
