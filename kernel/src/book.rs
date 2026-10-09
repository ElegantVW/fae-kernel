//! The book — keepers, sealed twice. House calls 11–14.
//!
//! Leaves `hands` and `twin` are the same plaintext roster, wrapped under
//! different keys. Ciphertexts must not compare equal. A split book refuses
//! the hall. A lone leaf, or wax on either side, is an empty book.

use crate::start::serial_print;

pub const ROLL: u64 = 11;
pub const ENLIST: u64 = 12;
pub const CHOOSE: u64 = 13;
pub const DISMISS: u64 = 14;

pub const SEAL_LEN: usize = 716;
const PLAIN_LEN: usize = 688;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const SLOTS: usize = 8;
const NAME_LEN: usize = 32;
const SALT_LEN: usize = 16;
const HASH_LEN: usize = 32;
const FLAG_LIVE: u8 = 1;
const MAGIC: [u8; 4] = *b"KHB1";
const WORD_MAX: usize = 63;

const EPERM: u64 = 1;
const ENOENT: u64 = 2;
const EIO: u64 = 5;
const EEXIST: u64 = 17;
const ENODEV: u64 = 19;
const EINVAL: u64 = 22;

fn err(n: u64) -> u64 {
    0u64.wrapping_sub(n)
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Slot {
    flags: u8,
    _pad: [u8; 3],
    name: [u8; NAME_LEN],
    salt: [u8; SALT_LEN],
    verifier: [u8; HASH_LEN],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Book {
    magic: [u8; 4],
    vol: u32,
    slots: [Slot; SLOTS],
    _pad: [u8; 8],
}

const _: () = assert!(core::mem::size_of::<Slot>() == 84);
const _: () = assert!(core::mem::size_of::<Book>() == PLAIN_LEN);
const _: () = assert!(NONCE_LEN + PLAIN_LEN + TAG_LEN == SEAL_LEN);

static HANDS_NAME: [u8; 6] = *b"hands\0";
static TWIN_NAME: [u8; 5] = *b"twin\0";
static HAND_NAME: [u8; 5] = *b"hand\0";

static mut HANDS_SEAL: [u8; SEAL_LEN] = [0; SEAL_LEN];
static mut TWIN_SEAL: [u8; SEAL_LEN] = [0; SEAL_LEN];
static mut PLAIN: Book = Book {
    magic: [0; 4],
    vol: 0,
    slots: [Slot {
        flags: 0,
        _pad: [0; 3],
        name: [0; NAME_LEN],
        salt: [0; SALT_LEN],
        verifier: [0; HASH_LEN],
    }; SLOTS],
    _pad: [0; 8],
};
static mut PLAIN2: Book = Book {
    magic: [0; 4],
    vol: 0,
    slots: [Slot {
        flags: 0,
        _pad: [0; 3],
        name: [0; NAME_LEN],
        salt: [0; SALT_LEN],
        verifier: [0; HASH_LEN],
    }; SLOTS],
    _pad: [0; 8],
};
static mut NONCE_SEQ: u64 = 1;

fn speak(s: &'static str) {
    serial_print(s);
    crate::glass::put_bytes(s.as_ptr(), s.len() as u64);
}

fn split() -> u64 {
    speak("the book is split\n");
    err(EIO)
}

fn shut() -> u64 {
    speak("the book stays shut\n");
    err(ENODEV)
}

fn all_zero(b: &[u8]) -> bool {
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != 0 {
            return false;
        }
        i += 1;
    }
    true
}

fn eq_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut d = 0u8;
    let mut i = 0usize;
    while i < a.len() {
        d |= a[i] ^ b[i];
        i += 1;
    }
    d == 0
}

fn copy_bytes(dst: &mut [u8], src: &[u8]) {
    let n = dst.len().min(src.len());
    let mut i = 0usize;
    while i < n {
        dst[i] = src[i];
        i += 1;
    }
}

fn zero_bytes(dst: &mut [u8]) {
    let mut i = 0usize;
    while i < dst.len() {
        dst[i] = 0;
        i += 1;
    }
}

fn as_bytes(book: &Book) -> &[u8; PLAIN_LEN] {
    unsafe { &*(book as *const Book as *const [u8; PLAIN_LEN]) }
}

fn as_bytes_mut(book: &mut Book) -> &mut [u8; PLAIN_LEN] {
    unsafe { &mut *(book as *mut Book as *mut [u8; PLAIN_LEN]) }
}

fn take_cstr(ptr: u64, cap: usize) -> Result<([u8; 64], usize), u64> {
    if ptr == 0 || cap == 0 || cap > 63 {
        return Err(EPERM);
    }
    let mut buf = [0u8; 64];
    let mut n = 0usize;
    loop {
        let b = unsafe { ((ptr + n as u64) as *const u8).read_volatile() };
        if b == 0 {
            break;
        }
        if n >= cap || !(0x20..=0x7E).contains(&b) {
            return Err(EPERM);
        }
        buf[n] = b;
        n += 1;
    }
    if n == 0 {
        return Err(EINVAL);
    }
    Ok((buf, n))
}

fn glean_leaf(name: &[u8], buf: &mut [u8]) -> Result<u64, u64> {
    let n = crate::cairn::glean(
        name.as_ptr() as u64,
        buf.as_mut_ptr() as u64,
        buf.len() as u64,
    );
    if (n as i64) < 0 {
        Err(n.wrapping_neg())
    } else {
        Ok(n)
    }
}

fn stow_leaf(name: &[u8], buf: &[u8]) -> Result<u64, u64> {
    let n = crate::cairn::stow(name.as_ptr() as u64, buf.as_ptr() as u64, buf.len() as u64);
    if (n as i64) < 0 {
        Err(n.wrapping_neg())
    } else {
        Ok(n)
    }
}

fn wrap_key(label: &[u8], vol: u32) -> [u8; 32] {
    let mut h = crate::sha256::Hasher::new();
    h.update(label);
    h.update(&vol.to_le_bytes());
    h.finish()
}

fn verifier(salt: &[u8; SALT_LEN], word: &[u8], vol: u32, name: &[u8; NAME_LEN]) -> [u8; HASH_LEN] {
    let mut h = crate::sha256::Hasher::new();
    h.update(salt);
    h.update(word);
    h.update(&vol.to_le_bytes());
    h.update(name);
    h.finish()
}

fn rdrand_fill(dst: &mut [u8]) -> bool {
    let mut off = 0usize;
    while off < dst.len() {
        let Some(v) = crate::cpu::rdrand_u64() else {
            return false;
        };
        let b = v.to_le_bytes();
        let mut i = 0usize;
        while i < 8 && off < dst.len() {
            dst[off] = b[i];
            off += 1;
            i += 1;
        }
    }
    true
}

fn fresh_nonce(out: &mut [u8; NONCE_LEN]) -> bool {
    if !rdrand_fill(out) {
        return false;
    }
    let seq = unsafe {
        let s = core::ptr::addr_of!(NONCE_SEQ).read();
        core::ptr::addr_of_mut!(NONCE_SEQ).write(s.wrapping_add(1));
        s
    };
    let b = seq.to_le_bytes();
    out[8] ^= b[0];
    out[9] ^= b[1];
    out[10] ^= b[2];
    out[11] ^= b[3];
    true
}

enum Load {
    Empty,
    Live(Book),
}

fn unseal_one(seal: &[u8; SEAL_LEN], key: &[u8; 32], dst: &mut Book) -> bool {
    let mut nonce = [0u8; NONCE_LEN];
    copy_bytes(&mut nonce, &seal[..NONCE_LEN]);
    let mut tag = [0u8; TAG_LEN];
    copy_bytes(&mut tag, &seal[NONCE_LEN + PLAIN_LEN..]);
    let ct = &seal[NONCE_LEN..NONCE_LEN + PLAIN_LEN];
    let pt = as_bytes_mut(dst);
    crate::aes::gcm_open(key, &nonce, ct, &tag, pt)
}

fn seal_one(book: &Book, key: &[u8; 32], out: &mut [u8; SEAL_LEN]) -> bool {
    let mut nonce = [0u8; NONCE_LEN];
    if !fresh_nonce(&mut nonce) {
        return false;
    }
    let pt = as_bytes(book);
    let tag = {
        let ct = &mut out[NONCE_LEN..NONCE_LEN + PLAIN_LEN];
        match crate::aes::gcm_seal(key, &nonce, pt, ct) {
            Some(t) => t,
            None => return false,
        }
    };
    copy_bytes(&mut out[..NONCE_LEN], &nonce);
    copy_bytes(&mut out[NONCE_LEN + PLAIN_LEN..], &tag);
    true
}

/// `None` is a disk error. `Some(0)` is missing or a zero-length leaf.
fn glean_side(g: Result<u64, u64>) -> Option<u64> {
    match g {
        Ok(n) => Some(n),
        Err(e) if e == ENOENT => Some(0),
        Err(_) => None,
    }
}

fn side_wax(n: u64, buf: &[u8]) -> bool {
    n == 0 || (n == SEAL_LEN as u64 && all_zero(buf))
}

fn load() -> Result<Load, u64> {
    if !crate::cpu::has_aes_ni() || !crate::cpu::has_rdrand() {
        return Err(shut());
    }
    let hands = unsafe { &mut *core::ptr::addr_of_mut!(HANDS_SEAL) };
    let twin = unsafe { &mut *core::ptr::addr_of_mut!(TWIN_SEAL) };
    zero_bytes(hands);
    zero_bytes(twin);
    let gh = glean_side(glean_leaf(&HANDS_NAME, hands));
    let gt = glean_side(glean_leaf(&TWIN_NAME, twin));
    let (Some(a), Some(b)) = (gh, gt) else {
        return Err(split());
    };
    // A lone leaf cannot unseal. Treat it as wax so enlist can lay the pair
    // again. Split is two live seals that disagree.
    if side_wax(a, hands) || side_wax(b, twin) {
        if (a != 0 && a != SEAL_LEN as u64) || (b != 0 && b != SEAL_LEN as u64) {
            return Err(split());
        }
        return Ok(Load::Empty);
    }
    if a != SEAL_LEN as u64 || b != SEAL_LEN as u64 {
        return Err(split());
    }
    let vol = crate::usb::volume_id();
    let kh = wrap_key(b"kindling-hands", vol);
    let kt = wrap_key(b"kindling-twin", vol);
    let p1 = unsafe { &mut *core::ptr::addr_of_mut!(PLAIN) };
    let p2 = unsafe { &mut *core::ptr::addr_of_mut!(PLAIN2) };
    zero_bytes(as_bytes_mut(p1));
    zero_bytes(as_bytes_mut(p2));
    if !unseal_one(hands, &kh, p1) || !unseal_one(twin, &kt, p2) {
        return Err(split());
    }
    if !eq_bytes(as_bytes(p1), as_bytes(p2)) {
        return Err(split());
    }
    if p1.magic != MAGIC || p1.vol != vol {
        return Err(split());
    }
    Ok(Load::Live(*p1))
}

fn save(book: &Book) -> Result<(), u64> {
    if !crate::cpu::has_aes_ni() || !crate::cpu::has_rdrand() {
        return Err(shut());
    }
    let vol = crate::usb::volume_id();
    let mut b = *book;
    b.magic = MAGIC;
    b.vol = vol;
    let kh = wrap_key(b"kindling-hands", vol);
    let kt = wrap_key(b"kindling-twin", vol);
    let hands = unsafe { &mut *core::ptr::addr_of_mut!(HANDS_SEAL) };
    let twin = unsafe { &mut *core::ptr::addr_of_mut!(TWIN_SEAL) };
    zero_bytes(hands);
    zero_bytes(twin);
    if !seal_one(&b, &kh, hands) || !seal_one(&b, &kt, twin) {
        return Err(shut());
    }
    if eq_bytes(hands, twin) {
        return Err(split());
    }
    match stow_leaf(&HANDS_NAME, hands) {
        Ok(n) if n == SEAL_LEN as u64 => {}
        Ok(_) => {
            speak("the ink will not hold\n");
            wax_pair();
            return Err(err(EIO));
        }
        Err(e) => {
            wax_pair();
            return ink_miss(e);
        }
    }
    match stow_leaf(&TWIN_NAME, twin) {
        Ok(n) if n == SEAL_LEN as u64 => Ok(()),
        Ok(_) => {
            speak("the ink will not hold\n");
            wax_pair();
            Err(err(EIO))
        }
        Err(e) => {
            wax_pair();
            ink_miss(e)
        }
    }
}

/// Best-effort: both leaves wax so a miss cannot split the next roll.
fn wax_pair() {
    let hands = unsafe { &mut *core::ptr::addr_of_mut!(HANDS_SEAL) };
    let twin = unsafe { &mut *core::ptr::addr_of_mut!(TWIN_SEAL) };
    zero_bytes(hands);
    zero_bytes(twin);
    let _ = stow_leaf(&HANDS_NAME, hands);
    let _ = stow_leaf(&TWIN_NAME, twin);
}

fn live_count(book: &Book) -> usize {
    let mut n = 0usize;
    let mut i = 0usize;
    while i < SLOTS {
        if book.slots[i].flags & FLAG_LIVE != 0 {
            n += 1;
        }
        i += 1;
    }
    n
}

fn find_name(book: &Book, name: &[u8; NAME_LEN]) -> Option<usize> {
    let mut i = 0usize;
    while i < SLOTS {
        if book.slots[i].flags & FLAG_LIVE != 0 && eq_bytes(&book.slots[i].name, name) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn pad_name(src: &[u8]) -> [u8; NAME_LEN] {
    let mut n = [0u8; NAME_LEN];
    copy_bytes(&mut n, src);
    n
}

fn ink_miss(e: u64) -> Result<(), u64> {
    // Size miss already named the leaf. Wrong volume stays quiet.
    if e != EPERM {
        speak("the ink will not hold\n");
    }
    Err(err(e))
}

fn ink_hand(name: &[u8; NAME_LEN]) -> Result<(), u64> {
    match stow_leaf(&HAND_NAME, name) {
        Ok(n) if n == NAME_LEN as u64 => Ok(()),
        Ok(_) => {
            speak("the ink will not hold\n");
            Err(err(EIO))
        }
        Err(e) => ink_miss(e),
    }
}

fn empty_book() -> Book {
    Book {
        magic: MAGIC,
        vol: crate::usb::volume_id(),
        slots: [Slot {
            flags: 0,
            _pad: [0; 3],
            name: [0; NAME_LEN],
            salt: [0; SALT_LEN],
            verifier: [0; HASH_LEN],
        }; SLOTS],
        _pad: [0; 8],
    }
}

/// Copy occupied names into `buf` as 32-byte records. Returns the count.
pub fn roll(buf: u64, len: u64) -> u64 {
    if buf == 0 || len < (SLOTS * NAME_LEN) as u64 {
        return err(EPERM);
    }
    match load() {
        Err(e) => e,
        Ok(Load::Empty) => 0,
        Ok(Load::Live(book)) => {
            let mut n = 0u64;
            let mut i = 0usize;
            while i < SLOTS {
                if book.slots[i].flags & FLAG_LIVE != 0 {
                    let dst = buf + n * NAME_LEN as u64;
                    let mut k = 0usize;
                    while k < NAME_LEN {
                        unsafe {
                            ((dst + k as u64) as *mut u8).write_volatile(book.slots[i].name[k]);
                        }
                        k += 1;
                    }
                    n += 1;
                }
                i += 1;
            }
            n
        }
    }
}

pub fn enlist(name_ptr: u64, word_ptr: u64, confirm_ptr: u64) -> u64 {
    let (nb, nl) = match take_cstr(name_ptr, 31) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let (wb, wl) = match take_cstr(word_ptr, WORD_MAX) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let (cb, cl) = match take_cstr(confirm_ptr, WORD_MAX) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    if nl == 0 || wl == 0 || cl == 0 {
        return err(EINVAL);
    }
    if wl != cl || !eq_bytes(&wb[..wl], &cb[..cl]) {
        return err(EINVAL);
    }
    let name = pad_name(&nb[..nl]);
    let word = &wb[..wl];
    let mut book = match load() {
        Err(e) => return e,
        Ok(Load::Empty) => empty_book(),
        Ok(Load::Live(b)) => b,
    };
    if find_name(&book, &name).is_some() {
        speak("that hand is taken\n");
        return err(EEXIST);
    }
    let mut slot = None;
    let mut i = 0usize;
    while i < SLOTS {
        if book.slots[i].flags & FLAG_LIVE == 0 {
            slot = Some(i);
            break;
        }
        i += 1;
    }
    let Some(i) = slot else {
        speak("the book is full\n");
        return err(EPERM);
    };
    let mut salt = [0u8; SALT_LEN];
    if !rdrand_fill(&mut salt) {
        return shut();
    }
    let vol = crate::usb::volume_id();
    book.slots[i].flags = FLAG_LIVE;
    book.slots[i].name = name;
    book.slots[i].salt = salt;
    book.slots[i].verifier = verifier(&salt, word, vol, &name);
    if let Err(e) = save(&book) {
        return e;
    }
    match ink_hand(&name) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

fn check_word(slot: &Slot, word: &[u8], vol: u32) -> bool {
    let want = verifier(&slot.salt, word, vol, &slot.name);
    eq_bytes(&want, &slot.verifier)
}

pub fn choose(name_ptr: u64, word_ptr: u64) -> u64 {
    let (nb, nl) = match take_cstr(name_ptr, 31) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let (wb, wl) = match take_cstr(word_ptr, WORD_MAX) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let name = pad_name(&nb[..nl]);
    let book = match load() {
        Err(e) => return e,
        Ok(Load::Empty) => return err(ENOENT),
        Ok(Load::Live(b)) => b,
    };
    let Some(i) = find_name(&book, &name) else {
        speak("the word fails\n");
        return err(EPERM);
    };
    let vol = crate::usb::volume_id();
    if !check_word(&book.slots[i], &wb[..wl], vol) {
        speak("the word fails\n");
        return err(EPERM);
    }
    match ink_hand(&name) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

pub fn dismiss(name_ptr: u64, word_ptr: u64) -> u64 {
    let (nb, nl) = match take_cstr(name_ptr, 31) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let (wb, wl) = match take_cstr(word_ptr, WORD_MAX) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let name = pad_name(&nb[..nl]);
    let mut book = match load() {
        Err(e) => return e,
        Ok(Load::Empty) => return err(ENOENT),
        Ok(Load::Live(b)) => b,
    };
    let Some(i) = find_name(&book, &name) else {
        speak("the word fails\n");
        return err(EPERM);
    };
    if live_count(&book) <= 1 {
        speak("the last hand stays\n");
        return err(EPERM);
    }
    let vol = crate::usb::volume_id();
    if !check_word(&book.slots[i], &wb[..wl], vol) {
        speak("the word fails\n");
        return err(EPERM);
    }
    book.slots[i] = Slot {
        flags: 0,
        _pad: [0; 3],
        name: [0; NAME_LEN],
        salt: [0; SALT_LEN],
        verifier: [0; HASH_LEN],
    };
    match save(&book) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

#[cfg(feature = "house-test")]
pub fn self_test() -> u64 {
    let empty = crate::sha256::hash(b"");
    let want_empty: [u8; 32] = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];
    if empty != want_empty {
        return 20;
    }
    let abc = crate::sha256::hash(b"abc");
    let want_abc: [u8; 32] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    if abc != want_abc {
        return 21;
    }
    if !crate::cpu::has_aes_ni() {
        return 0;
    }
    let key = [0u8; 32];
    let block = [0u8; 16];
    let Some(out) = crate::aes::ecb_block(&key, &block) else {
        return 22;
    };
    let want_ecb: [u8; 16] = [
        0xdc, 0x95, 0xc0, 0x78, 0xa2, 0x40, 0x89, 0x89, 0xad, 0x48, 0xa2, 0x14, 0x92, 0x84, 0x20,
        0x87,
    ];
    if out != want_ecb {
        return 23;
    }
    let nonce = [0u8; 12];
    let pt = [0u8; 16];
    let mut ct = [0u8; 16];
    let Some(tag) = crate::aes::gcm_seal(&key, &nonce, &pt, &mut ct) else {
        return 24;
    };
    let want_ct: [u8; 16] = [
        0xce, 0xa7, 0x40, 0x3d, 0x4d, 0x60, 0x6b, 0x6e, 0x07, 0x4e, 0xc5, 0xd3, 0xba, 0xf3, 0x9d,
        0x18,
    ];
    let want_tag: [u8; 16] = [
        0xd0, 0xd1, 0xc8, 0xa7, 0x99, 0x99, 0x6b, 0xf0, 0x26, 0x5b, 0x98, 0xb5, 0xd4, 0x8a, 0xb9,
        0x19,
    ];
    if ct != want_ct || tag != want_tag {
        return 25;
    }
    let mut back = [0u8; 16];
    if !crate::aes::gcm_open(&key, &nonce, &ct, &tag, &mut back) || back != pt {
        return 26;
    }
    let mut bad = tag;
    bad[0] ^= 1;
    if crate::aes::gcm_open(&key, &nonce, &ct, &bad, &mut back) {
        return 27;
    }
    0
}
