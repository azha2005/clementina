//! Sectores embebidos del *Input Routine* de 1959 (manuscrito anotado).
//!
//! Los sectores 0–12 y 38 vienen de `input_routine/sectorNN.txt`, y los
//! restantes (13–61 salvo el 38) de `input_routine/enlazados/sectorNN.txt`.

use mercury_core::{asm::assemble_sector, Machine};

const TELE_INPUT_SECTORES: [(usize, &str); 2] = [
    (0, include_str!("../../../input_routine/sector00.txt")),
    (1, include_str!("../../../input_routine/sector01.txt")),
];

const RESTO_SECTORES: [(usize, &str); 60] = [
    (2, include_str!("../../../input_routine/sector02.txt")),
    (3, include_str!("../../../input_routine/sector03.txt")),
    (4, include_str!("../../../input_routine/sector04.txt")),
    (5, include_str!("../../../input_routine/sector05.txt")),
    (6, include_str!("../../../input_routine/sector06.txt")),
    (7, include_str!("../../../input_routine/sector07.txt")),
    (8, include_str!("../../../input_routine/sector08.txt")),
    (9, include_str!("../../../input_routine/sector09.txt")),
    (10, include_str!("../../../input_routine/sector10.txt")),
    (11, include_str!("../../../input_routine/sector11.txt")),
    (12, include_str!("../../../input_routine/sector12.txt")),
    (13, include_str!("../../../input_routine/enlazados/sector13.txt")),
    (14, include_str!("../../../input_routine/enlazados/sector14.txt")),
    (15, include_str!("../../../input_routine/enlazados/sector15.txt")),
    (16, include_str!("../../../input_routine/enlazados/sector16.txt")),
    (17, include_str!("../../../input_routine/enlazados/sector17.txt")),
    (18, include_str!("../../../input_routine/enlazados/sector18.txt")),
    (19, include_str!("../../../input_routine/enlazados/sector19.txt")),
    (20, include_str!("../../../input_routine/enlazados/sector20.txt")),
    (21, include_str!("../../../input_routine/enlazados/sector21.txt")),
    (22, include_str!("../../../input_routine/enlazados/sector22.txt")),
    (23, include_str!("../../../input_routine/enlazados/sector23.txt")),
    (24, include_str!("../../../input_routine/enlazados/sector24.txt")),
    (25, include_str!("../../../input_routine/enlazados/sector25.txt")),
    (26, include_str!("../../../input_routine/enlazados/sector26.txt")),
    (27, include_str!("../../../input_routine/enlazados/sector27.txt")),
    (28, include_str!("../../../input_routine/enlazados/sector28.txt")),
    (29, include_str!("../../../input_routine/enlazados/sector29.txt")),
    (30, include_str!("../../../input_routine/enlazados/sector30.txt")),
    (31, include_str!("../../../input_routine/enlazados/sector31.txt")),
    (32, include_str!("../../../input_routine/enlazados/sector32.txt")),
    (33, include_str!("../../../input_routine/enlazados/sector33.txt")),
    (34, include_str!("../../../input_routine/enlazados/sector34.txt")),
    (35, include_str!("../../../input_routine/enlazados/sector35.txt")),
    (36, include_str!("../../../input_routine/enlazados/sector36.txt")),
    (37, include_str!("../../../input_routine/enlazados/sector37.txt")),
    (38, include_str!("../../../input_routine/sector38.txt")),
    (39, include_str!("../../../input_routine/enlazados/sector39.txt")),
    (40, include_str!("../../../input_routine/enlazados/sector40.txt")),
    (41, include_str!("../../../input_routine/enlazados/sector41.txt")),
    (42, include_str!("../../../input_routine/enlazados/sector42.txt")),
    (43, include_str!("../../../input_routine/enlazados/sector43.txt")),
    (44, include_str!("../../../input_routine/enlazados/sector44.txt")),
    (45, include_str!("../../../input_routine/enlazados/sector45.txt")),
    (46, include_str!("../../../input_routine/enlazados/sector46.txt")),
    (47, include_str!("../../../input_routine/enlazados/sector47.txt")),
    (48, include_str!("../../../input_routine/enlazados/sector48.txt")),
    (49, include_str!("../../../input_routine/enlazados/sector49.txt")),
    (50, include_str!("../../../input_routine/enlazados/sector50.txt")),
    (51, include_str!("../../../input_routine/enlazados/sector51.txt")),
    (52, include_str!("../../../input_routine/enlazados/sector52.txt")),
    (53, include_str!("../../../input_routine/enlazados/sector53.txt")),
    (54, include_str!("../../../input_routine/enlazados/sector54.txt")),
    (55, include_str!("../../../input_routine/enlazados/sector55.txt")),
    (56, include_str!("../../../input_routine/enlazados/sector56.txt")),
    (57, include_str!("../../../input_routine/enlazados/sector57.txt")),
    (58, include_str!("../../../input_routine/enlazados/sector58.txt")),
    (59, include_str!("../../../input_routine/enlazados/sector59.txt")),
    (60, include_str!("../../../input_routine/enlazados/sector60.txt")),
    (61, include_str!("../../../input_routine/enlazados/sector61.txt")),
];

fn escribir_sectores(m: &mut Machine, lista: &[(usize, &str)]) {
    for &(n, src) in lista {
        let sector = assemble_sector(src).expect("sector del Input Routine inválido");
        *m.drum.sector_mut(n) = sector;
    }
}

/// Carga sólo los sectores 0 y 1 en el tambor (suficiente para `tele-input`,
/// llave 2).
pub fn cargar_tele_input(m: &mut Machine) {
    escribir_sectores(m, &TELE_INPUT_SECTORES);
}

/// Carga el *Input Routine* completo (sectores 0–61) y sus *Quickies* en el tambor.
pub fn cargar_completo(m: &mut Machine) {
    escribir_sectores(m, &TELE_INPUT_SECTORES);
    escribir_sectores(m, &RESTO_SECTORES);
}
