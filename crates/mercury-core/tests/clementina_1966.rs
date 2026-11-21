use mercury_core::io::BufferIo;
use mercury_core::opcodes::{encode, make_instruction};
use mercury_core::{Config, Machine, Stop};

#[test]
fn las_ordenes_de_paiuk_seleccionan_la_mitad_sin_cambiar_el_control() {
    let mut m = Machine::new(Config::clementina_1966());
    assert_eq!(m.drum.len(), 1024);
    for (c, (funcion, segundo_cuarto, b, n)) in [
        (65, false, 0, 0),
        (0, false, 1, 300),
        (0, true, 2, 300),
        (10, false, 3, 1023),
        (20, false, 3, 0),
        (21, false, 0, 301),
        (60, true, 0, 302),
        (61, false, 0, 303),
        (63, false, 0, 300),
        (99, false, 0, 0),
        (66, false, 0, 0),
        (0, false, 4, 300),
        (99, false, 0, 0),
    ].into_iter().enumerate() {
        m.store.set_medium(c, make_instruction(encode(funcion, segundo_cuarto).unwrap(), b, n));
    }
    m.store.set_short(300, 7);
    m.store.set_short(2048 + 300, 42);
    m.store.set_short(3072 + 300, 43);
    m.store.set_short(4095, 52);

    let mut io = BufferIo::with_tape(&[13]);
    io.switches = 27;
    assert_eq!(m.run(&mut io, 20), Some(Stop::Stop99));
    assert_eq!(m.c, 10);
    assert!(m.mitad_alta_cortos);
    assert_eq!(m.b[1..3], [42, 43]);
    assert_eq!(m.store.short(301), 0);
    assert_eq!(m.store.short(2048 + 301), 52);
    assert_eq!(m.store.short(3072 + 302), 13);
    assert_eq!(m.store.short(2048 + 303), 27);
    assert_eq!(io.output[1], [7]);

    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert!(!m.mitad_alta_cortos);
    assert_eq!(m.run(&mut io, 5), Some(Stop::Stop99));
    assert_eq!(m.b[4], 7);

    m.goto(0);
    assert_eq!(m.step(&mut io), Ok(()));
    assert!(m.mitad_alta_cortos);
    m.initial_transfer();
    assert!(!m.mitad_alta_cortos);
}

#[test]
fn la_orden_65_conserva_el_comportamiento_ferranti_en_la_instalacion_base() {
    let mut m = Machine::new(Config::default());
    m.store.set_medium(0, make_instruction(encode(65, false).unwrap(), 0, 0));
    assert_eq!(m.step(&mut BufferIo::default()), Ok(()));
    assert!(!m.mitad_alta_cortos);
}
