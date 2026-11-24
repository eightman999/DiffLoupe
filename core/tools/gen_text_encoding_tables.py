#!/usr/bin/env python3
"""Generate the legacy text-encoding tables used by diffloupe-core's text_loader.

Run (macOS only; everything comes from this Mac, nothing is downloaded):

    cd core && python3 -I tools/gen_text_encoding_tables.py

Output: core/crates/diffloupe-core/src/legacy_tables/*.bin (little-endian blobs,
read via include_bytes! in src/legacy_tables.rs; formats are documented there).

Sources:
  * jisx0213.bin      JIS X 0213:2004 planes 1/2 <- Python codec `euc_jis_2004`
                      (cross-checked against `shift_jis_2004`, `iso2022_jp_2004`)
  * ebcdic_sbcs.bin   IBM037 / IBM500 / IBM1140 / IBM273 <- Python codecs cp037/cp500/cp1140/cp273;
                      IBM290 / IBM930 / IBM939 single-byte parts <- Apple libicucore (ibm-290_P100-1995,
                      ibm-930_P120-1999, ibm-939_P120-1999)
  * ibm930_dbcs.bin, ibm939_dbcs.bin
                      double-byte parts (in SO ... SI) <- Apple libicucore via ctypes
  * macjapanese.bin   MacJapanese <- CoreFoundation kCFStringEncodingMacJapanese via ctypes

Record format of the sparse tables: code u16, cp1 u32, cp2 u32 (0 = none), sorted by code.
"""
import ctypes
import os
import struct
import sys

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'crates', 'diffloupe-core', 'src', 'legacy_tables')
os.makedirs(OUT, exist_ok=True)

SECOND = {0: 0, 0x309A: 1, 0x0300: 2, 0x0301: 3, 0x02E5: 4, 0x02E9: 5}


def pack_jis(s):
    cps = [ord(c) for c in s]
    if len(cps) == 1:
        return cps[0]
    assert len(cps) == 2 and cps[1] in SECOND, cps
    return cps[0] | (SECOND[cps[1]] << 24)


# ---------- JIS X 0213 ----------
def gen_jisx0213():
    vals = []
    for plane in (1, 2):
        for row in range(1, 95):
            for cell in range(1, 95):
                b1, b2 = 0xA0 + row, 0xA0 + cell
                raw = bytes([b1, b2]) if plane == 1 else bytes([0x8F, b1, b2])
                try:
                    s = raw.decode('euc_jis_2004')
                except UnicodeDecodeError:
                    vals.append(0)
                    continue
                vals.append(pack_jis(s))
    open(os.path.join(OUT, 'jisx0213.bin'), 'wb').write(struct.pack('<%dI' % len(vals), *vals))
    return vals


# ---------- EBCDIC ----------
libicu = ctypes.CDLL('/usr/lib/libicucore.dylib')
libicu.ucnv_open.restype = ctypes.c_void_p
libicu.ucnv_open.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_int)]
libicu.ucnv_toUChars.restype = ctypes.c_int32
libicu.ucnv_toUChars.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int32, ctypes.c_char_p,
                                 ctypes.c_int32, ctypes.POINTER(ctypes.c_int)]
libicu.ucnv_reset.argtypes = [ctypes.c_void_p]


def icu_open(name):
    err = ctypes.c_int(0)
    c = libicu.ucnv_open(name, ctypes.byref(err))
    assert c and err.value <= 0, (name, err.value)
    return c


def icu_decode(cnv, raw):
    libicu.ucnv_reset(cnv)
    buf = (ctypes.c_uint16 * 16)()
    err = ctypes.c_int(0)
    n = libicu.ucnv_toUChars(cnv, buf, 16, raw, len(raw), ctypes.byref(err))
    if err.value > 0:
        return None
    cps = []
    i = 0
    units = [buf[k] for k in range(n)]
    while i < len(units):
        u = units[i]
        if 0xD800 <= u < 0xDC00 and i + 1 < len(units) and 0xDC00 <= units[i + 1] < 0xE000:
            cps.append(0x10000 + ((u - 0xD800) << 10) + (units[i + 1] - 0xDC00))
            i += 2
        else:
            cps.append(u)
            i += 1
    if 0xFFFD in cps:
        return None
    return cps


def sbcs_table_python(codec):
    out = []
    for b in range(256):
        try:
            s = bytes([b]).decode(codec)
            assert len(s) == 1
            out.append(ord(s))
        except UnicodeDecodeError:
            out.append(0xFFFF)
    return out


def sbcs_table_icu(name):
    cnv = icu_open(name)
    out = []
    for b in range(256):
        cps = icu_decode(cnv, bytes([b]))
        if cps is None or len(cps) != 1 or cps[0] > 0xFFFF:
            out.append(0xFFFF)
        else:
            out.append(cps[0])
    return out


def sparse(path, records):
    records.sort()
    with open(path, 'wb') as f:
        for code, cps in records:
            cp1 = cps[0]
            cp2 = cps[1] if len(cps) > 1 else 0
            assert len(cps) <= 2
            f.write(struct.pack('<HII', code, cp1, cp2))


def gen_ebcdic():
    tables = [
        sbcs_table_python('cp037'),
        sbcs_table_python('cp500'),
        sbcs_table_python('cp1140'),
        sbcs_table_python('cp273'),
        sbcs_table_icu(b'ibm-290_P100-1995'),
        sbcs_table_icu(b'ibm-930_P120-1999'),
        sbcs_table_icu(b'ibm-939_P120-1999'),
    ]
    with open(os.path.join(OUT, 'ebcdic_sbcs.bin'), 'wb') as f:
        for t in tables:
            f.write(struct.pack('<256H', *t))
    for name, icu in (('ibm930_dbcs.bin', b'ibm-930_P120-1999'), ('ibm939_dbcs.bin', b'ibm-939_P120-1999')):
        cnv = icu_open(icu)
        recs = []
        for b1 in range(0x40, 0xFF):
            for b2 in range(0x40, 0xFF):
                cps = icu_decode(cnv, bytes([0x0E, b1, b2, 0x0F]))
                if cps is None or not cps:
                    continue
                recs.append(((b1 << 8) | b2, cps))
        sparse(os.path.join(OUT, name), recs)
        print(name, len(recs), 'records')


# ---------- MacJapanese ----------
def gen_macjapanese():
    cf = ctypes.CDLL('/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation')
    cf.CFStringCreateWithBytes.restype = ctypes.c_void_p
    cf.CFStringCreateWithBytes.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_long, ctypes.c_uint32, ctypes.c_ubyte]
    cf.CFStringGetLength.restype = ctypes.c_long
    cf.CFStringGetLength.argtypes = [ctypes.c_void_p]
    cf.CFStringGetCharacters.argtypes = [ctypes.c_void_p, ctypes.c_long * 2, ctypes.c_void_p]
    cf.CFRelease.argtypes = [ctypes.c_void_p]
    MAC_JAPANESE = 1  # kCFStringEncodingMacJapanese

    class CFRange(ctypes.Structure):
        _fields_ = [('location', ctypes.c_long), ('length', ctypes.c_long)]

    cf.CFStringGetCharacters.argtypes = [ctypes.c_void_p, CFRange, ctypes.c_void_p]

    def dec(raw):
        s = cf.CFStringCreateWithBytes(None, raw, len(raw), MAC_JAPANESE, 0)
        if not s:
            return None
        n = cf.CFStringGetLength(s)
        buf = (ctypes.c_uint16 * (n + 1))()
        cf.CFStringGetCharacters(s, CFRange(0, n), buf)
        cf.CFRelease(s)
        units = [buf[i] for i in range(n)]
        text = b''.join(struct.pack('<H', u) for u in units).decode('utf-16-le', 'surrogatepass')
        cps = [ord(c) for c in text]
        if 0xFFFD in cps or not cps:
            return None
        return cps

    recs = []
    for b in range(256):
        cps = dec(bytes([b]))
        if cps is not None:
            recs.append((b, cps))
    leads = list(range(0x81, 0xA0)) + list(range(0xE0, 0xFD))
    for b1 in leads:
        for b2 in range(0x40, 0xFD):
            if b2 == 0x7F:
                continue
            cps = dec(bytes([b1, b2]))
            if cps is not None and len(cps) <= 2:
                recs.append(((b1 << 8) | b2, cps))
    sparse(os.path.join(OUT, 'macjapanese.bin'), recs)
    print('macjapanese.bin', len(recs), 'records')


if __name__ == '__main__':
    v = gen_jisx0213()
    print('jisx0213.bin', len(v), 'cells,', sum(1 for x in v if x), 'mapped')
    gen_ebcdic()
    gen_macjapanese()
