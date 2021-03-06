use super::{
    AbcdVar, CompressionConfig, EfghVar, RoundWordA, RoundWordDense, RoundWordE, RoundWordSpread,
    State,
};
use crate::table16::{
    util::*, CellValue16, CellValue32, SpreadVar, SpreadWord, StateWord, Table16Assignment,
};
use halo2::{
    arithmetic::FieldExt,
    circuit::Region,
    plonk::{Advice, Column, Error},
};

// Test vector 'abc'
#[cfg(test)]
pub const COMPRESSION_OUTPUT: [u32; 8] = [
    0b10111010011110000001011010111111,
    0b10001111000000011100111111101010,
    0b01000001010000010100000011011110,
    0b01011101101011100010001000100011,
    0b10110000000000110110000110100011,
    0b10010110000101110111101010011100,
    0b10110100000100001111111101100001,
    0b11110010000000000001010110101101,
];

// Rows needed for each gate
pub const SIGMA_0_ROWS: usize = 4;
pub const SIGMA_1_ROWS: usize = 4;
pub const CH_ROWS: usize = 8;
pub const MAJ_ROWS: usize = 4;
pub const DECOMPOSE_ABCD: usize = 2;
pub const DECOMPOSE_EFGH: usize = 2;

// Rows needed for main subregion
pub const SUBREGION_MAIN_LEN: usize = 64;
pub const SUBREGION_MAIN_WORD: usize =
    DECOMPOSE_ABCD + SIGMA_0_ROWS + DECOMPOSE_EFGH + SIGMA_1_ROWS + CH_ROWS + MAJ_ROWS;
pub const SUBREGION_MAIN_ROWS: usize = SUBREGION_MAIN_LEN * SUBREGION_MAIN_WORD;

/// Returns starting row number of a compression round
pub fn get_round_row(round_idx: i32) -> usize {
    assert!(round_idx >= -1);
    assert!(round_idx < 64);
    if round_idx == -1 {
        // Init subregion
        0
    } else {
        // Main subregion
        (round_idx as usize) * SUBREGION_MAIN_WORD
    }
}

pub fn get_decompose_e_row(round_idx: i32) -> usize {
    get_round_row(round_idx)
}

pub fn get_decompose_f_row(round_idx: i32) -> usize {
    assert_eq!(round_idx, -1);
    get_decompose_e_row(round_idx) + DECOMPOSE_EFGH
}

pub fn get_decompose_g_row(round_idx: i32) -> usize {
    get_decompose_f_row(round_idx) + DECOMPOSE_EFGH
}

pub fn get_upper_sigma_1_row(round_idx: i32) -> usize {
    assert!(round_idx >= 0);
    get_decompose_e_row(round_idx) + DECOMPOSE_EFGH + 1
}

pub fn get_ch_row(round_idx: i32) -> usize {
    assert!(round_idx >= 0);
    get_decompose_e_row(round_idx) + DECOMPOSE_EFGH + SIGMA_1_ROWS + 1
}

pub fn get_ch_neg_row(round_idx: i32) -> usize {
    get_ch_row(round_idx) + CH_ROWS / 2
}

pub fn get_decompose_a_row(round_idx: i32) -> usize {
    if round_idx == -1 {
        get_h_row(round_idx) + DECOMPOSE_EFGH
    } else {
        get_ch_neg_row(round_idx) - 1 + CH_ROWS / 2
    }
}

pub fn get_upper_sigma_0_row(round_idx: i32) -> usize {
    assert!(round_idx >= 0);
    get_decompose_a_row(round_idx) + DECOMPOSE_ABCD + 1
}

pub fn get_decompose_b_row(round_idx: i32) -> usize {
    assert_eq!(round_idx, -1);
    get_decompose_a_row(round_idx) + DECOMPOSE_ABCD
}

pub fn get_decompose_c_row(round_idx: i32) -> usize {
    get_decompose_b_row(round_idx) + DECOMPOSE_ABCD
}

pub fn get_maj_row(round_idx: i32) -> usize {
    assert!(round_idx >= 0);
    get_upper_sigma_0_row(round_idx) + SIGMA_0_ROWS
}

// Get state word rows
pub fn get_h_row(round_idx: i32) -> usize {
    if round_idx == -1 {
        get_decompose_g_row(round_idx) + DECOMPOSE_EFGH
    } else {
        get_ch_row(round_idx) - 1
    }
}

pub fn get_h_prime_row(round_idx: i32) -> usize {
    assert!(round_idx >= 0);
    get_ch_row(round_idx)
}

pub fn get_d_row(round_idx: i32) -> usize {
    if round_idx == -1 {
        get_decompose_c_row(round_idx) + DECOMPOSE_ABCD
    } else {
        get_ch_row(round_idx) + 2
    }
}

pub fn get_e_new_row(round_idx: i32) -> usize {
    assert!(round_idx >= 0);
    get_d_row(round_idx)
}

pub fn get_a_new_row(round_idx: i32) -> usize {
    get_maj_row(round_idx)
}

pub fn get_digest_abcd_row() -> usize {
    SUBREGION_MAIN_ROWS
}

pub fn get_digest_efgh_row() -> usize {
    get_digest_abcd_row() + 2
}

impl CompressionConfig {
    pub(super) fn decompose_abcd<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        row: usize,
        a_val: Option<u32>,
    ) -> Result<
        (
            SpreadVar,
            SpreadVar,
            SpreadVar,
            SpreadVar,
            SpreadVar,
            SpreadVar,
        ),
        Error,
    > {
        region.assign_fixed(
            || "s_decompose_abcd",
            self.s_decompose_abcd,
            row,
            || Ok(F::one()),
        )?;

        let a_3 = self.extras[0];
        let a_4 = self.extras[1];
        let a_5 = self.message_schedule;
        let a_6 = self.extras[2];

        let a_spread_pieces = a_val.map(|a_val| {
            chop_u32(a_val, &[2, 11, 3, 3, 3, 10])
                .iter()
                .map(|piece| SpreadWord::new(*piece as u16))
                .collect::<Vec<_>>()
        });
        let a_spread_pieces = transpose_option_vec(a_spread_pieces, 6);

        let a = SpreadVar::without_lookup(region, a_3, row + 1, a_4, row + 1, a_spread_pieces[0])?;
        let b = SpreadVar::with_lookup(region, &self.lookup, row, a_spread_pieces[1])?;
        let c_lo = SpreadVar::without_lookup(region, a_3, row, a_4, row, a_spread_pieces[2])?;
        let c_mid = SpreadVar::without_lookup(region, a_5, row, a_6, row, a_spread_pieces[3])?;
        let c_hi =
            SpreadVar::without_lookup(region, a_5, row + 1, a_6, row + 1, a_spread_pieces[4])?;
        let d = SpreadVar::with_lookup(region, &self.lookup, row + 1, a_spread_pieces[5])?;

        Ok((a, b, c_lo, c_mid, c_hi, d))
    }

    pub(super) fn decompose_efgh<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        row: usize,
        val: Option<u32>,
    ) -> Result<
        (
            SpreadVar,
            SpreadVar,
            SpreadVar,
            SpreadVar,
            SpreadVar,
            SpreadVar,
        ),
        Error,
    > {
        region.assign_fixed(
            || "s_decompose_efgh",
            self.s_decompose_efgh,
            row,
            || Ok(F::one()),
        )?;

        let a_3 = self.extras[0];
        let a_4 = self.extras[1];
        let a_5 = self.message_schedule;
        let a_6 = self.extras[2];

        let spread_pieces = val.map(|val| {
            chop_u32(val, &[3, 3, 2, 3, 14, 7])
                .iter()
                .map(|piece| SpreadWord::new(*piece as u16))
                .collect::<Vec<_>>()
        });
        let spread_pieces = transpose_option_vec(spread_pieces, 6);

        let a_lo = SpreadVar::without_lookup(region, a_3, row + 1, a_4, row + 1, spread_pieces[0])?;
        let a_hi = SpreadVar::without_lookup(region, a_5, row + 1, a_6, row + 1, spread_pieces[1])?;
        let b_lo = SpreadVar::without_lookup(region, a_3, row, a_4, row, spread_pieces[2])?;
        let b_hi = SpreadVar::without_lookup(region, a_5, row, a_6, row, spread_pieces[3])?;
        let c = SpreadVar::with_lookup(region, &self.lookup, row + 1, spread_pieces[4])?;
        let d = SpreadVar::with_lookup(region, &self.lookup, row, spread_pieces[5])?;

        Ok((a_lo, a_hi, b_lo, b_hi, c, d))
    }

    pub(super) fn decompose_a<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        idx: i32,
        a_val: Option<u32>,
    ) -> Result<RoundWordA, Error> {
        let row = get_decompose_a_row(idx);

        let (dense_halves, spread_halves) = self.assign_word_halves(region, row, a_val)?;
        let (a, b, c_lo, c_mid, c_hi, d) = self.decompose_abcd(region, row, a_val)?;
        let a_pieces = AbcdVar {
            idx,
            val: a_val,
            a,
            b,
            c_lo,
            c_mid,
            c_hi,
            d,
        };
        Ok(RoundWordA::new(a_pieces, dense_halves, spread_halves))
    }

    pub(super) fn decompose_e<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        idx: i32,
        e_val: Option<u32>,
    ) -> Result<RoundWordE, Error> {
        let row = get_decompose_e_row(idx);

        let (dense_halves, spread_halves) = self.assign_word_halves(region, row, e_val)?;
        let (a_lo, a_hi, b_lo, b_hi, c, d) = self.decompose_efgh(region, row, e_val)?;
        let e_pieces = EfghVar {
            idx,
            val: e_val,
            a_lo,
            a_hi,
            b_lo,
            b_hi,
            c,
            d,
        };
        Ok(RoundWordE::new(e_pieces, dense_halves, spread_halves))
    }

    pub(super) fn assign_upper_sigma_0<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        idx: i32,
        word: AbcdVar,
    ) -> Result<(CellValue16, CellValue16), Error> {
        // Rename these here for ease of matching the gates to the specification.
        let a_3 = self.extras[0];
        let a_4 = self.extras[1];
        let a_5 = self.message_schedule;

        let row = get_upper_sigma_0_row(idx);

        region.assign_fixed(
            || "s_upper_sigma_0",
            self.s_upper_sigma_0,
            row,
            || Ok(F::one()),
        )?;

        // Assign `spread_a` and copy constraint
        self.assign_and_constrain(region, || "spread_a", a_3, row + 1, word.a.spread)?;
        // Assign `spread_b` and copy constraint
        self.assign_and_constrain(region, || "spread_b", a_5, row, word.b.spread)?;
        // Assign `spread_c_lo` and copy constraint
        self.assign_and_constrain(region, || "spread_c_lo", a_3, row - 1, word.c_lo.spread)?;
        // Assign `spread_c_mid` and copy constraint
        self.assign_and_constrain(region, || "spread_c_mid", a_4, row - 1, word.c_mid.spread)?;
        // Assign `spread_c_hi` and copy constraint
        self.assign_and_constrain(region, || "spread_c_hi", a_4, row + 1, word.c_hi.spread)?;
        // Assign `spread_d` and copy constraint
        self.assign_and_constrain(region, || "spread_d", a_4, row, word.d.spread)?;

        // Calculate R_0^{even}, R_0^{odd}, R_1^{even}, R_1^{odd}
        let (r_0_even, r_0_odd, r_1_even, r_1_odd) = if word.a.spread.value.is_some() {
            let spread_a = word.a.spread.value.unwrap() as u64;
            let spread_b = word.b.spread.value.unwrap() as u64;
            let spread_c_lo = word.c_lo.spread.value.unwrap() as u64;
            let spread_c_mid = word.c_mid.spread.value.unwrap() as u64;
            let spread_c_hi = word.c_hi.spread.value.unwrap() as u64;
            let spread_d = word.d.spread.value.unwrap() as u64;

            let xor_0 = spread_b
                + (1 << 22) * spread_c_lo
                + (1 << 28) * spread_c_mid
                + (1 << 34) * spread_c_hi
                + (1 << 40) * spread_d
                + (1 << 60) * spread_a;
            let xor_1 = spread_c_lo
                + (1 << 6) * spread_c_mid
                + (1 << 12) * spread_c_hi
                + (1 << 18) * spread_d
                + (1 << 38) * spread_a
                + (1 << 42) * spread_b;
            let xor_2 = spread_d
                + (1 << 20) * spread_a
                + (1 << 24) * spread_b
                + (1 << 46) * spread_c_lo
                + (1 << 52) * spread_c_mid
                + (1 << 58) * spread_c_hi;
            let r = xor_0 + xor_1 + xor_2;
            let r_pieces = chop_u64(r, &[32, 32]); // r_0, r_1
            let (r_0_even, r_0_odd) = get_even_and_odd_bits_u32(r_pieces[0] as u32);
            let (r_1_even, r_1_odd) = get_even_and_odd_bits_u32(r_pieces[1] as u32);

            (Some(r_0_even), Some(r_0_odd), Some(r_1_even), Some(r_1_odd))
        } else {
            (None, None, None, None)
        };

        self.assign_sigma_outputs(
            region,
            &self.lookup,
            a_3,
            row,
            r_0_even,
            r_0_odd,
            r_1_even,
            r_1_odd,
        )
    }

    pub(super) fn assign_upper_sigma_1<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        idx: i32,
        word: EfghVar,
    ) -> Result<(CellValue16, CellValue16), Error> {
        // Rename these here for ease of matching the gates to the specification.
        let a_3 = self.extras[0];
        let a_4 = self.extras[1];
        let a_5 = self.message_schedule;

        let row = get_upper_sigma_1_row(idx);

        region.assign_fixed(
            || "s_upper_sigma_1",
            self.s_upper_sigma_1,
            row,
            || Ok(F::one()),
        )?;

        // Assign `spread_a_lo` and copy constraint
        self.assign_and_constrain(region, || "spread_a_lo", a_3, row + 1, word.a_lo.spread)?;
        // Assign `spread_a_hi` and copy constraint
        self.assign_and_constrain(region, || "spread_a_hi", a_4, row + 1, word.a_hi.spread)?;
        // Assign `spread_b_lo` and copy constraint
        self.assign_and_constrain(region, || "spread_b_lo", a_3, row - 1, word.b_lo.spread)?;
        // Assign `spread_b_hi` and copy constraint
        self.assign_and_constrain(region, || "spread_b_hi", a_4, row - 1, word.b_hi.spread)?;
        // Assign `spread_c` and copy constraint
        self.assign_and_constrain(region, || "spread_c", a_5, row, word.c.spread)?;
        // Assign `spread_d` and copy constraint
        self.assign_and_constrain(region, || "spread_d", a_4, row, word.d.spread)?;

        // Calculate R_0^{even}, R_0^{odd}, R_1^{even}, R_1^{odd}
        let (r_0_even, r_0_odd, r_1_even, r_1_odd) = if word.a_lo.spread.value.is_some() {
            let spread_a_lo = word.a_lo.spread.value.unwrap() as u64;
            let spread_a_hi = word.a_hi.spread.value.unwrap() as u64;
            let spread_b_lo = word.b_lo.spread.value.unwrap() as u64;
            let spread_b_hi = word.b_hi.spread.value.unwrap() as u64;
            let spread_c = word.c.spread.value.unwrap() as u64;
            let spread_d = word.d.spread.value.unwrap() as u64;

            let xor_0 = spread_b_lo
                + (1 << 4) * spread_b_hi
                + (1 << 10) * spread_c
                + (1 << 38) * spread_d
                + (1 << 52) * spread_a_lo
                + (1 << 58) * spread_a_hi;
            let xor_1 = spread_c
                + (1 << 28) * spread_d
                + (1 << 42) * spread_a_lo
                + (1 << 48) * spread_a_hi
                + (1 << 54) * spread_b_lo
                + (1 << 58) * spread_b_hi;
            let xor_2 = spread_d
                + (1 << 14) * spread_a_lo
                + (1 << 20) * spread_a_hi
                + (1 << 26) * spread_b_lo
                + (1 << 30) * spread_b_hi
                + (1 << 36) * spread_c;
            let r = xor_0 + xor_1 + xor_2;
            let r_pieces = chop_u64(r, &[32, 32]); // r_0, r_1
            let (r_0_even, r_0_odd) = get_even_and_odd_bits_u32(r_pieces[0] as u32);
            let (r_1_even, r_1_odd) = get_even_and_odd_bits_u32(r_pieces[1] as u32);

            (Some(r_0_even), Some(r_0_odd), Some(r_1_even), Some(r_1_odd))
        } else {
            (None, None, None, None)
        };

        self.assign_sigma_outputs(
            region,
            &self.lookup,
            a_3,
            row,
            r_0_even,
            r_0_odd,
            r_1_even,
            r_1_odd,
        )
    }

    fn assign_ch_outputs<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        row: usize,
        r_0_even: Option<u16>,
        r_0_odd: Option<u16>,
        r_1_even: Option<u16>,
        r_1_odd: Option<u16>,
    ) -> Result<(CellValue16, CellValue16), Error> {
        let a_3 = self.extras[0];

        let (_even, odd) = self.assign_spread_outputs(
            region,
            &self.lookup,
            a_3,
            row,
            r_0_even,
            r_0_odd,
            r_1_even,
            r_1_odd,
        )?;

        Ok(odd)
    }

    pub(super) fn assign_ch<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        idx: i32,
        spread_halves_e: (CellValue32, CellValue32),
        spread_halves_f: (CellValue32, CellValue32),
    ) -> Result<(CellValue16, CellValue16), Error> {
        let a_3 = self.extras[0];
        let a_4 = self.extras[1];

        let row = get_ch_row(idx);

        region.assign_fixed(|| "s_ch", self.s_ch, row, || Ok(F::one()))?;

        // Assign and copy spread_e_lo, spread_e_hi
        self.assign_and_constrain(region, || "spread_e_lo", a_3, row - 1, spread_halves_e.0)?;
        self.assign_and_constrain(region, || "spread_e_hi", a_4, row - 1, spread_halves_e.1)?;

        // Assign and copy spread_f_lo, spread_f_hi
        self.assign_and_constrain(region, || "spread_f_lo", a_3, row + 1, spread_halves_f.0)?;
        self.assign_and_constrain(region, || "spread_f_hi", a_4, row + 1, spread_halves_f.1)?;

        let (p0_even, p0_odd, p1_even, p1_odd) = if spread_halves_e.0.value.is_some() {
            let p: u64 = spread_halves_e.0.value.unwrap() as u64
                + spread_halves_f.0.value.unwrap() as u64
                + (1 << 32) * (spread_halves_e.1.value.unwrap() as u64)
                + (1 << 32) * (spread_halves_f.1.value.unwrap() as u64);
            let p_pieces = chop_u64(p, &[32, 32]); // p_0, p_1

            let (p0_even, p0_odd) = get_even_and_odd_bits_u32(p_pieces[0] as u32);
            let (p1_even, p1_odd) = get_even_and_odd_bits_u32(p_pieces[1] as u32);

            (Some(p0_even), Some(p0_odd), Some(p1_even), Some(p1_odd))
        } else {
            (None, None, None, None)
        };

        self.assign_ch_outputs(region, row, p0_even, p0_odd, p1_even, p1_odd)
    }

    pub(super) fn assign_ch_neg<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        idx: i32,
        spread_halves_e: (CellValue32, CellValue32),
        spread_halves_g: (CellValue32, CellValue32),
    ) -> Result<(CellValue16, CellValue16), Error> {
        let row = get_ch_neg_row(idx);

        region.assign_fixed(|| "s_ch_neg", self.s_ch_neg, row, || Ok(F::one()))?;

        let a_3 = self.extras[0];
        let a_4 = self.extras[1];
        let a_5 = self.message_schedule;

        // Assign and copy spread_e_lo, spread_e_hi
        self.assign_and_constrain(region, || "spread_e_lo", a_5, row - 1, spread_halves_e.0)?;
        self.assign_and_constrain(region, || "spread_e_hi", a_5, row, spread_halves_e.1)?;

        // Assign and copy spread_g_lo, spread_g_hi
        self.assign_and_constrain(region, || "spread_g_lo", a_3, row + 1, spread_halves_g.0)?;
        self.assign_and_constrain(region, || "spread_g_hi", a_4, row + 1, spread_halves_g.1)?;

        // Calculate neg_e_lo, neg_e_hi
        let spread_neg_e_lo = spread_halves_e
            .0
            .value
            .map(|spread_e_lo| (MASK_EVEN_32 - spread_e_lo) as u64);
        let spread_neg_e_hi = spread_halves_e
            .1
            .value
            .map(|spread_e_hi| (MASK_EVEN_32 - spread_e_hi) as u64);

        // Assign spread_neg_e_lo, spread_neg_e_hi
        region.assign_advice(
            || "spread_neg_e_lo",
            a_3,
            row - 1,
            || {
                spread_neg_e_lo
                    .map(F::from_u64)
                    .ok_or(Error::SynthesisError)
            },
        )?;
        region.assign_advice(
            || "spread_neg_e_hi",
            a_4,
            row - 1,
            || {
                spread_neg_e_hi
                    .map(F::from_u64)
                    .ok_or(Error::SynthesisError)
            },
        )?;

        let (p0_even, p0_odd, p1_even, p1_odd) = if let Some(spread_neg_e_lo) = spread_neg_e_lo {
            let p: u64 = spread_neg_e_lo as u64
                + spread_halves_g.0.value.unwrap() as u64
                + (1 << 32) * spread_neg_e_hi.unwrap() as u64
                + (1 << 32) * (spread_halves_g.1.value.unwrap() as u64);
            let p_pieces = chop_u64(p, &[32, 32]); // p_0, p_1

            let (p0_even, p0_odd) = get_even_and_odd_bits_u32(p_pieces[0] as u32);
            let (p1_even, p1_odd) = get_even_and_odd_bits_u32(p_pieces[1] as u32);

            (Some(p0_even), Some(p0_odd), Some(p1_even), Some(p1_odd))
        } else {
            (None, None, None, None)
        };

        self.assign_ch_outputs(region, row, p0_even, p0_odd, p1_even, p1_odd)
    }

    fn assign_maj_outputs<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        row: usize,
        r_0_even: Option<u16>,
        r_0_odd: Option<u16>,
        r_1_even: Option<u16>,
        r_1_odd: Option<u16>,
    ) -> Result<(CellValue16, CellValue16), Error> {
        let a_3 = self.extras[0];
        let (_even, odd) = self.assign_spread_outputs(
            region,
            &self.lookup,
            a_3,
            row,
            r_0_even,
            r_0_odd,
            r_1_even,
            r_1_odd,
        )?;

        Ok(odd)
    }

    pub(super) fn assign_maj<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        idx: i32,
        spread_halves_a: (CellValue32, CellValue32),
        spread_halves_b: (CellValue32, CellValue32),
        spread_halves_c: (CellValue32, CellValue32),
    ) -> Result<(CellValue16, CellValue16), Error> {
        let a_4 = self.extras[1];
        let a_5 = self.message_schedule;

        let row = get_maj_row(idx);

        region.assign_fixed(|| "s_maj", self.s_maj, row, || Ok(F::one()))?;

        // Assign and copy spread_a_lo, spread_a_hi
        self.assign_and_constrain(region, || "spread_a_lo", a_4, row - 1, spread_halves_a.0)?;
        self.assign_and_constrain(region, || "spread_a_hi", a_5, row - 1, spread_halves_a.1)?;

        // Assign and copy spread_b_lo, spread_b_hi
        self.assign_and_constrain(region, || "spread_b_lo", a_4, row, spread_halves_b.0)?;
        self.assign_and_constrain(region, || "spread_b_hi", a_5, row, spread_halves_b.1)?;

        // Assign and copy spread_c_lo, spread_c_hi
        self.assign_and_constrain(region, || "spread_c_lo", a_4, row + 1, spread_halves_c.0)?;
        self.assign_and_constrain(region, || "spread_c_hi", a_5, row + 1, spread_halves_c.1)?;

        let (m0_even, m0_odd, m1_even, m1_odd) = if spread_halves_a.0.value.is_some() {
            let m: u64 = spread_halves_a.0.value.unwrap() as u64
                + spread_halves_b.0.value.unwrap() as u64
                + spread_halves_c.0.value.unwrap() as u64
                + (1 << 32) * (spread_halves_a.1.value.unwrap() as u64)
                + (1 << 32) * (spread_halves_b.1.value.unwrap() as u64)
                + (1 << 32) * (spread_halves_c.1.value.unwrap() as u64);
            let m_pieces = chop_u64(m, &[32, 32]); // m_0, m_1

            let (m0_even, m0_odd) = get_even_and_odd_bits_u32(m_pieces[0] as u32);
            let (m1_even, m1_odd) = get_even_and_odd_bits_u32(m_pieces[1] as u32);

            (Some(m0_even), Some(m0_odd), Some(m1_even), Some(m1_odd))
        } else {
            (None, None, None, None)
        };

        self.assign_maj_outputs(region, row, m0_even, m0_odd, m1_even, m1_odd)
    }

    // s_h_prime to get H' = H + Ch(E, F, G) + s_upper_sigma_1(E) + K + W
    #[allow(clippy::too_many_arguments)]
    pub(super) fn assign_h_prime<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        idx: i32,
        h: (CellValue16, CellValue16),
        ch: (CellValue16, CellValue16),
        ch_neg: (CellValue16, CellValue16),
        sigma_1: (CellValue16, CellValue16),
        k: u32,
        w: (CellValue16, CellValue16),
    ) -> Result<(CellValue16, CellValue16), Error> {
        let row = get_h_prime_row(idx);
        region.assign_fixed(|| "s_h_prime", self.s_h_prime, row, || Ok(F::one()))?;

        let a_4 = self.extras[1];
        let a_5 = self.message_schedule;
        let a_6 = self.extras[2];
        let a_7 = self.extras[3];
        let a_8 = self.extras[4];
        let a_9 = self.extras[5];

        // Assign and copy h
        self.assign_and_constrain(region, || "h_lo", a_7, row - 1, h.0)?;
        self.assign_and_constrain(region, || "h_hi", a_7, row, h.1)?;

        // Assign and copy sigma_1
        self.assign_and_constrain(region, || "sigma_1_lo", a_4, row, sigma_1.0)?;
        self.assign_and_constrain(region, || "sigma_1_hi", a_5, row, sigma_1.1)?;

        // Assign k
        let k_pieces = chop_u32(k, &[16, 16]);
        region.assign_advice(
            || "k_lo",
            a_6,
            row - 1,
            || Ok(F::from_u64(k_pieces[0] as u64)),
        )?;
        region.assign_advice(|| "k_hi", a_6, row, || Ok(F::from_u64(k_pieces[1] as u64)))?;

        // Assign and copy w
        self.assign_and_constrain(region, || "w_lo", a_8, row - 1, w.0)?;
        self.assign_and_constrain(region, || "w_hi", a_8, row, w.1)?;

        // Assign and copy ch
        self.assign_and_constrain(region, || "ch_neg_hi", a_6, row + 1, ch.1)?;

        // Assign and copy ch_neg
        self.assign_and_constrain(region, || "ch_neg_lo", a_5, row - 1, ch_neg.0)?;
        self.assign_and_constrain(region, || "ch_neg_hi", a_5, row + 1, ch_neg.1)?;

        // Assign h_prime_lo, h_prime_hi, h_prime_carry
        let (h_prime, h_prime_carry) = sum_with_carry(vec![
            (h.0.value, h.1.value),
            (ch.0.value, ch.1.value),
            (ch_neg.0.value, ch_neg.1.value),
            (sigma_1.0.value, sigma_1.1.value),
            (Some(k_pieces[0] as u16), Some(k_pieces[1] as u16)),
            (w.0.value, w.1.value),
        ]);
        let h_prime_halves = h_prime.map(|h_prime| chop_u32(h_prime as u32, &[16, 16]));
        let (h_prime_lo, h_prime_hi) = (
            h_prime_halves.clone().map(|halves| halves[0] as u16),
            h_prime_halves.map(|halves| halves[1] as u16),
        );

        let h_prime_lo = {
            let cell = region.assign_advice(
                || "h_prime_lo",
                a_7,
                row + 1,
                || {
                    h_prime_lo
                        .map(|value| F::from_u64(value as u64))
                        .ok_or(Error::SynthesisError)
                },
            )?;
            CellValue16::new(cell, h_prime_lo)
        };
        let h_prime_hi = {
            let cell = region.assign_advice(
                || "h_prime_hi",
                a_8,
                row + 1,
                || {
                    h_prime_hi
                        .map(|value| F::from_u64(value as u64))
                        .ok_or(Error::SynthesisError)
                },
            )?;
            CellValue16::new(cell, h_prime_hi)
        };

        region.assign_advice(
            || "h_prime_carry",
            a_9,
            row + 1,
            || {
                h_prime_carry
                    .map(|value| F::from_u64(value as u64))
                    .ok_or(Error::SynthesisError)
            },
        )?;

        Ok((h_prime_lo, h_prime_hi))
    }

    // s_e_new to get E_new = H' + D
    pub(super) fn assign_e_new<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        idx: i32,
        d: (CellValue16, CellValue16),
        h_prime: (CellValue16, CellValue16),
    ) -> Result<(CellValue16, CellValue16), Error> {
        let row = get_e_new_row(idx);

        region.assign_fixed(|| "s_e_new", self.s_e_new, row, || Ok(F::one()))?;

        let a_7 = self.extras[3];
        let a_8 = self.extras[4];
        let a_9 = self.extras[5];

        // Assign and copy d_lo, d_hi
        self.assign_and_constrain(region, || "d_lo", a_7, row, d.0)?;
        self.assign_and_constrain(region, || "d_hi", a_7, row + 1, d.1)?;

        // Assign e_new, e_new_carry
        let (e_new, e_new_carry) = sum_with_carry(vec![
            (h_prime.0.value, h_prime.1.value),
            (d.0.value, d.1.value),
        ]);

        let e_new_dense = self.assign_word_halves_dense(region, row, a_8, row + 1, a_8, e_new)?;
        region.assign_advice(
            || "e_new_carry",
            a_9,
            row + 1,
            || e_new_carry.map(F::from_u64).ok_or(Error::SynthesisError),
        )?;

        Ok(e_new_dense)
    }

    // s_a_new to get A_new = H' + Maj(A, B, C) + s_upper_sigma_0(A)
    pub(super) fn assign_a_new<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        idx: i32,
        maj: (CellValue16, CellValue16),
        sigma_0: (CellValue16, CellValue16),
        h_prime: (CellValue16, CellValue16),
    ) -> Result<(CellValue16, CellValue16), Error> {
        let row = get_a_new_row(idx);

        region.assign_fixed(|| "s_a_new", self.s_a_new, row, || Ok(F::one()))?;

        let a_3 = self.extras[0];
        let a_6 = self.extras[2];
        let a_7 = self.extras[3];
        let a_8 = self.extras[4];
        let a_9 = self.extras[5];

        // Assign and copy maj_1
        self.assign_and_constrain(region, || "maj_1_hi", a_3, row - 1, maj.1)?;

        // Assign and copy sigma_0
        self.assign_and_constrain(region, || "sigma_0_lo", a_6, row, sigma_0.0)?;
        self.assign_and_constrain(region, || "sigma_0_hi", a_6, row + 1, sigma_0.1)?;

        // Assign and copy h_prime
        self.assign_and_constrain(region, || "h_prime_lo", a_7, row - 1, h_prime.0)?;
        self.assign_and_constrain(region, || "h_prime_hi", a_8, row - 1, h_prime.1)?;

        // Assign a_new, a_new_carry
        let (a_new, a_new_carry) = sum_with_carry(vec![
            (h_prime.0.value, h_prime.1.value),
            (sigma_0.0.value, sigma_0.1.value),
            (maj.0.value, maj.1.value),
        ]);

        let a_new_dense = self.assign_word_halves_dense(region, row, a_8, row + 1, a_8, a_new)?;
        region.assign_advice(
            || "a_new_carry",
            a_9,
            row,
            || a_new_carry.map(F::from_u64).ok_or(Error::SynthesisError),
        )?;

        Ok(a_new_dense)
    }

    pub fn assign_word_halves_dense<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        lo_row: usize,
        lo_col: Column<Advice>,
        hi_row: usize,
        hi_col: Column<Advice>,
        word: Option<u32>,
    ) -> Result<(CellValue16, CellValue16), Error> {
        let (lo, hi) = if let Some(word) = word {
            let halves = chop_u32(word, &[16, 16]);
            (Some(halves[0] as u16), Some(halves[1] as u16))
        } else {
            (None, None)
        };
        let lo = {
            let cell = region.assign_advice(
                || "lo",
                lo_col,
                lo_row,
                || {
                    lo.map(|lo| F::from_u64(lo as u64))
                        .ok_or(Error::SynthesisError)
                },
            )?;
            CellValue16::new(cell, lo)
        };
        let hi = {
            let cell = region.assign_advice(
                || "hi",
                hi_col,
                hi_row,
                || {
                    hi.map(|hi| F::from_u64(hi as u64))
                        .ok_or(Error::SynthesisError)
                },
            )?;
            CellValue16::new(cell, hi)
        };

        Ok((lo, hi))
    }

    // Assign hi and lo halves for both dense and spread versions of a word
    #[allow(clippy::type_complexity)]
    pub fn assign_word_halves<F: FieldExt>(
        &self,
        region: &mut Region<'_, F>,
        row: usize,
        word: Option<u32>,
    ) -> Result<((CellValue16, CellValue16), (CellValue32, CellValue32)), Error> {
        // Rename these here for ease of matching the gates to the specification.
        let a_7 = self.extras[3];
        let a_8 = self.extras[4];

        let halves = word.map(|word| chop_u32(word, &[16, 16]));
        let halves = transpose_option_vec(halves, 2);
        let w_lo = SpreadWord::opt_new(halves[0].map(|value| value as u16));
        let w_hi = SpreadWord::opt_new(halves[1].map(|value| value as u16));

        let w_lo = SpreadVar::without_lookup(region, a_7, row, a_8, row, w_lo)?;
        let w_hi = SpreadVar::without_lookup(region, a_7, row + 1, a_8, row + 1, w_hi)?;

        let w_lo_cell = CellValue16::new(w_lo.dense.var, w_lo.dense.value);
        let w_hi_cell = CellValue16::new(w_hi.dense.var, w_hi.dense.value);
        let spread_w_lo_cell = CellValue32::new(w_lo.spread.var, w_lo.spread.value);
        let spread_w_hi_cell = CellValue32::new(w_hi.spread.var, w_hi.spread.value);

        Ok(((w_lo_cell, w_hi_cell), (spread_w_lo_cell, spread_w_hi_cell)))
    }
}

pub fn val_from_dense_halves(dense_halves: (CellValue16, CellValue16)) -> Option<u32> {
    dense_halves
        .0
        .value
        .zip(dense_halves.1.value)
        .map(|(lo, hi)| lo as u32 + (1 << 16) * hi as u32)
}

#[allow(clippy::many_single_char_names)]
pub fn match_state(
    state: State,
) -> (
    RoundWordA,
    RoundWordSpread,
    RoundWordSpread,
    RoundWordDense,
    RoundWordE,
    RoundWordSpread,
    RoundWordSpread,
    RoundWordDense,
) {
    let a = match state.a {
        Some(StateWord::A(a)) => a,
        _ => unreachable!(),
    };
    let b = match state.b {
        Some(StateWord::B(b)) => b,
        _ => unreachable!(),
    };
    let c = match state.c {
        Some(StateWord::C(c)) => c,
        _ => unreachable!(),
    };
    let d = match state.d {
        Some(StateWord::D(d)) => d,
        _ => unreachable!(),
    };
    let e = match state.e {
        Some(StateWord::E(e)) => e,
        _ => unreachable!(),
    };
    let f = match state.f {
        Some(StateWord::F(f)) => f,
        _ => unreachable!(),
    };
    let g = match state.g {
        Some(StateWord::G(g)) => g,
        _ => unreachable!(),
    };
    let h = match state.h {
        Some(StateWord::H(h)) => h,
        _ => unreachable!(),
    };

    (a, b, c, d, e, f, g, h)
}
