//! Almacén de cómputo (núcleos de ferrita) y almacén de respaldo (tambores).
//!
//! CS 225 A §1.3 y §1.7: el almacén de cómputo tiene 2048 palabras de 20 bits
//! (32 páginas de 64). Se puede ver como 4096 registros cortos de 10 bits,
//! 2048 medios de 20 o 1024 largos de 40. Lo modelamos como 4096 cortos.
//!
//! * Medio `M` = cortos `2M` (mitad izquierda, más significativa) y `2M+1`
//!   (mitad derecha, la "H+" del manual).
//! * Largo `L` = cortos `4L..4L+3` = exponente y, 10 bits menos significativos
//!   de x, 10 bits del medio de x, 10 bits más significativos de x (§1.7.2).

pub const SHORT_MASK: u16 = 0x3ff;
pub const SHORTS: usize = 4096;
pub const PAGES: usize = 32;
pub const SHORTS_PER_PAGE: usize = 128;

fn paridad(v: u16) -> bool {
    (v & SHORT_MASK).count_ones() & 1 != 0
}

/// Una escritura de página o sector desde el anfitrión genera paridad nueva.
pub struct Escritura<'a, T: AsMut<[u16]> + ?Sized> {
    datos: &'a mut T,
    paridades: &'a mut [bool],
}

impl<T: AsMut<[u16]> + ?Sized> std::ops::Deref for Escritura<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.datos
    }
}

impl<T: AsMut<[u16]> + ?Sized> std::ops::DerefMut for Escritura<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.datos
    }
}

impl<T: AsMut<[u16]> + ?Sized> Drop for Escritura<'_, T> {
    fn drop(&mut self) {
        for (dato, bit) in self.datos.as_mut().iter_mut().zip(self.paridades.iter_mut()) {
            *dato &= SHORT_MASK;
            *bit = paridad(*dato);
        }
    }
}

/// Almacén de cómputo.
#[derive(Clone)]
pub struct Store {
    shorts: Box<[u16; SHORTS]>,
    paridades: Box<[bool; SHORTS]>,
}

impl Default for Store {
    fn default() -> Self {
        Self { shorts: Box::new([0; SHORTS]), paridades: Box::new([false; SHORTS]) }
    }
}

impl Store {
    pub fn short(&self, addr: usize) -> u16 {
        self.shorts[addr % SHORTS]
    }

    pub fn set_short(&mut self, addr: usize, v: u16) {
        let addr = addr % SHORTS;
        self.shorts[addr] = v & SHORT_MASK;
        self.paridades[addr] = paridad(v);
    }

    /// Registro medio (20 bits). `addr` en 0..2047.
    pub fn medium(&self, addr: usize) -> u32 {
        let a = (addr % (SHORTS / 2)) * 2;
        ((self.shorts[a] as u32) << 10) | self.shorts[a + 1] as u32
    }

    pub fn set_medium(&mut self, addr: usize, v: u32) {
        let a = (addr % (SHORTS / 2)) * 2;
        self.set_short(a, (v >> 10) as u16);
        self.set_short(a + 1, v as u16);
    }

    /// Registro largo como 4 cortos [y, x_bajo, x_medio, x_alto]. `addr` en 0..1023.
    pub fn long(&self, addr: usize) -> [u16; 4] {
        let a = (addr % (SHORTS / 4)) * 4;
        [self.shorts[a], self.shorts[a + 1], self.shorts[a + 2], self.shorts[a + 3]]
    }

    pub fn set_long(&mut self, addr: usize, v: [u16; 4]) {
        let a = (addr % (SHORTS / 4)) * 4;
        for (i, s) in v.iter().enumerate() {
            self.set_short(a + i, *s);
        }
    }

    pub fn page(&self, page: usize) -> &[u16] {
        let p = page % PAGES;
        &self.shorts[p * SHORTS_PER_PAGE..(p + 1) * SHORTS_PER_PAGE]
    }

    pub fn page_mut(&mut self, page: usize) -> Escritura<'_, [u16]> {
        let p = page % PAGES;
        let rango = p * SHORTS_PER_PAGE..(p + 1) * SHORTS_PER_PAGE;
        Escritura { datos: &mut self.shorts[rango.clone()], paridades: &mut self.paridades[rango] }
    }

    pub(crate) fn fallo_paridad(&self, inicio: usize, len: usize) -> Option<usize> {
        (inicio..inicio + len).find(|&i| self.paridades[i] != paridad(self.shorts[i]))
    }

    /// Diagnóstico del emulador. Cambia un bit de datos (0–9) o de paridad (10).
    pub fn invertir_bit(&mut self, corto: usize, bit: u8) -> bool {
        if corto >= SHORTS {
            return false;
        }
        invertir_bit(&mut self.shorts[corto], &mut self.paridades[corto], bit)
    }

    pub fn as_slice(&self) -> &[u16] {
        &self.shorts[..]
    }
}

/// Almacén de respaldo: tambores magnéticos divididos en sectores del tamaño de
/// una página (64 medios = 128 cortos). Clementina tenía dos tambores con
/// 512 sectores en total (García Camarero, *Sistema CONVENCIONAL*, 1962, §2.1);
/// algunas Mercury tenían 1024 (CS 225 A §3.12).
#[derive(Clone)]
pub struct Drum {
    sectors: Vec<[u16; SHORTS_PER_PAGE]>,
    paridades: Vec<[bool; SHORTS_PER_PAGE]>,
}

impl Drum {
    /// Carga una imagen de cortos de 10 bits guardados como u16 little-endian.
    /// Una imagen inválida deja el tambor intacto.
    pub fn cargar_imagen(&mut self, datos: &[u8]) -> Result<(), String> {
        let esperado = self.len() * SHORTS_PER_PAGE * 2;
        if datos.len() != esperado {
            return Err(format!("se esperaban {esperado} bytes para {} sectores, hay {}", self.len(), datos.len()));
        }
        for (i, palabra) in datos.chunks_exact(2).enumerate() {
            if palabra[1] > 3 {
                return Err(format!(
                    "valor fuera de 10 bits en el sector {}, corto {}",
                    i / SHORTS_PER_PAGE, i % SHORTS_PER_PAGE
                ));
            }
        }
        for (sector, bytes) in datos.chunks_exact(SHORTS_PER_PAGE * 2).enumerate() {
            for (corto, palabra) in self.sector_mut(sector).iter_mut().zip(bytes.chunks_exact(2)) {
                *corto = u16::from_le_bytes([palabra[0], palabra[1]]);
            }
        }
        Ok(())
    }

    pub fn new(sectors: usize) -> Self {
        Self { sectors: vec![[0; SHORTS_PER_PAGE]; sectors], paridades: vec![[false; SHORTS_PER_PAGE]; sectors] }
    }

    pub fn len(&self) -> usize {
        self.sectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sectors.is_empty()
    }

    pub fn get(&self, n: usize) -> Option<&[u16; SHORTS_PER_PAGE]> {
        self.sectors.get(n)
    }

    pub fn get_mut(&mut self, n: usize) -> Option<Escritura<'_, [u16; SHORTS_PER_PAGE]>> {
        Some(Escritura { datos: self.sectors.get_mut(n)?, paridades: &mut self.paridades[n] })
    }

    pub fn sector(&self, n: usize) -> &[u16; SHORTS_PER_PAGE] {
        &self.sectors[n]
    }

    pub fn sector_mut(&mut self, n: usize) -> Escritura<'_, [u16; SHORTS_PER_PAGE]> {
        self.get_mut(n).expect("sector de tambor existente")
    }

    pub(crate) fn fallo_paridad(&self, sector: usize) -> Option<usize> {
        self.sectors.get(sector)?.iter().zip(&self.paridades[sector])
            .position(|(&dato, &bit)| paridad(dato) != bit)
    }

    /// Diagnóstico del emulador. Cambia un bit de datos (0–9) o de paridad (10).
    pub fn invertir_bit(&mut self, sector: usize, corto: usize, bit: u8) -> bool {
        if sector >= self.len() || corto >= SHORTS_PER_PAGE {
            return false;
        }
        invertir_bit(&mut self.sectors[sector][corto], &mut self.paridades[sector][corto], bit)
    }
}

fn invertir_bit(dato: &mut u16, par: &mut bool, bit: u8) -> bool {
    match bit {
        0..=9 => *dato ^= 1 << bit,
        10 => *par = !*par,
        _ => return false,
    }
    true
}

/// Extiende el signo de un valor de `bits` bits.
pub fn sext(v: i64, bits: u32) -> i64 {
    let shift = 64 - bits;
    (v << shift) >> shift
}

/// Interpreta un corto de 10 bits como entero con signo (convención ±, §1.7.1).
pub fn signed10(v: u16) -> i32 {
    sext((v & SHORT_MASK) as i64, 10) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_systems_overlap_as_in_manual() {
        // §1.3: cortos 24..27 = medios 12, 13 = largo 6.
        let mut s = Store::default();
        s.set_long(6, [1, 2, 3, 4]);
        assert_eq!(s.short(24), 1);
        assert_eq!(s.short(27), 4);
        assert_eq!(s.medium(12), (1 << 10) | 2);
        assert_eq!(s.medium(13), (3 << 10) | 4);
    }

    #[test]
    fn signed_convention() {
        assert_eq!(signed10(7), 7);
        assert_eq!(signed10(511), 511);
        assert_eq!(signed10(512), -512);
        assert_eq!(signed10(1023), -1);
    }
}
