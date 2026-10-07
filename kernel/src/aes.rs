//! AES-256-GCM with AES-NI for the block cipher. GHASH is in-tree.
//! A miss of AES-NI is a closed book — no software AES lie.

use core::arch::asm;

const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const RCON: [u32; 8] = [
    0,
    0x0100_0000,
    0x0200_0000,
    0x0400_0000,
    0x0800_0000,
    0x1000_0000,
    0x2000_0000,
    0x4000_0000,
];

#[repr(align(16))]
struct RoundKeys {
    k: [[u8; 16]; 15],
}

#[repr(align(16))]
struct Block([u8; 16]);

#[repr(align(16))]
struct Fx([u8; 512]);

static mut FX: Fx = Fx([0; 512]);
static mut RK: RoundKeys = RoundKeys { k: [[0; 16]; 15] };
static mut BLK: Block = Block([0; 16]);

fn sub_word(x: u32) -> u32 {
    let b = x.to_be_bytes();
    u32::from_be_bytes([
        SBOX[b[0] as usize],
        SBOX[b[1] as usize],
        SBOX[b[2] as usize],
        SBOX[b[3] as usize],
    ])
}

fn rot_word(x: u32) -> u32 {
    x.rotate_left(8)
}

fn expand(key: &[u8; 32]) -> RoundKeys {
    let mut w = [0u32; 60];
    let mut i = 0usize;
    while i < 8 {
        w[i] = u32::from_be_bytes([key[i * 4], key[i * 4 + 1], key[i * 4 + 2], key[i * 4 + 3]]);
        i += 1;
    }
    while i < 60 {
        let mut t = w[i - 1];
        if i % 8 == 0 {
            t = sub_word(rot_word(t)) ^ RCON[i / 8];
        } else if i % 8 == 4 {
            t = sub_word(t);
        }
        w[i] = w[i - 8] ^ t;
        i += 1;
    }
    let mut rk = RoundKeys { k: [[0; 16]; 15] };
    i = 0;
    while i < 15 {
        let mut j = 0usize;
        while j < 4 {
            let b = w[i * 4 + j].to_be_bytes();
            rk.k[i][j * 4] = b[0];
            rk.k[i][j * 4 + 1] = b[1];
            rk.k[i][j * 4 + 2] = b[2];
            rk.k[i][j * 4 + 3] = b[3];
            j += 1;
        }
        i += 1;
    }
    rk
}

unsafe fn fxsave(fx: &mut Fx) {
    unsafe {
        asm!("fxsave64 [{p}]", p = in(reg) fx.0.as_mut_ptr(), options(nostack));
    }
}

unsafe fn fxrstor(fx: &Fx) {
    unsafe {
        asm!("fxrstor64 [{p}]", p = in(reg) fx.0.as_ptr(), options(nostack));
    }
}

unsafe fn encrypt_ni(rk: &RoundKeys, inp: &[u8; 16]) -> [u8; 16] {
    let out = core::ptr::addr_of_mut!(BLK);
    unsafe {
        (*out).0 = *inp;
        asm!(
            "movdqu xmm0, [{out}]",
            "movdqu xmm1, [{k}]",
            "pxor xmm0, xmm1",
            "movdqu xmm1, [{k} + 16]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 32]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 48]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 64]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 80]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 96]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 112]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 128]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 144]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 160]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 176]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 192]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 208]",
            "aesenc xmm0, xmm1",
            "movdqu xmm1, [{k} + 224]",
            "aesenclast xmm0, xmm1",
            "movdqu [{out}], xmm0",
            k = in(reg) rk.k.as_ptr(),
            out = in(reg) core::ptr::addr_of_mut!(BLK),
            out("xmm0") _,
            out("xmm1") _,
            options(nostack),
        );
        (*out).0
    }
}

fn encrypt(rk: &RoundKeys, inp: &[u8; 16]) -> [u8; 16] {
    unsafe { encrypt_ni(rk, inp) }
}

fn xor16(a: &mut [u8; 16], b: &[u8; 16]) {
    let mut i = 0usize;
    while i < 16 {
        a[i] ^= b[i];
        i += 1;
    }
}

fn shr_block(v: &mut [u8; 16]) {
    let mut i = 15usize;
    while i > 0 {
        v[i] = (v[i] >> 1) | ((v[i - 1] & 1) << 7);
        i -= 1;
    }
    v[0] >>= 1;
}

fn gmul(x: &[u8; 16], y: &[u8; 16]) -> [u8; 16] {
    let mut z = [0u8; 16];
    let mut v = *y;
    let mut i = 0usize;
    while i < 16 {
        let mut j = 0u8;
        while j < 8 {
            if x[i] & (0x80 >> j) != 0 {
                xor16(&mut z, &v);
            }
            let lsb = v[15] & 1;
            shr_block(&mut v);
            if lsb != 0 {
                v[0] ^= 0xE1;
            }
            j += 1;
        }
        i += 1;
    }
    z
}

fn inc32(b: &mut [u8; 16]) {
    let mut i = 15usize;
    loop {
        let (v, c) = b[i].overflowing_add(1);
        b[i] = v;
        if !c || i == 12 {
            break;
        }
        i -= 1;
    }
}

fn ghash(h: &[u8; 16], data: &[u8]) -> [u8; 16] {
    let mut x = [0u8; 16];
    let mut off = 0usize;
    while off < data.len() {
        let mut blk = [0u8; 16];
        let n = (data.len() - off).min(16);
        let mut i = 0usize;
        while i < n {
            blk[i] = data[off + i];
            i += 1;
        }
        xor16(&mut x, &blk);
        x = gmul(&x, h);
        off += n;
    }
    x
}

fn gcm_tag(rk: &RoundKeys, h: &[u8; 16], j0: &[u8; 16], ct: &[u8]) -> [u8; 16] {
    let mut x = ghash(h, ct);
    let mut lenblk = [0u8; 16];
    let cbits = (ct.len() as u64).wrapping_mul(8);
    let b = cbits.to_be_bytes();
    let mut i = 0usize;
    while i < 8 {
        lenblk[8 + i] = b[i];
        i += 1;
    }
    xor16(&mut x, &lenblk);
    x = gmul(&x, h);
    let mut t = encrypt(rk, j0);
    xor16(&mut t, &x);
    t
}

fn ctr_crypt(rk: &RoundKeys, nonce: &[u8; 12], src: &[u8], dst: &mut [u8]) {
    let mut ctr = [0u8; 16];
    let mut i = 0usize;
    while i < 12 {
        ctr[i] = nonce[i];
        i += 1;
    }
    ctr[15] = 1;
    inc32(&mut ctr);
    let mut off = 0usize;
    while off < src.len() {
        let ks = encrypt(rk, &ctr);
        let n = (src.len() - off).min(16);
        i = 0;
        while i < n {
            dst[off + i] = src[off + i] ^ ks[i];
            i += 1;
        }
        inc32(&mut ctr);
        off += n;
    }
}

/// AES-256-ECB of one block. None if AES-NI is missing.
#[allow(dead_code)]
pub fn ecb_block(key: &[u8; 32], block: &[u8; 16]) -> Option<[u8; 16]> {
    if !crate::cpu::has_aes_ni() {
        return None;
    }
    unsafe {
        fxsave(&mut *core::ptr::addr_of_mut!(FX));
        *core::ptr::addr_of_mut!(RK) = expand(key);
        let out = encrypt(&*core::ptr::addr_of!(RK), block);
        fxrstor(&*core::ptr::addr_of!(FX));
        Some(out)
    }
}

/// Seal `pt` into `ct` (same length). Returns the 16-byte tag.
pub fn gcm_seal(key: &[u8; 32], nonce: &[u8; 12], pt: &[u8], ct: &mut [u8]) -> Option<[u8; 16]> {
    if ct.len() != pt.len() || !crate::cpu::has_aes_ni() {
        return None;
    }
    unsafe {
        fxsave(&mut *core::ptr::addr_of_mut!(FX));
        *core::ptr::addr_of_mut!(RK) = expand(key);
        let rk = &*core::ptr::addr_of!(RK);
        let h = encrypt(rk, &[0u8; 16]);
        ctr_crypt(rk, nonce, pt, ct);
        let mut j0 = [0u8; 16];
        let mut i = 0usize;
        while i < 12 {
            j0[i] = nonce[i];
            i += 1;
        }
        j0[15] = 1;
        let tag = gcm_tag(rk, &h, &j0, ct);
        fxrstor(&*core::ptr::addr_of!(FX));
        Some(tag)
    }
}

/// Open `ct` into `pt`. False on tag mismatch (pt is not written).
pub fn gcm_open(
    key: &[u8; 32],
    nonce: &[u8; 12],
    ct: &[u8],
    tag: &[u8; 16],
    pt: &mut [u8],
) -> bool {
    if pt.len() != ct.len() || !crate::cpu::has_aes_ni() {
        return false;
    }
    unsafe {
        fxsave(&mut *core::ptr::addr_of_mut!(FX));
        *core::ptr::addr_of_mut!(RK) = expand(key);
        let rk = &*core::ptr::addr_of!(RK);
        let h = encrypt(rk, &[0u8; 16]);
        let mut j0 = [0u8; 16];
        let mut i = 0usize;
        while i < 12 {
            j0[i] = nonce[i];
            i += 1;
        }
        j0[15] = 1;
        let got = gcm_tag(rk, &h, &j0, ct);
        let mut d = 0u8;
        i = 0;
        while i < 16 {
            d |= got[i] ^ tag[i];
            i += 1;
        }
        if d != 0 {
            fxrstor(&*core::ptr::addr_of!(FX));
            return false;
        }
        ctr_crypt(rk, nonce, ct, pt);
        fxrstor(&*core::ptr::addr_of!(FX));
        true
    }
}
